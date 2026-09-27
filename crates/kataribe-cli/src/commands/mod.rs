//! サブコマンドごとの実装。

pub mod api_key;
pub mod export;
pub mod generate;
pub mod models;
pub mod new;
pub mod project_settings;
pub mod quality;
pub mod run;
pub mod status;

use std::fmt::Write as _;
use std::sync::Arc;

use anyhow::Context;
use kataribe_engine::{ChangeSet, LlmSettings};
use kataribe_llm::ChatModel;
use kataribe_project::Project;

use crate::ApplyGuard;
use crate::output::Console;
use crate::settings;

/// コマンドの実行結果。中止されたかどうかで終了コードを決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Cancelled,
}

/// 変更案を作品フォルダに適用する（`generate` / `run` で共有する）。
///
/// 適用している間は `apply_guard` を保持する。これにより、2 回目の Ctrl+C を受けて
/// 応答しなくなった処理を待たずに終了するときも、適用の途中であれば `main.rs` が
/// 完了を待ってから終了できる（原稿を失わないため）。
///
/// 適用に失敗したときは、失った生成内容が分かるよう変更案を先に表示してから、適用のエラーを
/// 返す。表示（標準出力・標準エラー出力）に失敗しても、本来の失敗理由を隠さないよう無視する。
pub(crate) async fn apply_change_set(
    changes: &ChangeSet,
    project: &Project,
    apply_guard: &ApplyGuard,
    console: &dyn Console,
) -> anyhow::Result<()> {
    let result = {
        let _guard = apply_guard.enter().await;
        changes.apply(project)
    };
    result.map_err(|error| {
        let _ = console.print(&render_change_set(changes));
        let _ = console.eprint("適用に失敗しました。生成した内容を表示します。\n");
        anyhow::Error::from(error).context("変更を作品フォルダに書き込めません")
    })
}

/// 接続設定から生成に使う LLM を作る。`generate` / `run` で共有する。
pub(crate) fn build_chat_model(
    llm: &LlmSettings,
    api_key_env: &str,
) -> anyhow::Result<Arc<dyn ChatModel>> {
    let api_key = api_key_for(llm, api_key_env)?;
    kataribe_engine::build_chat_model(llm, api_key).context("LLM クライアントを初期化できません")
}

/// API キーを使う接続先のときだけ API キーを探す（使わない接続先では資格情報マネージャーに触らない）。
pub(crate) fn api_key_for(llm: &LlmSettings, api_key_env: &str) -> anyhow::Result<Option<String>> {
    if llm.provider.uses_api_key() {
        settings::resolve_api_key(api_key_env)
    } else {
        Ok(None)
    }
}

/// 変更案を人が読める形にする。ファイルごとのパスと、書き込まれるはずの内容を並べる。
///
/// `generate --dry-run` で見せる内容と、適用に失敗したときに失われる内容を表示するのに使う
/// （`generate` / `run` で共有する）。
pub(crate) fn render_change_set(changes: &ChangeSet) -> String {
    let mut rendered = format!("{}\n", changes.summary);
    for file in &changes.files {
        // String への write! は失敗しないため、戻り値は無視してよい。
        let _ = write!(rendered, "\n=== {} ===\n{}\n", file.path, file.content);
    }
    rendered
}
