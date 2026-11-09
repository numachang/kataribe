//! `import` サブコマンド: 手元のテキスト原稿を、1 つの章として末尾に取り込む。

use anyhow::Context;
use kataribe_engine::{ManuscriptImport, plan_manuscript_import};
use kataribe_project::Project;

use crate::ApplyGuard;
use crate::output::Console;
use crate::structure_args::ImportArgs;

use super::{Outcome, structure_edit};

pub async fn run(
    args: &ImportArgs,
    console: &dyn Console,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<Outcome> {
    let text = std::fs::read_to_string(&args.from)
        .with_context(|| format!("原稿を読めません: {}", args.from.display()))?;
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let plan = plan_manuscript_import(
        &project,
        &ManuscriptImport {
            chapter_title: args.chapter_title.clone(),
            text,
            min_scene_chars: args.min_scene_chars,
        },
    )?;
    structure_edit::show_and_apply(&project, &plan, args.dry_run, console, apply_guard).await
}
