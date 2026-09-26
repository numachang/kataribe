//! 章をシーンに分ける（シーン構成の生成）。

use std::collections::BTreeMap;

use kataribe_project::{Chapter, ChapterId, ChapterMeta, SceneId, ScenePlan, layout};
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
    let chapter_chars =
        stage.manifest.target_length / u32::try_from(chapters.len()).unwrap_or(1).max(1);
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
