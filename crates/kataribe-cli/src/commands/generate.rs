//! `generate` サブコマンド: 1 つの工程を生成する。

use std::sync::Arc;

use anyhow::Context;
use kataribe_engine::{ChangeSet, Engine, EngineError, GenerationEvent};
use kataribe_llm::ChatModel;
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::args::{GenerateArgs, GlobalOptions};
use crate::output::{Console, report_generation_event};
use crate::settings;

use super::{Outcome, render_change_set};

pub async fn run(
    args: &GenerateArgs,
    global: &GlobalOptions,
    console: &dyn Console,
    cancel: &CancellationToken,
) -> anyhow::Result<Outcome> {
    let settings = settings::load_effective_settings(global)?;
    let api_key = settings::resolve_api_key(&global.api_key_env)?;
    let client = super::build_llm_client(&settings.llm, api_key)?;
    let model: Arc<dyn ChatModel> = Arc::new(client);
    let engine =
        Engine::new(model, settings.generation).context("執筆エンジンを初期化できません")?;

    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let task = args.task.clone().into_task(args.instruction.clone());

    // --dry-run では、流れてくる生成そのものは見せず、最後に変更案だけをまとめて出す
    // （そうしないと、ストリーミング表示と変更案の表示とで同じ内容が二度出てしまう）。
    let suppress_content = global.quiet || args.dry_run;
    let sink = |event: GenerationEvent| {
        report_generation_event(console, suppress_content, global.verbose, &event);
    };

    match engine.generate(&project, &task, &sink, cancel).await {
        Ok(changes) => apply_or_show(&changes, &project, args.dry_run, console),
        Err(EngineError::Cancelled) => {
            console.eprint("生成を中止しました。\n")?;
            Ok(Outcome::Cancelled)
        }
        Err(error) => Err(error.into()),
    }
}

fn apply_or_show(
    changes: &ChangeSet,
    project: &Project,
    dry_run: bool,
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
    if let Err(error) = changes.apply(project) {
        // 適用に失敗すると、せっかく生成した内容が画面のどこにも残らず消えてしまう。
        // 何を失ったか分かるように、エラーで終わる前に生成結果を出しておく。
        console.eprint("適用に失敗しました。生成した内容を表示します。\n")?;
        console.print(&render_change_set(changes))?;
        return Err(error).context("変更を作品フォルダに書き込めません");
    }
    for file in &changes.files {
        console.eprint(&format!("書き込み: {}\n", file.path))?;
    }
    Ok(Outcome::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BufferConsole;
    use kataribe_engine::{FileChange, NewProject, create_project};
    use kataribe_project::RelPath;

    fn new_project(folder: &std::path::Path) -> Project {
        create_project(
            folder,
            NewProject {
                title: "みさき館の殺人".into(),
                author: None,
                genre: "mystery".into(),
                genre_note: None,
                rating: kataribe_project::Rating::General,
                target_length: 6000,
                idea: "嵐で孤立した洋館で起きる密室殺人。".into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn apply_or_show_prints_the_generated_content_when_apply_fails() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
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
        changes.files.push(FileChange {
            path: RelPath::new("concept.md").unwrap(),
            content: "LLM が生成した企画の内容".into(),
            previous: None,
            base_hash: None,
        });

        let console = BufferConsole::new();
        let error = apply_or_show(&changes, &project, false, &console).unwrap_err();

        assert!(error.to_string().contains("書き込めません"));
        assert!(
            console.stdout().contains("LLM が生成した企画の内容"),
            "生成した内容が表示されるはず: {}",
            console.stdout()
        );
    }
}
