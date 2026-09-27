//! サブコマンドごとの実装。

pub mod api_key;
pub mod export;
pub mod generate;
pub mod models;
pub mod new;
pub mod quality;
pub mod run;
pub mod status;

use std::fmt::Write as _;

use anyhow::Context;
use kataribe_engine::{ChangeSet, LlmSettings};
use kataribe_llm::{ClientConfig, OpenAiCompatClient};
use kataribe_project::Project;

use crate::ApplyGuard;
use crate::output::Console;

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

/// LLM サーバーへ接続するクライアントを組み立てる。`generate` / `run` / `models` で共有する。
pub(crate) fn build_llm_client(
    llm: &LlmSettings,
    api_key: Option<String>,
) -> anyhow::Result<OpenAiCompatClient> {
    let config = ClientConfig {
        base_url: llm.base_url.clone(),
        api_key,
        model: llm.model.clone(),
        ..ClientConfig::default()
    };
    OpenAiCompatClient::new(config).context("LLM クライアントを初期化できません")
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
