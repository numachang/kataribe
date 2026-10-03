//! `generate` サブコマンド: 1 つの工程を生成する。

use anyhow::Context;
use kataribe_engine::{ChangeSet, Engine, EngineError, GenerationEvent};
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::ApplyGuard;
use crate::args::{GenerateArgs, GlobalOptions};
use crate::output::{Console, StdoutBoundary, report_generation_event};
use crate::settings;

use super::{Outcome, render_change_set};

pub async fn run(
    args: &GenerateArgs,
    global: &GlobalOptions,
    console: &dyn Console,
    cancel: &CancellationToken,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<Outcome> {
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let settings = settings::load_effective_settings(global, Some(&project))?;
    let model = super::build_chat_model(&settings.llm, &global.api_key_env)?;
    let engine =
        Engine::new(model, settings.generation).context("執筆エンジンを初期化できません")?;

    let task = args.task.clone().into_task(args.instruction.clone());

    // --dry-run では、流れてくる生成そのものは見せず、最後に変更案だけをまとめて出す
    // （そうしないと、ストリーミング表示と変更案の表示とで同じ内容が二度出てしまう）。
    let suppress_content = global.quiet || args.dry_run;
    let boundary = StdoutBoundary::new();
    let sink = |event: GenerationEvent| {
        report_generation_event(console, suppress_content, global.verbose, &boundary, &event);
    };

    match engine.generate(&project, &task, &sink, cancel).await {
        Ok(changes) => apply_or_show(&changes, &project, args.dry_run, apply_guard, console).await,
        // 中止したことは呼び出し元（`run_cancellable`）が一度だけ知らせるので、ここでは
        // 何も出力せず、中止した結果だけを伝える。
        Err(EngineError::Cancelled) => Ok(Outcome::Cancelled),
        Err(error) => Err(error.into()),
    }
}

async fn apply_or_show(
    changes: &ChangeSet,
    project: &Project,
    dry_run: bool,
    apply_guard: &ApplyGuard,
    console: &dyn Console,
) -> anyhow::Result<Outcome> {
    if changes.is_empty() {
        console.eprint("変更はありませんでした。\n")?;
        return Ok(Outcome::Success);
    }
    if dry_run {
        console.print(&render_change_set(changes))?;
        return Ok(Outcome::Success);
    }
    super::apply_change_set(changes, project, apply_guard, console).await?;
    super::report_applied_changes(console, changes, "")?;
    Ok(Outcome::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BufferConsole;
    use crate::test_support::new_test_project;
    use kataribe_engine::FileChange;
    use kataribe_project::RelPath;

    #[tokio::test]
    async fn apply_or_show_prints_the_generated_content_when_apply_fails() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_test_project(folder.path());
        // わざと外部で先に concept.md を作っておき、`base_hash` が無い（新規のはず）
        // 変更案の適用を Conflict で失敗させる。
        project
            .store()
            .write_text(
                &RelPath::new("concept.md").unwrap(),
                "先に誰かが書いた内容",
                kataribe_project::WriteOptions::default(),
            )
            .unwrap();

        let mut changes = ChangeSet::new("企画を生成しました。").made_for(&project);
        changes.files.push(FileChange::Write {
            path: RelPath::new("concept.md").unwrap(),
            content: "LLM が生成した企画の内容".into(),
            previous: None,
            base_hash: None,
        });

        let console = BufferConsole::new();
        let apply_guard = ApplyGuard::new();
        let error = apply_or_show(&changes, &project, false, &apply_guard, &console)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("書き込めません"));
        assert!(
            console.stdout().contains("LLM が生成した企画の内容"),
            "生成した内容が表示されるはず: {}",
            console.stdout()
        );
    }
}
