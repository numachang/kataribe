//! 本文の品質指標と問題点の検出。
//!
//! LLM が生成した本文を機械的に検査し、再生成すべきかどうかの判断材料にする。

// 本文の文字数・文の数は現実的にはせいぜい数万程度で、f64 の仮数部（52 bit）に
// 対して十分小さい。比率や平均を求めるための usize -> f64 変換に精度の実害は無い。
#![allow(clippy::cast_precision_loss)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::count::{self, TextStats};
use crate::meta;
use crate::ruby;

/// 12 文字以上の同一部分文字列が 3 回以上出現するかを調べるときの窓の大きさ。
const REPEATED_PHRASE_WINDOW: usize = 12;
/// 「繰り返し」とみなす最小出現回数。
const REPEATED_PHRASE_MIN_OCCURRENCES: usize = 3;
/// `repeated_phrase` 警告を出す反復率のしきい値。
const REPEATED_PHRASE_RATIO_THRESHOLD: f64 = 0.08;
/// `monotonous_endings` 警告を出す連続数のしきい値。
const MONOTONOUS_ENDING_RUN_THRESHOLD: usize = 5;
/// `repeated_sentence` 警告の対象とする最小文字数。
const REPEATED_SENTENCE_MIN_CHARS: usize = 10;
/// 英単語がこの数だけ連続したら `foreign_script` 警告を出す。
const FOREIGN_ENGLISH_WORD_THRESHOLD: usize = 5;
/// 日本語の語に挟まれた英字の断片とみなす最大の長さ。
/// LLM の出力が乱れたときに混ざるのは「Myc」「ing」のような短い断片で、
/// 「iPhone」のような正当な語まで拾わないよう短く限る。
const EMBEDDED_LATIN_FRAGMENT_MAX_CHARS: usize = 4;
/// 目標文字数に対する下限・上限の倍率。
const TOO_SHORT_RATIO: f64 = 0.7;
const TOO_LONG_RATIO: f64 = 1.5;
/// `excerpt` に載せる本文の最大文字数。
const EXCERPT_MAX_CHARS: usize = 60;

/// [`analyze`] の挙動を切り替えるオプション。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct QualityOptions {
    /// 目標文字数。指定すると `too_short` / `too_long` を検出する。
    pub target_chars: Option<usize>,
}

/// 問題点の深刻さ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// 検出する問題点の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum IssueKind {
    MetaCommentary,
    MarkdownArtifact,
    ForeignScript,
    RepeatedSentence,
    RepeatedPhrase,
    MonotonousEndings,
    TooShort,
    TooLong,
    UnbalancedBrackets,
}

/// 検出された 1 件の問題点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct QualityIssue {
    pub kind: IssueKind,
    pub severity: Severity,
    pub message: String,
    /// 該当箇所の抜粋。行全体に関わる問題でない場合などは `None`。
    pub excerpt: Option<String>,
}

/// 機械的に測定できる品質指標。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct QualityMetrics {
    /// 「」『』の内側の文字数 / 全文字数。
    pub dialogue_ratio: f64,
    /// 漢字の数 / 全文字数。
    pub kanji_ratio: f64,
    /// 文（。！？と閉じ括弧で区切る）の平均文字数。
    pub average_sentence_length: f64,
    /// 地の文で、文末 2 文字が同じ文が連続した最大数。
    pub longest_same_ending_run: usize,
    /// 12 文字以上の同一部分文字列が 3 回以上出現する箇所が本文に占める割合。
    pub repeated_phrase_ratio: f64,
}

/// 品質指標と問題点をまとめた検査結果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct QualityReport {
    pub stats: TextStats,
    pub metrics: QualityMetrics,
    pub issues: Vec<QualityIssue>,
}

impl QualityReport {
    /// 重大な問題（[`Severity::Error`]）が 1 件でもあるかどうか。
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == Severity::Error)
    }
}

