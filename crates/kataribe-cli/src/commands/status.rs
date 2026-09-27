//! `status` サブコマンド: 工程の状態と文字数を表示する。

use std::fmt::Write as _;

use anyhow::Context;
use kataribe_engine::{PipelineStep, StepState, overview, pipeline};
use kataribe_project::Project;
use serde_json::json;

use crate::args::{GlobalOptions, StatusArgs};
use crate::output::Console;
use crate::settings;

use super::Outcome;

pub fn run(
    args: &StatusArgs,
    global: &GlobalOptions,
    console: &dyn Console,
) -> anyhow::Result<Outcome> {
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let settings = settings::load_effective_settings(global, Some(&project))?;
    let steps =
        pipeline(&project, settings.generation.draft_unit).context("工程の一覧を取得できません")?;
    let overview = overview(&project).context("作品の一覧を取得できません")?;

    if args.json {
        let payload = json!({ "pipeline": steps, "overview": overview });
        console
            .print(&serde_json::to_string_pretty(&payload)?)
            .context("標準出力への書き込みに失敗しました")?;
        console
            .print("\n")
            .context("標準出力への書き込みに失敗しました")?;
    } else {
        console
            .print(&render_steps(&steps))
            .context("標準出力への書き込みに失敗しました")?;
        console
            .print(&format!(
                "\n本文: {} / {} 文字\n",
                overview.total_chars, overview.target_length
            ))
            .context("標準出力への書き込みに失敗しました")?;
    }
    Ok(Outcome::Success)
}

fn render_steps(steps: &[PipelineStep]) -> String {
    let mut rendered = String::new();
    for step in steps {
        let mark = match step.state {
            StepState::Done => "✓ 済",
            StepState::Ready => "▶ 可",
            StepState::Blocked => "・ 待ち",
        };
        // String への write! は失敗しないため、戻り値は無視してよい。
        let _ = write!(rendered, "{mark}  {}", step.label);
        if let Some(reason) = &step.blocked_by {
            let _ = write!(rendered, "（{reason}）");
        }
        rendered.push('\n');
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_engine::Task;

    fn step(task: Task, label: &str, state: StepState, blocked_by: Option<&str>) -> PipelineStep {
        PipelineStep {
            task,
            label: label.to_owned(),
            state,
            blocked_by: blocked_by.map(str::to_owned),
        }
    }

    #[test]
    fn render_steps_shows_a_mark_and_reason_per_state() {
        let steps = vec![
            step(Task::Concept, "企画", StepState::Done, None),
            step(Task::Style, "文体ガイド", StepState::Ready, None),
            step(
                Task::World,
                "世界観",
                StepState::Blocked,
                Some("先に企画が必要です。"),
            ),
        ];

        let rendered = render_steps(&steps);

        assert!(rendered.contains("✓ 済  企画"));
        assert!(rendered.contains("▶ 可  文体ガイド"));
        assert!(rendered.contains("・ 待ち  世界観（先に企画が必要です。）"));
    }

    #[test]
    fn run_reports_a_failure_when_stdout_cannot_be_written_to() {
        use crate::output::testing::FailingConsole;
        use crate::test_support::new_test_project;

        let folder = tempfile::tempdir().unwrap();
        new_test_project(folder.path());
        let args = StatusArgs {
            folder: folder.path().to_path_buf(),
            json: false,
        };

        let error = run(&args, &GlobalOptions::default(), &FailingConsole).unwrap_err();

        assert!(error.to_string().contains("標準出力"));
    }
}
