//! 本文の生成。

mod context;
mod digest;
mod prose;
mod story;
mod units;

use kataribe_project::{ChapterId, SceneId};

use super::Stage;
use super::materials::require;
use crate::change_set::ChangeSet;
use crate::error::Result;
use crate::settings::DraftUnit;
use context::DraftMaterial;
use story::Story;

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
    let story_so_far = digest::story_so_far(stage, &story, position).await?;
    let scenes = match stage.settings.draft_unit {
        DraftUnit::Chapter => story.unwritten_run(stage.project, position)?,
        DraftUnit::Scene | DraftUnit::Beat => position.scene..position.scene + 1,
    };
    let material = DraftMaterial {
        story: &story,
        position,
        unit: stage.settings.draft_unit,
        scenes,
        story_so_far: &story_so_far,
    };
    match stage.settings.draft_unit {
        DraftUnit::Scene => units::by_scene(stage, &material).await,
        DraftUnit::Beat => units::by_beat(stage, &material).await,
        DraftUnit::Chapter => units::by_chapter(stage, &material).await,
    }
}