/// 本文を検査し、品質指標と問題点をまとめて返す。
#[must_use]
pub fn analyze(text: &str, options: &QualityOptions) -> QualityReport {
    let stats = count::stats(text);
    let plain = ruby::to_plain(text);
    let sentences = split_sentences(&plain);
    let dense_chars = visible_chars(&plain);

    let longest_same_ending_run = longest_same_ending_run(&sentences);
    let repeated_phrase_ratio = repeated_phrase_ratio(&dense_chars);

    let metrics = QualityMetrics {
        dialogue_ratio: dialogue_ratio(&plain, stats.chars),
        kanji_ratio: kanji_ratio(&plain, stats.chars),
        average_sentence_length: average_sentence_length(&sentences),
        longest_same_ending_run,
        repeated_phrase_ratio,
    };

    let mut issues = Vec::new();
    collect_line_issues(text, &mut issues);
    collect_foreign_script_issue(text, &mut issues);
    collect_repeated_sentence_issue(&sentences, &mut issues);
    collect_repeated_phrase_issue(repeated_phrase_ratio, &mut issues);
    collect_monotonous_endings_issue(longest_same_ending_run, &mut issues);
    collect_length_issues(stats.chars, options.target_chars, &mut issues);
    collect_unbalanced_brackets_issue(&plain, &mut issues);

    QualityReport {
        stats,
        metrics,
        issues,
    }
}

/// 空白・改行を除いた 1 文。会話文（鉤括弧の中）かどうかを保持する。
struct Sentence {
    text: String,
    is_dialogue: bool,
}

impl Sentence {
    fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// 比較用の文末 2 文字。2 文字に満たない場合はそれ以下の文字列。
    fn ending(&self) -> String {
        let len = self.char_count();
        self.text.chars().skip(len.saturating_sub(2)).collect()
    }
}

fn is_whitespace_like(c: char) -> bool {
    matches!(c, ' ' | '\u{3000}' | '\t' | '\n' | '\r')
}

fn is_sentence_delimiter(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '」' | '』')
}

/// 空白を除いた各文字に、そのとき鉤括弧の内側かどうかを付けて返す。
fn tag_dialogue_depth(plain: &str) -> Vec<(char, bool)> {
    let mut depth = 0usize;
    let mut tagged = Vec::new();

    for c in plain.chars() {
        if is_whitespace_like(c) {
            continue;
        }
        match c {
            '「' | '『' => {
                depth += 1;
                tagged.push((c, true));
            }
            '」' | '』' => {
                tagged.push((c, depth > 0));
                depth = depth.saturating_sub(1);
            }
            _ => tagged.push((c, depth > 0)),
        }
    }

    tagged
}

/// 。！？と閉じ括弧（の連続）で文を区切る。
///
/// 句読点や閉じ括弧が連続する場合（例：「た。」の `。」`）は 1 つの区切りとしてまとめる。
fn split_sentences(plain: &str) -> Vec<Sentence> {
    let tagged = tag_dialogue_depth(plain);
    let mut sentences = Vec::new();
    let mut buffer = String::new();
    let mut buffer_is_dialogue = false;
    let mut i = 0;

    while i < tagged.len() {
        let (c, inside) = tagged[i];
        buffer.push(c);
        buffer_is_dialogue |= inside;
        i += 1;

        if is_sentence_delimiter(c) {
            while i < tagged.len() && is_sentence_delimiter(tagged[i].0) {
                buffer.push(tagged[i].0);
                buffer_is_dialogue |= tagged[i].1;
                i += 1;
            }
            sentences.push(Sentence {
                text: std::mem::take(&mut buffer),
                is_dialogue: buffer_is_dialogue,
            });
            buffer_is_dialogue = false;
        }
    }

    if !buffer.is_empty() {
        sentences.push(Sentence {
            text: buffer,
            is_dialogue: buffer_is_dialogue,
        });
    }

    sentences
}

fn dialogue_ratio(plain: &str, total_chars: usize) -> f64 {
    if total_chars == 0 {
        return 0.0;
    }
    let dialogue_chars = tag_dialogue_depth(plain)
        .into_iter()
        .filter(|&(c, inside)| inside && !matches!(c, '「' | '』' | '『' | '」'))
        .count();
    dialogue_chars as f64 / total_chars as f64
}

