//! `run` サブコマンド: 取りかかれる工程(ready)を順に生成・適用し続ける。

use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use kataribe_engine::{
    ChangeSet, Engine, EngineError, GenerationEvent, PipelineStep, StepState, pipeline,
};
use kataribe_llm::ChatModel;
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::args::{GlobalOptions, RunArgs};
use crate::output::{Console, report_generation_event};
use crate::settings;
use crate::stage::reached_stage;

use super::Outcome;

pub async fn run(
    args: &RunArgs,
    global: &GlobalOptions,
    console: &dyn Console,
    cancel: &CancellationToken,
) -> anyhow::Result<Outcome> {
    let settings = settings::load_effective_settings(global)?;
    let unit = settings.generation.draft_unit;
    let api_key = settings::resolve_api_key(&global.api_key_env)?;
    let client = super::build_llm_client(&settings.llm, api_key)?;
    let model: Arc<dyn ChatModel> = Arc::new(client);
    let engine =
        Engine::new(model, settings.generation).context("執筆エンジンを初期化できません")?;

    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;

    let mut executed = 0u32;
    loop {
        let steps = pipeline(&project, unit).context("工程の一覧を取得できません")?;

        if let Some(stage) = args.until
            && reached_stage(&steps, stage)
        {
            console.eprint("指定した段階まで完了しました。\n");
            return Ok(Outcome::Success);
        }

        let Some(next) = steps.iter().find(|step| step.state == StepState::Ready) else {
            return finish_without_ready_steps(&steps, console);
        };

        if let Some(max_steps) = args.max_steps
            && executed >= max_steps
        {
            console.eprint("指定した工程数に達したため停止します。\n");
            return Ok(Outcome::Success);
        }

        console.eprint(&format!("=== {} ===\n", next.label));
        let started = Instant::now();
        let sink = |event: GenerationEvent| {
            report_generation_event(console, global.quiet, global.verbose, &event);
        };

        let changes = match engine.generate(&project, &next.task, &sink, cancel).await {
            Ok(changes) => changes,
            Err(EngineError::Cancelled) => {
                console.eprint("生成を中止しました。\n");
                return Ok(Outcome::Cancelled);
            }
            Err(error) => return Err(error.into()),
        };
        apply(&changes, &project, console)?;
        console.eprint(&format!(
            "--- 完了(所要 {:.1} 秒) ---\n",
            started.elapsed().as_secs_f64()
        ));
        executed += 1;
    }
}

fn apply(changes: &ChangeSet, project: &Project, console: &dyn Console) -> anyhow::Result<()> {
    changes
        .apply(project)
        .context("変更を作品フォルダに書き込めません")?;
    for file in &changes.files {
        console.eprint(&format!("  書き込み: {}\n", file.path));
    }
    Ok(())
}

/// ready な工程が無くなったときの締めくくり。
///
/// 全工程が完了していれば成功。まだ完了していない工程が `Blocked` のまま残っているなら、
/// 前提を満たせず行き詰まったということなので、理由を示したうえで失敗として扱う。
fn finish_without_ready_steps(
    steps: &[PipelineStep],
    console: &dyn Console,
) -> anyhow::Result<Outcome> {
    let blocked: Vec<&PipelineStep> = steps
        .iter()
        .filter(|step| step.state == StepState::Blocked)
        .collect();
    if blocked.is_empty() {
        console.eprint("すべての工程が完了しました。\n");
        return Ok(Outcome::Success);
    }
    console.eprint("これ以上進められる工程がありません。残りの工程:\n");
    for step in blocked {
        if let Some(reason) = &step.blocked_by {
            console.eprint(&format!("  ・ {}: {reason}\n", step.label));
        }
    }
    anyhow::bail!("行き詰まりました。上に示した理由を解消してから、もう一度実行してください。")
}
