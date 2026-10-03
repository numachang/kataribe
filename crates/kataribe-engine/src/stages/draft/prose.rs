//! 本文 1 回分の生成。後処理・品質チェック・再生成・推敲をまとめて行う。

use kataribe_text::count::count_chars;
use kataribe_text::quality::{IssueKind, QualityOptions, QualityReport, Severity, analyze};
use minijinja::context;

use crate::cleanup::{clean_prose, drop_unfinished_sentence};
use crate::error::{EngineError, Result};
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::prompt::{Prompt, PromptTemplate};
use crate::stages::Stage;

/// 推敲の結果がこれより短くなったら、内容が削られたとみなして推敲前の文章を使う。
const MIN_POLISH_RATIO: f64 = 0.8;
/// 目標の文字数に対してこれより短い出力は、書けなかったものとして生成し直す。
const MIN_OUTPUT_RATIO: f64 = 0.3;
const STYLE_CHARS_FOR_POLISH: usize = 1500;

/// 本文 1 回分の依頼。
pub(crate) struct ProseJob<'a> {
    pub label: String,
    pub prompt: Prompt,
    pub target_chars: u32,
    /// 重複の除去に使う、直前の本文（書きかけがあればそれ、無ければ前のシーン）。
    pub previous_text: &'a str,
    /// 推敲パスに渡す文体ガイド。
    pub style: &'a str,
}

/// 生成した本文と、その品質の検査結果。
struct Attempt {
    text: String,
    report: QualityReport,
}

impl Attempt {
    fn new(text: String, target_chars: u32) -> Self {
        let report = inspect(&text, target_chars);
        Self { text, report }
    }

    /// 生成し直す理由になる問題。
    fn problems(&self, target_chars: u32) -> Vec<String> {
        let mut problems: Vec<String> = error_messages(&self.report).map(str::to_owned).collect();
        if is_too_short(&self.text, target_chars) {
            problems.push("本文がほとんど書かれていません".to_owned());
        }
        problems
    }
}

/// 1 回分の本文を生成する。重大な問題があれば設定の回数まで生成し直し、最も問題の少ないものを使う。
pub(crate) async fn generate(stage: &Stage<'_>, job: &ProseJob<'_>) -> Result<String> {
    let output_tokens =
        stage.output_tokens(&job.prompt, stage.output_tokens_for_chars(job.target_chars))?;
    let mut best = attempt(stage, job, output_tokens).await?;
    let mut retries_left = stage.settings.quality_retries;
    while !best.problems(job.target_chars).is_empty() && retries_left > 0 {
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!(
                "品質チェックで問題が見つかったため、生成し直します（{}）。",
                best.problems(job.target_chars).join("、")
            ),
        );
        stage.caller.expect_steps(1);
        retries_left -= 1;
        let candidate = attempt(stage, job, output_tokens).await?;
        if candidate.problems(job.target_chars).len() < best.problems(job.target_chars).len() {
            best = candidate;
        }
    }
    if is_too_short(&best.text, job.target_chars) {
        // 既存の本文を、ほとんど空の文章で置き換えないようにする
        return Err(EngineError::InvalidOutput(format!(
            "「{}」の本文を書けませんでした（{} 字）。設定やモデルを見直してください。",
            job.label,
            count_chars(&best.text)
        )));
    }
    let remaining = best.problems(job.target_chars);
    if !remaining.is_empty() {
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!(
                "品質チェックの問題が残っています（{}）。確認して直してください。",
                remaining.join("、")
            ),
        );
    }
    if stage.settings.polish {
        return polish(stage, job, best).await;
    }
    Ok(best.text)
}

async fn attempt(stage: &Stage<'_>, job: &ProseJob<'_>, output_tokens: u32) -> Result<Attempt> {
    let output = stage
        .caller
        .text(&job.label, &job.prompt, output_tokens)
        .await?;
    let text = clean_prose(&output.text, job.previous_text);
    let text = if output.truncated {
        drop_unfinished_sentence(&text)
    } else {
        text
    };
    Ok(Attempt::new(text, job.target_chars))
}

/// 推敲パス。分量が大きく減ったり新たな問題が出たりしたら、推敲前の文章を使う。
async fn polish(stage: &Stage<'_>, job: &ProseJob<'_>, draft: Attempt) -> Result<String> {
    // 分量の過不足は推敲で直す対象ではない（「分量は変えずに」と矛盾するため渡さない）
    let issues: Vec<&str> = draft
        .report
        .issues
        .iter()
        .filter(|issue| !matches!(issue.kind, IssueKind::TooShort | IssueKind::TooLong))
        .map(|issue| issue.message.as_str())
        .collect();
    let prompt = stage.prompts.render(
        PromptTemplate::Polish,
        &context! {
            project => &stage.info,
            style => excerpt::head(job.style, STYLE_CHARS_FOR_POLISH),
            text => &draft.text,
            issues => issues,
        },
    )?;
    let original_chars = count_chars(&draft.text);
    let target = u32::try_from(original_chars).unwrap_or(u32::MAX);
    let output_tokens = stage.output_tokens(&prompt, stage.output_tokens_for_chars(target))?;
    stage.caller.expect_steps(1);
    let output = stage
        .caller
        .text(&format!("{}の推敲", job.label), &prompt, output_tokens)
        .await?;
    let polished = clean_prose(&output.text, "");
    if !output.truncated && is_acceptable_polish(original_chars, &polished, target) {
        Ok(polished)
    } else {
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            "推敲の結果に問題があったため、推敲前の文章を使います。",
        );
        Ok(draft.text)
    }
}

fn is_acceptable_polish(original_chars: usize, polished: &str, target: u32) -> bool {
    #[allow(clippy::cast_precision_loss)]
    let ratio = count_chars(polished) as f64 / original_chars.max(1) as f64;
    ratio >= MIN_POLISH_RATIO && error_messages(&inspect(polished, target)).next().is_none()
}

fn is_too_short(text: &str, target_chars: u32) -> bool {
    #[allow(clippy::cast_precision_loss)]
    let ratio = count_chars(text) as f64 / f64::from(target_chars.max(1));
    ratio < MIN_OUTPUT_RATIO
}

pub(crate) fn inspect(text: &str, target_chars: u32) -> QualityReport {
    analyze(
        text,
        &QualityOptions {
            target_chars: Some(target_chars as usize),
        },
    )
}

fn error_messages(report: &QualityReport) -> impl Iterator<Item = &str> {
    report
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .map(|issue| issue.message.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_that_drops_content_is_rejected() {
        let original = "雨が降っていた。".repeat(20);
        let original_chars = count_chars(&original);
        assert!(!is_acceptable_polish(
            original_chars,
            "雨が降っていた。",
            160
        ));
        assert!(is_acceptable_polish(original_chars, &original, 160));
    }

    #[test]
    fn nearly_empty_output_counts_as_a_problem() {
        let attempt = Attempt::new("　雨。\n".to_owned(), 1000);
        assert_ne!(attempt.problems(1000), Vec::<String>::new());
        let attempt = Attempt::new("　雨が降っていた。".repeat(40), 1000);
        assert_eq!(attempt.problems(1000), Vec::<String>::new());
    }
}
