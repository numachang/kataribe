//! 企画・文体ガイド・世界観・あらすじ（front matter の無い Markdown の資料）の生成。

use kataribe_project::{MarkdownDoc, RelPath, TextFile, layout};
use kataribe_text::quality::{IssueKind, QualityOptions, analyze};
use minijinja::context;

use super::Stage;
use super::materials::{briefs, document_body, require, world_text};
use crate::change_set::ChangeSet;
use crate::cleanup::clean_markdown;
use crate::error::{EngineError, Result};
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::prompt::{Prompt, PromptTemplate};

/// 資料 1 つの出力に確保するトークン数。
const DOCUMENT_OUTPUT_TOKENS: u32 = 4096;
const CONCEPT_CHARS: usize = 2500;
const WORLD_CHARS: usize = 2500;
const PROFILE_CHARS: usize = 400;

pub(super) async fn concept(stage: &Stage<'_>) -> Result<ChangeSet> {
    require(
        !stage.info.idea.is_empty(),
        "企画の種（作品情報の idea）を書いてから生成してください。",
    )?;
    let prompt = stage.prompts.render(
        PromptTemplate::Concept,
        &context! { project => &stage.info },
    )?;
    generate(stage, "企画", layout::CONCEPT, &prompt).await
}

pub(super) async fn style(stage: &Stage<'_>) -> Result<ChangeSet> {
    let concept = required_concept(stage)?;
    let prompt = stage.prompts.render(
        PromptTemplate::Style,
        &context! { project => &stage.info, concept => concept },
    )?;
    generate(stage, "文体ガイド", layout::STYLE, &prompt).await
}

pub(super) async fn world(stage: &Stage<'_>) -> Result<ChangeSet> {
    let concept = required_concept(stage)?;
    let prompt = stage.prompts.render(
        PromptTemplate::World,
        &context! { project => &stage.info, concept => concept },
    )?;
    generate(stage, "世界観", layout::WORLD_OVERVIEW, &prompt).await
}

pub(super) async fn synopsis(stage: &Stage<'_>) -> Result<ChangeSet> {
    let concept = required_concept(stage)?;
    let characters = stage.project.characters()?;
    require(!characters.is_empty(), "先に登場人物を生成してください。")?;
    let prompt = stage.prompts.render(
        PromptTemplate::Synopsis,
        &context! {
            project => &stage.info,
            concept => concept,
            world => excerpt::head(&world_text(stage.project)?, WORLD_CHARS),
            characters => briefs(&characters, PROFILE_CHARS),
        },
    )?;
    generate(stage, "あらすじ", layout::SYNOPSIS, &prompt).await
}

fn required_concept(stage: &Stage<'_>) -> Result<String> {
    let concept = document_body(stage.project, layout::CONCEPT)?;
    require(!concept.is_empty(), "先に企画を生成してください。")?;
    Ok(excerpt::head(&concept, CONCEPT_CHARS))
}

async fn generate(
    stage: &Stage<'_>,
    label: &str,
    path: &str,
    prompt: &Prompt,
) -> Result<ChangeSet> {
    let path = RelPath::new(path)?;
    let base = stage.snapshot(&path)?;
    let output_tokens = stage.output_tokens(prompt, DOCUMENT_OUTPUT_TOKENS)?;
    let body = write_document(stage, &format!("{label}を生成"), prompt, output_tokens).await?;
    let content = replace_body(base.as_ref(), body)?;
    let mut changes = ChangeSet::new(format!("{label}を生成しました。"));
    changes.put(path, content, base);
    Ok(changes)
}

/// 資料の本文を 1 つ生成する。空だったり日本語以外の文字が混ざったりしたら、設定の回数まで生成し直す。
/// 日本語以外の文字が残っても、資料の一部なので止めずに警告して使う（空なら失敗にする）。
pub(super) async fn write_document(
    stage: &Stage<'_>,
    label: &str,
    prompt: &Prompt,
    output_tokens: u32,
) -> Result<String> {
    let mut retries_left = stage.settings.quality_retries;
    loop {
        let output = stage.caller.text(label, prompt, output_tokens).await?;
        let body = clean_markdown(&output.text);
        let Some(problem) = document_problem(&body) else {
            return Ok(body);
        };
        if retries_left == 0 {
            if body.trim().is_empty() {
                return Err(EngineError::InvalidOutput(format!(
                    "「{label}」の内容が空でした。"
                )));
            }
            notice(
                stage.caller.sink(),
                NoticeLevel::Warning,
                format!("「{label}」: {problem}"),
            );
            return Ok(body);
        }
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!("「{label}」: {problem} 生成し直します。"),
        );
        stage.caller.expect_steps(1);
        retries_left -= 1;
    }
}

fn document_problem(body: &str) -> Option<String> {
    if body.trim().is_empty() {
        return Some("内容が空でした。".to_owned());
    }
    analyze(body, &QualityOptions::default())
        .issues
        .into_iter()
        .find(|issue| issue.kind == IssueKind::ForeignScript)
        .map(|issue| issue.message)
}

/// 既存の資料に front matter（題名など）があれば残し、本文だけを差し替える。
fn replace_body(base: Option<&TextFile>, body: String) -> Result<String> {
    match base {
        Some(file) => Ok(MarkdownDoc {
            body,
            ..MarkdownDoc::parse(&file.content)?
        }
        .render()?),
        None => Ok(body),
    }
}
