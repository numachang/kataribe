//! `run --until <STAGE>` の段階指定。
//!
//! [`kataribe_engine::pipeline`] が返す工程は、企画→文体→世界観→キャスト→人物→
//! あらすじ→章立て→シーン構成→本文、の順に並ぶ。ここではその並び順を序数として
//! 扱い、「指定した段階までの工程がすべて済んだか」を判定する。

use clap::ValueEnum;
use kataribe_engine::{PipelineStep, StepState, Task};

/// `--until` に指定できる段階。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Stage {
    Concept,
    Style,
    World,
    Cast,
    Characters,
    Synopsis,
    Outline,
    Scenes,
    Draft,
}

impl Stage {
    fn order(self) -> u8 {
        match self {
            Stage::Concept => 0,
            Stage::Style => 1,
            Stage::World => 2,
            Stage::Cast => 3,
            Stage::Characters => 4,
            Stage::Synopsis => 5,
            Stage::Outline => 6,
            Stage::Scenes => 7,
            Stage::Draft => 8,
        }
    }
}

/// 工程が属する段階の序数。[`Task::Revise`] と、足す生成（`AddCharacter`・`AddWorldDocument`）は工程一覧には現れないため扱わない。
fn task_order(task: &Task) -> u8 {
    match task {
        Task::Concept => 0,
        Task::Style => 1,
        Task::World => 2,
        Task::Cast => 3,
        Task::Character { .. } => 4,
        Task::Synopsis => 5,
        Task::Outline => 6,
        Task::ScenePlan { .. } => 7,
        Task::Draft { .. } => 8,
        Task::Revise { .. } | Task::AddCharacter { .. } | Task::AddWorldDocument { .. } => u8::MAX,
    }
}

/// `task` が `stage` と同じか、それより手前の段階に属するか。
///
/// `run --until` が、指定した段階より後ろの工程を生成の候補にしないために使う
/// （`pipeline` が返す一覧の並び順に依存しないようにするため）。
#[must_use]
pub(crate) fn task_is_within_stage(task: &Task, stage: Stage) -> bool {
    task_order(task) <= stage.order()
}

/// `steps` のうち、`stage` と同じか、それより手前の段階の工程がすべて済んでいるか。
///
/// 章立てが済むまで章ごとの工程がまだ 1 つも無いように、後の段階の工程は
/// 前提が満たされるまで工程一覧に現れない。そのため「該当する工程が無い」ことは
/// 「その段階が済んでいる」ことの十分な根拠にならない。より手前の段階まで含めて
/// すべて `Done` であることを確かめることで、この見せかけの完了を避ける。
#[must_use]
pub fn reached_stage(steps: &[PipelineStep], stage: Stage) -> bool {
    steps
        .iter()
        .filter(|step| task_is_within_stage(&step.task, stage))
        .all(|step| step.state == StepState::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_project::{ChapterId, CharacterId};

    fn step(task: Task, state: StepState) -> PipelineStep {
        PipelineStep {
            task,
            label: "テスト".into(),
            state,
            blocked_by: None,
        }
    }

    #[test]
    fn task_is_within_stage_excludes_later_categories() {
        assert!(task_is_within_stage(&Task::Concept, Stage::Concept));
        assert!(task_is_within_stage(&Task::Style, Stage::Outline));
        assert!(!task_is_within_stage(
            &Task::Draft {
                chapter: ChapterId::from_number(1),
                scene: kataribe_project::SceneId::from_number(1),
            },
            Stage::Scenes
        ));
    }

    #[test]
    fn not_reached_when_an_earlier_stage_is_still_pending() {
        let steps = vec![
            step(Task::Concept, StepState::Done),
            step(Task::Style, StepState::Ready),
            step(Task::World, StepState::Blocked),
        ];
        assert!(!reached_stage(&steps, Stage::World));
    }

    #[test]
    fn reached_when_every_step_up_to_the_stage_is_done() {
        let steps = vec![
            step(Task::Concept, StepState::Done),
            step(Task::Style, StepState::Done),
            step(Task::World, StepState::Ready),
        ];
        assert!(reached_stage(&steps, Stage::Style));
        assert!(!reached_stage(&steps, Stage::World));
    }

    #[test]
    fn later_stage_with_no_instances_yet_is_not_vacuously_reached() {
        // 章立てがまだ無いので Draft の工程は 1 つも無いが、
        // それより手前の Outline がまだ Ready なので「Draft まで到達」とは扱わない。
        let steps = vec![
            step(Task::Concept, StepState::Done),
            step(Task::Style, StepState::Done),
            step(Task::World, StepState::Done),
            step(Task::Cast, StepState::Done),
            step(
                Task::Character {
                    id: CharacterId::new("rin").unwrap(),
                },
                StepState::Done,
            ),
            step(Task::Synopsis, StepState::Done),
            step(Task::Outline, StepState::Ready),
        ];
        assert!(!reached_stage(&steps, Stage::Draft));
    }

    #[test]
    fn stage_is_reached_once_its_own_category_has_no_pending_instances() {
        let steps = vec![
            step(Task::Concept, StepState::Done),
            step(Task::Style, StepState::Done),
            step(Task::World, StepState::Done),
            step(Task::Cast, StepState::Done),
            step(
                Task::Character {
                    id: CharacterId::new("rin").unwrap(),
                },
                StepState::Done,
            ),
            step(Task::Synopsis, StepState::Done),
            step(Task::Outline, StepState::Done),
            step(
                Task::ScenePlan {
                    chapter: ChapterId::from_number(1),
                },
                StepState::Done,
            ),
        ];
        assert!(reached_stage(&steps, Stage::Scenes));
    }
}
