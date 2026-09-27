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

/// コマンドの実行結果。中止されたかどうかで終了コードを決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Cancelled,
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