fn kanji_ratio(plain: &str, total_chars: usize) -> f64 {
    if total_chars == 0 {
        return 0.0;
    }
    let kanji_chars = plain.chars().filter(|&c| ruby::is_kanji(c)).count();
    kanji_chars as f64 / total_chars as f64
}

fn average_sentence_length(sentences: &[Sentence]) -> f64 {
    if sentences.is_empty() {
        return 0.0;
    }
    let total: usize = sentences.iter().map(Sentence::char_count).sum();
    total as f64 / sentences.len() as f64
}

/// 地の文（会話文ではない文）で、文末 2 文字が同じ文が連続した最大数。
///
/// 会話文を挟むと、読んだときの単調さは途切れると考え、連続はリセットする
/// （地の文どうしが会話文をまたいでも「連続」とはみなさない）。
fn longest_same_ending_run(sentences: &[Sentence]) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    let mut previous_ending: Option<String> = None;

    for sentence in sentences {
        if sentence.is_dialogue {
            current = 0;
            previous_ending = None;
            continue;
        }
        let ending = sentence.ending();
        if ending.chars().count() < 2 {
            previous_ending = None;
            current = 0;
            continue;
        }
        current = if previous_ending.as_deref() == Some(ending.as_str()) {
            current + 1
        } else {
            1
        };
        longest = longest.max(current);
        previous_ending = Some(ending);
    }

    longest
}

fn visible_chars(plain: &str) -> Vec<char> {
    plain.chars().filter(|&c| !is_whitespace_like(c)).collect()
}

/// 12 文字以上の同一部分文字列が 3 回以上出現する箇所が占める割合を、
/// 固定長の窓（[`REPEATED_PHRASE_WINDOW`]）を使って概算する。
///
/// 本文全体を総当たりで比較するのではなく、窓ごとの出現回数をハッシュマップで
/// 数えるため、2 万字程度の本文でも一瞬で終わる。
fn repeated_phrase_ratio(dense_chars: &[char]) -> f64 {
    let len = dense_chars.len();
    if len < REPEATED_PHRASE_WINDOW {
        return 0.0;
    }

    let window_starts = len - REPEATED_PHRASE_WINDOW + 1;
    let mut window_counts: HashMap<&[char], usize> = HashMap::with_capacity(window_starts);
    for start in 0..window_starts {
        let window = &dense_chars[start..start + REPEATED_PHRASE_WINDOW];
        *window_counts.entry(window).or_insert(0) += 1;
    }

    let mut covered = vec![false; len];
    for start in 0..window_starts {
        let window = &dense_chars[start..start + REPEATED_PHRASE_WINDOW];
        if window_counts[window] >= REPEATED_PHRASE_MIN_OCCURRENCES {
            covered[start..start + REPEATED_PHRASE_WINDOW].fill(true);
        }
    }

    covered.iter().filter(|&&c| c).count() as f64 / len as f64
}

fn excerpt(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= EXCERPT_MAX_CHARS {
        return trimmed.to_string();
    }
    let mut truncated: String = trimmed.chars().take(EXCERPT_MAX_CHARS).collect();
    truncated.push('…');
    truncated
}

fn collect_line_issues(text: &str, issues: &mut Vec<QualityIssue>) {
    for line in text.lines() {
        if meta::is_markdown_artifact_line(line) {
            issues.push(QualityIssue {
                kind: IssueKind::MarkdownArtifact,
                severity: Severity::Error,
                message: "本文に Markdown の記法が混入しています。".to_string(),
                excerpt: Some(excerpt(line)),
            });
        } else if meta::is_meta_phrase_line(line) {
            issues.push(QualityIssue {
                kind: IssueKind::MetaCommentary,
                severity: Severity::Error,
                message: "本文に LLM の前置き・後書きらしい行が混入しています。".to_string(),
                excerpt: Some(excerpt(line)),
            });
        }
    }
}

