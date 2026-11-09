//! 本文の生成。

mod context;
mod digest;
mod prose;
mod story;
mod units;

use std::ops::Range;

use kataribe_project::{ChapterId, SceneId};

use super::Stage;
use super::materials::require;
use super::references;
use crate::change_set::ChangeSet;
use crate::error::Result;
use crate::settings::DraftUnit;
use context::{DraftMaterial, verbatim_prose_tokens};
use story::{Position, Story};

/// 使いすぎの表現を探す範囲（直前までの本文の末尾の文字数）。
const RECENT_PROSE_CHARS: usize = 20_000;

pub(super) async fn write(
    stage: &Stage<'_>,
    chapter_id: ChapterId,
    scene_id: SceneId,
) -> Result<ChangeSet> {
    let story = Story::load(stage.project)?;
    require(
        !story.style.is_empty(),
        "先に文体ガイドを生成してください。",
    )?;
    let position = story.locate(chapter_id, scene_id)?;
    let scenes = match stage.settings.draft_unit {
        DraftUnit::Chapter => story.unwritten_run(stage.project, position)?,
        DraftUnit::Scene | DraftUnit::Beat => position.scene..position.scene + 1,
    };
    let verbatim = story.verbatim_prose(
        stage.project,
        position,
        verbatim_prose_tokens(
            stage.settings.context_tokens,
            stage.prompt_budget(wanted_output_tokens(stage, &story, position, &scenes)),
        ),
    )?;
    // そのまま渡すシーンの要約は要らないので、LLM に作らせない
    let story_so_far = digest::story_so_far(stage, &story, verbatim.start).await?;
    let recent_prose = story.recent_prose(stage.project, position, RECENT_PROSE_CHARS)?;
    let references = references::load(stage)?;
    let material = DraftMaterial {
        story: &story,
        position,
        unit: stage.settings.draft_unit,
        scenes,
        story_so_far: &story_so_far,
        verbatim_scenes: &verbatim.scenes,
        recent_prose: &recent_prose,
        references: &references,
    };
    match stage.settings.draft_unit {
        DraftUnit::Scene => units::by_scene(stage, &material).await,
        DraftUnit::Beat => units::by_beat(stage, &material).await,
        DraftUnit::Chapter => units::by_chapter(stage, &material).await,
    }
}

/// 今回書く範囲を 1 回で書かせるときに確保する出力トークン数。
/// 書き継ぐ単位（シーン・ビート）の 1 回はこれより少ないが、要約するかどうかを書き始める前に決めるので、多めに見る。
fn wanted_output_tokens(
    stage: &Stage<'_>,
    story: &Story,
    position: Position,
    scenes: &Range<usize>,
) -> u32 {
    let chars = scenes
        .clone()
        .map(|scene| story.scene_target(Position { scene, ..position }, &stage.manifest))
        .fold(0, u32::saturating_add);
    stage.output_tokens_for_chars(chars)
}
