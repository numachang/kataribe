//! 章立て（章ごとの題とストーリーライン）の生成。

use std::collections::BTreeMap;

use kataribe_project::{Chapter, ChapterId, ChapterMeta, TextFile, layout};
use minijinja::context;
use schemars::JsonSchema;
use serde::Deserialize;

use super::Stage;
use super::materials::{
    briefs, chapter_count_for, document_body, require, round_to_hundreds, snapshot_chapter,
};
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::prompt::PromptTemplate;

const OUTLINE_OUTPUT_TOKENS: u32 = 4096;
const CONCEPT_CHARS: usize = 1500;
const SYNOPSIS_CHARS: usize = 4000;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Outline {
    chapters: Vec<OutlineChapter>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OutlineChapter {
    title: String,
    storyline: String,
}

/// あらすじを章に分ける。既存の章は、シーン構成を残して題とストーリーラインだけを更新する。
pub(super) async fn chapters(stage: &Stage<'_>) -> Result<ChangeSet> {
    let synopsis = document_body(stage.project, layout::SYNOPSIS)?;
    require(!synopsis.is_empty(), "先にあらすじを生成してください。")?;
    let chapter_count = chapter_count_for(stage.manifest.target_length);
    let existing = stage
        .project
        .chapters()?
        .iter()
        .map(|chapter| snapshot_chapter(stage.project, chapter.id))
        .collect::<Result<Vec<_>>>()?;
    let prompt = stage.prompts.render(
        PromptTemplate::Outline,
        &context! {
            project => &stage.info,
            concept => excerpt::head(&document_body(stage.project, layout::CONCEPT)?, CONCEPT_CHARS),
            characters => briefs(&stage.project.characters()?, 0),
            synopsis => excerpt::head(&synopsis, SYNOPSIS_CHARS),
            chapter_count => chapter_count,
            chars_per_chapter => round_to_hundreds(stage.manifest.target_length / chapter_count),
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, OUTLINE_OUTPUT_TOKENS)?;
    let outline: Outline = stage
        .caller
        .json("章立てを生成", &prompt, output_tokens)
        .await?;
    if outline.chapters.is_empty() {
        return Err(EngineError::InvalidOutput(
            "章が一つもありませんでした。".into(),
        ));
    }

    warn_about_leftover_chapters(stage, &existing, outline.chapters.len());
    let mut changes = ChangeSet::new(format!(
        "{} 章の章立てを生成しました。",
        outline.chapters.len()
    ));
    for (index, planned) in outline.chapters.into_iter().enumerate() {
        let id = ChapterId::from_number(u32::try_from(index + 1).unwrap_or(u32::MAX));
        let current = existing.iter().find(|(chapter, _)| chapter.id == id);
        let base = current.map(|(_, file)| file.clone());
        let chapter = match current {
            Some((current, _)) => Chapter {
                meta: ChapterMeta {
                    title: planned.title,
                    ..current.meta.clone()
                },
                storyline: planned.storyline,
                ..current.clone()
            },
            None => Chapter {
                id,
                meta: ChapterMeta {
                    title: planned.title,
                    scenes: Vec::new(),
                    extra: BTreeMap::default(),
                },
                storyline: planned.storyline,
            },
        };
        changes.put(layout::chapter_path(&id), chapter.render()?, base);
    }
    Ok(changes)
}

fn warn_about_leftover_chapters(
    stage: &Stage<'_>,
    existing: &[(Chapter, TextFile)],
    new_count: usize,
) {
    let leftovers: Vec<String> = existing
        .iter()
        .skip(new_count)
        .map(|(chapter, _)| layout::chapter_path(&chapter.id).to_string())
        .collect();
    if !leftovers.is_empty() {
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!(
                "新しい章立てより後ろの既存の章（{}）はそのまま残ります。不要なら削除してください。",
                leftovers.join("、")
            ),
        );
    }
}