fn collect_foreign_script_issue(original: &str, issues: &mut Vec<QualityIssue>) {
    if let Some(found) = find_foreign_letter_line(original) {
        issues.push(QualityIssue {
            kind: IssueKind::ForeignScript,
            severity: Severity::Error,
            message:
                "本文に日本語では使わない文字（ハングル・簡体字・他の言語の文字）が混入しています。"
                    .to_string(),
            excerpt: Some(found),
        });
        return;
    }
    if let Some(run) = detect_long_english_run(original) {
        issues.push(QualityIssue {
            kind: IssueKind::ForeignScript,
            severity: Severity::Error,
            message: "本文に長い英文が混入しています。".to_string(),
            excerpt: Some(excerpt(&run)),
        });
        return;
    }
    if let Some(line) = find_embedded_latin_fragment_line(original) {
        issues.push(QualityIssue {
            kind: IssueKind::ForeignScript,
            severity: Severity::Error,
            message: "本文の日本語の途中に、意味をなさない英字の断片が混入しています。".to_string(),
            excerpt: Some(line),
        });
    }
}

/// 仮名・漢字に前後を挟まれた、小文字を含む短い英字の並び（「上Mycを」の「Myc」）がある行。
/// 大文字だけの略語（「OK」）や、括弧・空白で区切られた英語は対象にしない。
fn find_embedded_latin_fragment_line(original: &str) -> Option<String> {
    original
        .lines()
        .find(|line| has_embedded_latin_fragment(line))
        .map(excerpt)
}

fn has_embedded_latin_fragment(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let mut start = 0;
    while start < chars.len() {
        if !chars[start].is_ascii_alphabetic() {
            start += 1;
            continue;
        }
        let end = chars[start..]
            .iter()
            .position(|c| !c.is_ascii_alphabetic())
            .map_or(chars.len(), |length| start + length);
        let run = &chars[start..end];
        let is_fragment = run.len() <= EMBEDDED_LATIN_FRAGMENT_MAX_CHARS
            && run.iter().any(char::is_ascii_lowercase)
            && start > 0
            && end < chars.len()
            && is_japanese_letter(chars[start - 1])
            && is_japanese_letter(chars[end]);
        if is_fragment {
            return true;
        }
        start = end;
    }
    false
}

/// ひらがな・カタカナ・漢字（語の一部になる文字。記号や句読点は含まない）。
fn is_japanese_letter(c: char) -> bool {
    matches!(
        c,
        '\u{3041}'..='\u{3096}' | '\u{30A1}'..='\u{30FA}' | '\u{30FC}' | '\u{4E00}'..='\u{9FFF}' | '々'
    )
}

fn find_foreign_letter_line(original: &str) -> Option<String> {
    original
        .lines()
        .find(|line| line.chars().any(is_foreign_letter))
        .map(excerpt)
}

/// 日本語の小説の本文には現れない文字（LLM が他の言語に引きずられたときに混ざる）。
fn is_foreign_letter(c: char) -> bool {
    is_hangul(c) || is_simplified_only_kanji(c) || is_other_script_letter(c)
}

/// 発音記号付きのラテン文字、キリル文字、アラビア文字、デーヴァナーガリー、タイ文字。
fn is_other_script_letter(c: char) -> bool {
    matches!(
        c,
        '\u{00C0}'..='\u{00D6}'
            | '\u{00D8}'..='\u{00F6}'
            | '\u{00F8}'..='\u{024F}'
            | '\u{0400}'..='\u{04FF}'
            | '\u{0600}'..='\u{06FF}'
            | '\u{0900}'..='\u{097F}'
            | '\u{0E00}'..='\u{0E7F}'
    )
}

fn is_hangul(c: char) -> bool {
    matches!(c, '\u{AC00}'..='\u{D7A3}')
}

