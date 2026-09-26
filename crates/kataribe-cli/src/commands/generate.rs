//! `generate` サブコマンド: 1 つの工程を生成する。

use std::fmt::Write as _;
use std::sync::Arc;

use anyhow::Context;
use kataribe_engine::{ChangeSet, Engine, EngineError, GenerationEvent};
use kataribe_llm::ChatModel;
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::args::{GenerateArgs, GlobalOptions};
use crate::output::{Console, report_generation_event};
use crate::settings;

use super::Outcome;

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

    let sink = |event: GenerationEvent| {
        report_generation_event(console, global.quiet, global.verbose, &event);
    };

    match engine.generate(&project, &task, &sink, cancel).await {
        Ok(changes) => apply_or_show(&changes, &project, args.dry_run, console),
        Err(EngineError::Cancelled) => {
            console.eprint("生成を中止しました。\n");
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
        console.eprint("変更はありませんでした。\n");
        return Ok(Outcome::Success);
    }
    if dry_run {
        console.print(&render_change_set(changes));
        return Ok(Outcome::Success);
    }
    changes
        .apply(project)
        .context("変更を作品フォルダに書き込めません")?;
    for file in &changes.files {
        console.eprint(&format!("書き込み: {}\n", file.path));
    }
    Ok(Outcome::Success)
}

/// ドライラン時に見せる変更案。ファイルごとのパスと、書き込まれるはずの内容を並べる。
fn render_change_set(changes: &ChangeSet) -> String {
    let mut rendered = format!("{}\n", changes.summary);
    for file in &changes.files {
        // String への write! は失敗しないため、戻り値は無視してよい。
        let _ = write!(rendered, "\n=== {} ===\n{}\n", file.path, file.content);
    }
    rendered
}
