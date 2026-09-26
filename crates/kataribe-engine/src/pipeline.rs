//! 生成の工程の一覧。どの工程が済み、どれに取りかかれるかを判定する。

use kataribe_project::{Chapter, Project, layout};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::settings::DraftUnit;
use crate::stages::materials::{chapter_label, document_body};
use crate::task::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum StepState {
    Done,
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PipelineStep {
    pub task: Task,
    pub label: String,
    pub state: StepState,
    /// `Blocked` のとき、先に済ませる必要がある工程の説明。
    pub blocked_by: Option<String>,
}

/// 工程の前提条件。満たしていなければ `missing` を理由として示す。
struct Prerequisite {
    satisfied: bool,
    missing: &'static str,
}

fn needs(satisfied: bool, missing: &'static str) -> Prerequisite {
    Prerequisite { satisfied, missing }
}

fn step(task: Task, label: String, done: bool, prerequisites: &[Prerequisite]) -> PipelineStep {
    let unmet: Vec<&str> = prerequisites
        .iter()
        .filter(|prerequisite| !prerequisite.satisfied)
        .map(|prerequisite| prerequisite.missing)
        .collect();
    let (state, blocked_by) = match (done, unmet.is_empty()) {
        (true, _) => (StepState::Done, None),
        (false, true) => (StepState::Ready, None),
        (false, false) => (
            StepState::Blocked,
            Some(format!("先に{}が必要です。", unmet.join("・"))),
        ),
    };
    PipelineStep {
        task,
        label,
        state,
        blocked_by,
    }
}

/// 作品の生成工程を、取りかかる順に並べて返す。
pub fn pipeline(project: &Project, unit: DraftUnit) -> Result<Vec<PipelineStep>> {
    let manifest = project.manifest()?;
    let has_idea = !manifest.idea.trim().is_empty();
    let has_concept = has_document(project, layout::CONCEPT)?;
    let has_style = has_document(project, layout::STYLE)?;
    let has_world = has_document(project, layout::WORLD_OVERVIEW)?;
    let has_synopsis = has_document(project, layout::SYNOPSIS)?;
    let characters = project.characters()?;
    let chapters = project.chapters()?;

    let mut steps = vec![
        step(
            Task::Concept,
            "企画".into(),
            has_concept,
            &[needs(has_idea, "企画の種")],
        ),
        step(
            Task::Style,
            "文体ガイド".into(),
            has_style,
            &[needs(has_concept, "企画")],
        ),
        step(
            Task::World,
            "世界観".into(),
            has_world,
            &[needs(has_concept, "企画")],
        ),
        step(
            Task::Cast,
            "登場人物の一覧".into(),
            !characters.is_empty(),
            &[needs(has_concept, "企画"), needs(has_world, "世界観")],
        ),
    ];
    steps.extend(characters.iter().map(|character| {
        step(
            Task::Character {
                id: character.id.clone(),
            },
            format!("人物資料: {}", character.meta.name),
            !character.body.trim().is_empty(),
            &[],
        )
    }));
    steps.push(step(
        Task::Synopsis,
        "あらすじ".into(),
        has_synopsis,
        &[needs(!characters.is_empty(), "登場人物")],
    ));
    steps.push(step(
        Task::Outline,
        "章立て".into(),
        !chapters.is_empty(),
        &[needs(has_synopsis, "あらすじ")],
    ));
    steps.extend(chapters.iter().map(|chapter| {
        step(
            Task::ScenePlan {
                chapter: chapter.id,
            },
            format!("{}のシーン構成", chapter_label(chapter)),
            !chapter.meta.scenes.is_empty(),
            &[],
        )
    }));
    steps.extend(draft_steps(project, &chapters, unit, has_style)?);
    Ok(steps)
}

fn has_document(project: &Project, path: &str) -> Result<bool> {
    Ok(!document_body(project, path)?.is_empty())
}

/// 本文の工程。前のシーン（章単位なら前の章）を書き終えるまで、次には進めない。
fn draft_steps(
    project: &Project,
    chapters: &[Chapter],
    unit: DraftUnit,
    has_style: bool,
) -> Result<Vec<PipelineStep>> {
    let mut steps = Vec::new();
    let mut everything_before_is_written = true;
    for chapter in chapters {
        let written = written_scenes(project, chapter)?;
        match unit {
            DraftUnit::Chapter => steps.extend(chapter_step(
                chapter,
                &written,
                has_style,
                everything_before_is_written,
            )),
            DraftUnit::Scene | DraftUnit::Beat => {
                steps.extend(scene_steps(
                    chapter,
                    &written,
                    has_style,
                    everything_before_is_written,
                ));
            }
        }
        // シーン構成の無い章は、まだ書き終えていない（次の章の本文には進めない）
        everything_before_is_written &= !written.is_empty() && written.iter().all(|done| *done);
    }
    Ok(steps)
}

/// 章の各シーンの本文が書かれているか。
fn written_scenes(project: &Project, chapter: &Chapter) -> Result<Vec<bool>> {
    chapter
        .meta
        .scenes
        .iter()
        .map(|scene| {
            let text = project.scene_text(&chapter.id, &scene.id)?;
            Ok(text.is_some_and(|text| !text.trim().is_empty()))
        })
        .collect()
}

/// 章単位: まだ書いていない最初のシーンから章の終わりまでを 1 つの工程にする。
fn chapter_step(
    chapter: &Chapter,
    written: &[bool],
    has_style: bool,
    previous_written: bool,
) -> Option<PipelineStep> {
    let first_scene = chapter.meta.scenes.first()?;
    let first_unwritten = written.iter().position(|done| !done);
    let scene = first_unwritten.map_or(first_scene, |index| &chapter.meta.scenes[index]);
    Some(step(
        Task::Draft {
            chapter: chapter.id,
            scene: scene.id,
        },
        format!("{}の本文", chapter_label(chapter)),
        first_unwritten.is_none(),
        &[
            needs(has_style, "文体ガイド"),
            needs(previous_written, "前の章の本文"),
        ],
    ))
}

/// シーン単位・ビート単位: シーンごとの工程。前のシーンを書き終えていなければ進めない。
fn scene_steps(
    chapter: &Chapter,
    written: &[bool],
    has_style: bool,
    previous_written: bool,
) -> Vec<PipelineStep> {
    let mut everything_before_is_written = previous_written;
    let mut steps = Vec::with_capacity(chapter.meta.scenes.len());
    for (index, scene) in chapter.meta.scenes.iter().enumerate() {
        steps.push(step(
            Task::Draft {
                chapter: chapter.id,
                scene: scene.id,
            },
            format!(
                "{} シーン{}「{}」の本文",
                chapter_label(chapter),
                index + 1,
                scene.title
            ),
            written[index],
            &[
                needs(has_style, "文体ガイド"),
                needs(everything_before_is_written, "前のシーンの本文"),
            ],
        ));
        everything_before_is_written &= written[index];
    }
    steps
}