/// 日本語の字形とは重ならない、簡体字だけで使われる漢字。
fn is_simplified_only_kanji(c: char) -> bool {
    matches!(
        c,
        '们' | '这'
            | '说'
            | '对'
            | '时'
            | '还'
            | '过'
            | '为'
            | '么'
            | '吗'
            | '呢'
            | '个'
            | '请'
            | '让'
            | '开'
            | '关'
            | '问'
            | '东'
            | '车'
            | '长'
            | '门'
            | '马'
            | '鸟'
            | '龙'
            | '见'
            | '觉'
            | '头'
            | '应'
            | '该'
            | '样'
            | '现'
            | '实'
            | '书'
            | '买'
            | '卖'
    )
}

fn is_english_word(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| c.is_ascii_punctuation());
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '\'')
}

/// 英単語が [`FOREIGN_ENGLISH_WORD_THRESHOLD`] 個以上続く箇所を探す。
fn detect_long_english_run(text: &str) -> Option<String> {
    let mut buffer: Vec<&str> = Vec::new();
    for token in text.split_whitespace() {
        if is_english_word(token) {
            buffer.push(token);
            if buffer.len() >= FOREIGN_ENGLISH_WORD_THRESHOLD {
                return Some(buffer.join(" "));
            }
        } else {
            buffer.clear();
        }
    }
    None
}

fn collect_repeated_sentence_issue(sentences: &[Sentence], issues: &mut Vec<QualityIssue>) {
    if let Some(text) = detect_repeated_sentence(sentences) {
        issues.push(QualityIssue {
            kind: IssueKind::RepeatedSentence,
            severity: Severity::Warning,
            message: "同じ文が繰り返し使われています。".to_string(),
            excerpt: Some(excerpt(text)),
        });
    }
}

fn detect_repeated_sentence(sentences: &[Sentence]) -> Option<&str> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for sentence in sentences {
        if sentence.char_count() >= REPEATED_SENTENCE_MIN_CHARS {
            *counts.entry(sentence.text.as_str()).or_insert(0) += 1;
        }
    }
    let mut repeated: Vec<&str> = counts
        .into_iter()
        .filter(|&(_, count)| count >= 2)
        .map(|(text, _)| text)
        .collect();
    repeated.sort_unstable();
    repeated.into_iter().next()
}

fn collect_repeated_phrase_issue(ratio: f64, issues: &mut Vec<QualityIssue>) {
    if ratio > REPEATED_PHRASE_RATIO_THRESHOLD {
        issues.push(QualityIssue {
            kind: IssueKind::RepeatedPhrase,
            severity: Severity::Warning,
            message: format!(
                "同じ言い回しが多用されています（反復率 {:.0}%）。",
                ratio * 100.0
            ),
            excerpt: None,
        });
    }
}

fn collect_monotonous_endings_issue(longest_run: usize, issues: &mut Vec<QualityIssue>) {
    if longest_run >= MONOTONOUS_ENDING_RUN_THRESHOLD {
        issues.push(QualityIssue {
            kind: IssueKind::MonotonousEndings,
            severity: Severity::Warning,
            message: format!("地の文の文末が {longest_run} 文連続で同じ形になっています。"),
            excerpt: None,
        });
    }
}

fn collect_length_issues(
    chars: usize,
    target_chars: Option<usize>,
    issues: &mut Vec<QualityIssue>,
) {
    let Some(target_chars) = target_chars else {
        return;
    };
    let chars_f = chars as f64;
    let target_f = target_chars as f64;

    if chars_f < target_f * TOO_SHORT_RATIO {
        issues.push(QualityIssue {
            kind: IssueKind::TooShort,
            severity: Severity::Warning,
            message: format!(
                "目標文字数の 7 割未満です（{chars} 文字 / 目標 {target_chars} 文字）。"
            ),
            excerpt: None,
        });
    } else if chars_f > target_f * TOO_LONG_RATIO {
        issues.push(QualityIssue {
            kind: IssueKind::TooLong,
            severity: Severity::Warning,
            message: format!(
                "目標文字数の 1.5 倍を超えています（{chars} 文字 / 目標 {target_chars} 文字）。"
            ),
            excerpt: None,
        });
    }
}

