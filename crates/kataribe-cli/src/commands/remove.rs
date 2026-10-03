//! `remove` サブコマンド: 人物・世界観の資料・シーンを、ゴミ箱（`.kataribe/trash/`）へ移して消す。

use crate::ApplyGuard;
use crate::output::Console;
use crate::structure_args::RemoveArgs;

use super::{Outcome, structure_edit};

pub async fn run(
    args: &RemoveArgs,
    console: &dyn Console,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<Outcome> {
    let edit = args.target.clone().into_edit();
    structure_edit::run(&args.folder, &edit, args.dry_run, console, apply_guard).await
}
