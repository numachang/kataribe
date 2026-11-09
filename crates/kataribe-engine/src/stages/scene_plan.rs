//! 章をシーンに分ける（シーン構成の生成）。

use std::collections::BTreeMap;

use kataribe_project::{Chapter, ChapterId, ChapterMeta, SceneId, ScenePlan, layout};
use kataribe_text::count::count_chars;
use minijinja::context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Stage;
use super::materials::{briefs, non_empty, round_to_hundreds, scene_count_for, snapshot_chapter};
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::prompt::PromptTemplate;

const SCENE_PLAN_OUTPUT_TOKENS: u32 = 4096;
/// 書き済みの本文が目標を使い切っていても、章に割り振る字数の下限。
const MIN_CHAPTER_CHARS: u32 = 1000;
const STORYLINE_CHARS: usize = 600;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScenePlanOutput {
    scenes: Vec<PlannedScene>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlannedScene {
    title: String,
    summary: String,
    pov: String,
    characters: Vec<String>,
    place: String,
    time: String,
}

#[derive(Debug, Serialize)]
struct ChapterSummary {
    number: u32,
    title: String,
    storyline: String,
    is_current: bool,
}

pub(super) async fn scenes(stage: &Stage<'_>, chapter_id: ChapterId) -> Result<ChangeSet> {
    let chapters = stage.project.chapters()?;
    let (chapter, base) = snapshot_chapter(stage.project, chapter_id)?;
    let chapter_chars = chars_for_chapter(stage, &chapters, chapter_id)?;
    let scene_count = scene_count_for(chapter_chars);
    let chars_per_scene = round_to_hundreds(chapter_chars / scene_count);
    let summaries: Vec<ChapterSummary> = chapters
        .iter()
        .map(|other| ChapterSummary {
            number: other.id.number(),
            title: other.meta.title.clone(),
            storyline: if other.id == chapter_id {
                other.storyline.trim().to_owned()
            } else {
                excerpt::head(&other.storyline, STORYLINE_CHARS)
            },
            is_current: other.id == chapter_id,
        })
        .collect();
    let prompt = stage.prompts.render(
        PromptTemplate::ScenePlan,
        &context! {
            project => &stage.info,
            characters => briefs(&stage.project.characters()?, 0),
            chapters => summaries,
            chapter => context! { number => chapter.id.number(), title => &chapter.meta.title },
            scene_count => scene_count,
            chars_per_scene => chars_per_scene,
            manuscript => stage.existing_manuscript(SCENE_PLAN_OUTPUT_TOKENS),
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, SCENE_PLAN_OUTPUT_TOKENS)?;
    let label = format!("第{}章のシーン構成を生成", chapter.id.number());
    let output: ScenePlanOutput = stage.caller.json(&label, &prompt, output_tokens).await?;
    if output.scenes.is_empty() {
        return Err(EngineError::InvalidOutput(
            "シーンが一つもありませんでした。".into(),
        ));
    }

    warn_about_existing_manuscripts(stage, &chapter)?;
    let updated = Chapter {
        meta: ChapterMeta {
            scenes: numbered_scenes(output.scenes, chars_per_scene),
            ..chapter.meta.clone()
        },
        ..chapter
    };
    let mut changes = ChangeSet::new(format!(
        "第{}章を {} シーンに分けました。",
        updated.id.number(),
        updated.meta.scenes.len()
    ));
    changes.put(
        layout::chapter_path(&updated.id),
        updated.render()?,
        Some(base),
    );
    Ok(changes)
}

/// この章に割り振る字数。ほかの章にすでに本文があれば、目標からその字数を引いた残りを、本文のない章で分ける。
/// 取り込んだ原稿のあとの章の目標が、実際の残りと合うようにする。本文が無ければ、これまでどおり目標を章の数で割る。
fn chars_for_chapter(
    stage: &Stage<'_>,
    chapters: &[Chapter],
    chapter_id: ChapterId,
) -> Result<u32> {
    let target = stage.manifest.target_length;
    let evenly = target / u32::try_from(chapters.len()).unwrap_or(1).max(1);
    let mut written_chars: u32 = 0;
    let mut chapters_to_write: u32 = 1;
    for chapter in chapters.iter().filter(|chapter| chapter.id != chapter_id) {
        let chars = written_chars_of(stage, chapter)?;
        if chars == 0 {
            chapters_to_write += 1;
        } else {
            written_chars = written_chars.saturating_add(chars);
        }
    }
    if written_chars == 0 {
        return Ok(evenly);
    }
    Ok((target.saturating_sub(written_chars) / chapters_to_write).max(MIN_CHAPTER_CHARS))
}

fn written_chars_of(stage: &Stage<'_>, chapter: &Chapter) -> Result<u32> {
    let mut chars = 0usize;
    for scene in &chapter.meta.scenes {
        if let Some(text) = stage.project.scene_text(&chapter.id, &scene.id)? {
            chars += count_chars(&text);
        }
    }
    Ok(u32::try_from(chars).unwrap_or(u32::MAX))
}

/// シーンに s01, s02, … の id を振る。本文のファイル名はこの id になる。
fn numbered_scenes(planned: Vec<PlannedScene>, target_chars: u32) -> Vec<ScenePlan> {
    let mut ids: Vec<SceneId> = Vec::new();
    planned
        .into_iter()
        .map(|scene| {
            let id = SceneId::next_available(&ids);
            ids.push(id);
            ScenePlan {
                id,
                title: scene.title,
                summary: scene.summary,
                pov: non_empty(&scene.pov),
                characters: scene
                    .characters
                    .into_iter()
                    .filter(|name| !name.trim().is_empty())
                    .collect(),
                place: non_empty(&scene.place),
                time: non_empty(&scene.time),
                target_chars: Some(target_chars),
                beats: Vec::new(),
                extra: BTreeMap::default(),
            }
        })
        .collect()
}

/// 書き終えた本文があるのにシーン構成を作り直すと、本文と構成が食い違うおそれがある。
fn warn_about_existing_manuscripts(stage: &Stage<'_>, chapter: &Chapter) -> Result<()> {
    let mut written = Vec::new();
    for scene in &chapter.meta.scenes {
        if stage.project.scene_text(&chapter.id, &scene.id)?.is_some() {
            written.push(layout::scene_text_path(&chapter.id, &scene.id).to_string());
        }
    }
    if !written.is_empty() {
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!(
                "この章にはすでに本文があります（{}）。新しいシーン構成と内容が合わなくなる場合は、本文を書き直してください。",
                written.join("、")
            ),
        );
    }
    Ok(())
}
