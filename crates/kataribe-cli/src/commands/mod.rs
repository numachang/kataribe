//! サブコマンドごとの実装。

pub mod api_key;
pub mod export;
pub mod generate;
pub mod models;
pub mod new;
pub mod quality;
pub mod run;
pub mod status;

use anyhow::Context;
use kataribe_engine::LlmSettings;
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