fn collect_unbalanced_brackets_issue(plain: &str, issues: &mut Vec<QualityIssue>) {
    if let Some(message) = unbalanced_bracket_message(plain) {
        issues.push(QualityIssue {
            kind: IssueKind::UnbalancedBrackets,
            severity: Severity::Warning,
            message,
            excerpt: None,
        });
    }
}

fn unbalanced_bracket_message(plain: &str) -> Option<String> {
    let open_kagi = plain.matches('「').count();
    let close_kagi = plain.matches('」').count();
    if open_kagi != close_kagi {
        return Some(format!(
            "「」の数が一致しません（開き {open_kagi}・閉じ {close_kagi}）。"
        ));
    }

    let open_nikagi = plain.matches('『').count();
    let close_nikagi = plain.matches('』').count();
    if open_nikagi != close_nikagi {
        return Some(format!(
            "『』の数が一致しません（開き {open_nikagi}・閉じ {close_nikagi}）。"
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use pretty_assertions::assert_eq;

    use super::*;

    fn analyze_default(text: &str) -> QualityReport {
        analyze(text, &QualityOptions::default())
    }

    #[test]
    fn natural_novel_text_has_no_issues() {
        let text = "\
朝の光が窓辺に差し込んでいた。\n\
「もう起きる時間よ」と母が言った。\n\
彼は目をこすりながら起き上がった。\n\
『今日は学校に行きたくないな』と、彼は心の中でつぶやいた。\n\
それでも彼は制服に着替え、階段を下りていった。\n\
台所からは味噌汁の匂いが漂ってきて、少しだけ気持ちが軽くなった。";

        let report = analyze_default(text);
        assert_eq!(report.issues, Vec::new());
        assert!(!report.has_errors());
        assert!(report.metrics.dialogue_ratio > 0.0);
        assert!(report.metrics.kanji_ratio > 0.0);
    }

    #[test]
    fn detects_meta_commentary_and_markdown_as_errors() {
        let text = "以下は本文です。\n# 第一章\n本物の物語がここから始まる。";
        let report = analyze_default(text);
        assert!(report.has_errors());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::MetaCommentary)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::MarkdownArtifact)
        );
    }

    #[test]
    fn detects_hangul_as_foreign_script() {
        let text = "彼女は안녕하세요と言った。".to_string() + &"これは普通の文だ。".repeat(3);
        let report = analyze_default(&text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::ForeignScript)
        );
    }

    #[test]
    fn detects_simplified_chinese_only_kanji_as_foreign_script() {
        let text = "彼女はこれについて这样说了。".to_string() + &"これは普通の文だ。".repeat(3);
        let report = analyze_default(&text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::ForeignScript)
        );
    }

    #[test]
    fn detects_accented_latin_and_cyrillic_as_foreign_script() {
        for text in [
            "その瞳には常に något な緊張感が宿る。",
            "彼は привет と言った。",
        ] {
            let report = analyze(text, &QualityOptions::default());
            assert!(
                report
                    .issues
                    .iter()
                    .any(|issue| issue.kind == IssueKind::ForeignScript),
                "{text}"
            );
        }
    }

    #[test]
    fn plain_ascii_names_are_not_foreign_script() {
        let report = analyze(
            "　ジョンは「OK」と答え、ラジオを消した。",
            &QualityOptions::default(),
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::ForeignScript)
        );
    }

    #[test]
    fn detects_latin_fragment_inside_japanese_words() {
        let report = analyze_default("　彼女の声がわずかに上Mycを跳ねさせた。");
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.kind == IssueKind::ForeignScript)
            .expect("英字の断片を検出する");
        assert_eq!(issue.severity, Severity::Error);
        assert!(issue.excerpt.as_deref().unwrap_or_default().contains("Myc"));
    }

    #[test]
    fn legitimate_latin_words_are_not_fragments() {
        for text in [
            "　ジョンはOKと答えた。",
            "　彼のiPhoneが鳴った。",
            "　Myc は行頭の英字だ。",
            "　「Myc」と彼は書いた。",
            "　値はpH7だった。",
        ] {
            let report = analyze_default(text);
            assert!(
                !report
                    .issues
                    .iter()
                    .any(|issue| issue.kind == IssueKind::ForeignScript),
                "{text}"
            );
        }
    }

    #[test]
    fn detects_long_english_run_as_foreign_script() {
        let text = "This is a very long English sentence inside the manuscript.\n本文はここから。";
        let report = analyze_default(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::ForeignScript)
        );
    }

    #[test]
    fn detects_repeated_sentence() {
        let text =
            "彼はゆっくりと部屋を出ていった。\n何も言わずに。\n彼はゆっくりと部屋を出ていった。";
        let report = analyze_default(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::RepeatedSentence)
        );
    }

    #[test]
    fn detects_monotonous_endings() {
        // すべて文末 2 文字が「た。」で終わる、地の文だけの文を 6 つ並べる。
        let text = "彼は歩いた。\n彼は止まった。\n彼は考えた。\n彼は頷いた。\n彼は笑った。\n彼は立ち止まった。";
        let report = analyze_default(text);
        assert!(report.metrics.longest_same_ending_run >= 5);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::MonotonousEndings)
        );
    }

    #[test]
    fn dialogue_endings_do_not_count_toward_monotonous_narration() {
        let text = "「そうだ」\n「そうだ」\n「そうだ」\n「そうだ」\n「そうだ」\n「そうだ」";
        let report = analyze_default(text);
        assert_eq!(report.metrics.longest_same_ending_run, 0);
    }

    #[test]
    fn detects_too_short_and_too_long() {
        let short_report = analyze(
            "短い文章。",
            &QualityOptions {
                target_chars: Some(1000),
            },
        );
        assert!(
            short_report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::TooShort)
        );

        let long_text = "とても長い文章だ。".repeat(200);
        let long_report = analyze(
            &long_text,
            &QualityOptions {
                target_chars: Some(100),
            },
        );
        assert!(
            long_report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::TooLong)
        );
    }

    #[test]
    fn no_target_chars_means_no_length_issues() {
        let report = analyze_default("短い。");
        assert!(
            !report
                .issues
                .iter()
                .any(|i| matches!(i.kind, IssueKind::TooShort | IssueKind::TooLong))
        );
    }

    #[test]
    fn detects_unbalanced_brackets() {
        let text = "「これは閉じられていない。\n普通の文が続く。";
        let report = analyze_default(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::UnbalancedBrackets)
        );
    }

    #[test]
    fn balanced_brackets_are_not_flagged() {
        let text = "「これは正しく閉じている」\n『これも同じく』";
        let report = analyze_default(text);
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::UnbalancedBrackets)
        );
    }

    #[test]
    fn repeated_phrase_ratio_detects_repeated_long_substring() {
        let phrase = "その部屋には古い机と椅子が置かれていた";
        let text = format!("{phrase}。それから彼は考えた。{phrase}。そして再び{phrase}と思った。");
        let report = analyze_default(&text);
        assert!(report.metrics.repeated_phrase_ratio > 0.0);
    }

    #[test]
    fn repeated_phrase_ratio_finishes_quickly_for_long_text() {
        let paragraph = "とても長い夜だった。彼女は静かに窓の外を見つめていた。".repeat(400);
        let started = Instant::now();
        let _ = analyze_default(&paragraph);
        assert!(
            started.elapsed().as_secs() < 2,
            "analyze took too long: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn has_errors_reflects_error_severity_only() {
        let only_warning = QualityReport {
            stats: TextStats {
                chars: 0,
                paragraphs: 0,
                dialogue_lines: 0,
                manuscript_pages: 0.0,
            },
            metrics: QualityMetrics {
                dialogue_ratio: 0.0,
                kanji_ratio: 0.0,
                average_sentence_length: 0.0,
                longest_same_ending_run: 0,
                repeated_phrase_ratio: 0.0,
            },
            issues: vec![QualityIssue {
                kind: IssueKind::TooShort,
                severity: Severity::Warning,
                message: String::new(),
                excerpt: None,
            }],
        };
        assert!(!only_warning.has_errors());
    }
}
