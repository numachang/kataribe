//! `move` サブコマンド: 人物・章・シーンの順番を変える。

use crate::ApplyGuard;
use crate::output::Console;
use crate::structure_args::MoveArgs;

use super::{Outcome, structure_edit};

pub async fn run(
    args: &MoveArgs,
    console: &dyn Console,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<Outcome> {
    // 画面の位置は 1 始まり。エンジンは 0 始まり
    let position = args.to.get() - 1;
    let edit = args.target.clone().into_edit(position);
    structure_edit::run(&args.folder, &edit, args.dry_run, console, apply_guard).await
}
