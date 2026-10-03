//! `run` サブコマンド: 取りかかれる工程(ready)を順に生成・適用し続ける。

use std::time::Instant;

use anyhow::Context;
use kataribe_engine::{
    ChangeSet, Engine, EngineError, GenerationEvent, PipelineStep, StepState, pipeline,
};
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::ApplyGuard;
use crate::args::{GlobalOptions, RunArgs};
use crate::output::{Console, StdoutBoundary, report_generation_event};
use crate::settings;
use crate::stage::{Stage, reached_stage, task_is_within_stage};

use super::Outcome;

pub async fn run(
    args: &RunArgs,
    global: &GlobalOptions,
    console: &dyn Console,
    cancel: &CancellationToken,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<Outcome> {
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let settings = settings::load_effective_settings(global, Some(&project))?;
    let unit = settings.generation.draft_unit;
    let model = super::build_chat_model(&settings.llm, &global.api_key_env)?;
    let engine =
        Engine::new(model, settings.generation).context("執筆エンジンを初期化できません")?;

    let boundary = StdoutBoundary::new();

    let mut executed = 0u32;
    loop {
        let steps = pipeline(&project, unit).context("工程の一覧を取得できません")?;

        if let Some(stage) = args.until
            && reached_stage(&steps, stage)
        {
            console.eprint("指定した段階まで完了しました。\n")?;
            return Ok(Outcome::Success);
        }

        // `--until` があるときは、それより後ろの段階の工程を候補にしない。
        // `pipeline` が返す並び順（たまたま段階順になっている）に依存せず、
        // ここで明示的に境界を決める。
        let candidates = candidates_up_to(steps, args.until);

        let Some(next) = candidates
            .iter()
            .find(|step| step.state == StepState::Ready)
        else {
            return finish_without_ready_steps(&candidates, console);
        };

        if let Some(max_steps) = args.max_steps
            && executed >= max_steps
        {
            console.eprint("指定した工程数に達したため停止します。\n")?;
            return Ok(Outcome::Success);
        }

        // 2 つ目以降の工程は、別の `engine.generate` 呼び出しになるため `StepStarted` の
        // `index` が 1 から数え直される。ここで明示的に区切りを入れないと、前の工程の本文と
        // 混ざって見えてしまう。
        if executed > 0 && !global.quiet {
            boundary.separate(console);
        }
        console.eprint(&format!("=== {} ===\n", next.label))?;
        let started = Instant::now();
        let sink = |event: GenerationEvent| {
            report_generation_event(console, global.quiet, global.verbose, &boundary, &event);
        };

        let changes = match engine.generate(&project, &next.task, &sink, cancel).await {
            Ok(changes) => changes,
            // 中止したことは呼び出し元（`run_cancellable`）が一度だけ知らせる。
            Err(EngineError::Cancelled) => return Ok(Outcome::Cancelled),
            Err(error) => return Err(error.into()),
        };
        apply(&changes, &project, apply_guard, console).await?;
        console.eprint(&format!(
            "--- 完了(所要 {:.1} 秒) ---\n",
            started.elapsed().as_secs_f64()
        ))?;
        executed += 1;
    }
}

/// `until` が指定されていれば、それより後ろの段階の工程を取り除く。
fn candidates_up_to(steps: Vec<PipelineStep>, until: Option<Stage>) -> Vec<PipelineStep> {
    match until {
        Some(stage) => steps
            .into_iter()
            .filter(|step| task_is_within_stage(&step.task, stage))
            .collect(),
        None => steps,
    }
}

/// 変更案を適用する。適用に失敗した（外部での編集と競合したなど）ときは、
/// 生成した内容を失わないよう標準出力に表示してから、エラーとして返す。
async fn apply(
    changes: &ChangeSet,
    project: &Project,
    apply_guard: &ApplyGuard,
    console: &dyn Console,
) -> anyhow::Result<()> {
    super::apply_change_set(changes, project, apply_guard, console).await?;
    super::report_applied_changes(console, changes, "  ")?;
    Ok(())
}

/// ready な工程が無くなったときの締めくくり。
///
/// 候補の工程がすべて完了していれば成功。まだ完了していない工程が `Blocked` のまま
/// 残っているなら、前提を満たせず行き詰まったということなので、理由を示したうえで
/// 失敗として扱う。
fn finish_without_ready_steps(
    candidates: &[PipelineStep],
    console: &dyn Console,
) -> anyhow::Result<Outcome> {
    let blocked: Vec<&PipelineStep> = candidates
        .iter()
        .filter(|step| step.state == StepState::Blocked)
        .collect();
    if blocked.is_empty() {
        console.eprint("すべての工程が完了しました。\n")?;
        return Ok(Outcome::Success);
    }
    console.eprint("これ以上進められる工程がありません。残りの工程:\n")?;
    for step in blocked {
        if let Some(reason) = &step.blocked_by {
            console.eprint(&format!("  ・ {}: {reason}\n", step.label))?;
        }
    }
    anyhow::bail!("行き詰まりました。上に示した理由を解消してから、もう一度実行してください。")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BufferConsole;
    use kataribe_engine::Task;

    fn step(task: Task, state: StepState, blocked_by: Option<&str>) -> PipelineStep {
        let label = format!("{task:?}");
        PipelineStep {
            task,
            label,
            state,
            blocked_by: blocked_by.map(str::to_owned),
        }
    }

    #[test]
    fn finish_without_ready_steps_succeeds_when_nothing_is_blocked() {
        let console = BufferConsole::new();
        let steps = vec![step(Task::Concept, StepState::Done, None)];

        let outcome = finish_without_ready_steps(&steps, &console).unwrap();

        assert_eq!(outcome, Outcome::Success);
        assert!(console.stderr().contains("すべての工程が完了しました"));
    }

    #[test]
    fn finish_without_ready_steps_fails_and_explains_when_something_is_blocked() {
        let console = BufferConsole::new();
        let steps = vec![step(
            Task::Style,
            StepState::Blocked,
            Some("先に企画が必要です。"),
        )];

        let error = finish_without_ready_steps(&steps, &console).unwrap_err();

        assert!(error.to_string().contains("行き詰まりました"));
        assert!(console.stderr().contains("先に企画が必要です。"));
    }

    #[test]
    fn candidates_up_to_excludes_stages_after_until() {
        let steps = vec![
            step(Task::Concept, StepState::Done, None),
            step(Task::Style, StepState::Ready, None),
            step(Task::World, StepState::Ready, None),
        ];

        let candidates = candidates_up_to(steps, Some(Stage::Style));

        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|step| step.task != Task::World));
    }

    #[test]
    fn candidates_up_to_keeps_everything_when_until_is_absent() {
        let steps = vec![
            step(Task::Concept, StepState::Done, None),
            step(Task::World, StepState::Ready, None),
        ];

        let candidates = candidates_up_to(steps, None);

        assert_eq!(candidates.len(), 2);
    }
}
