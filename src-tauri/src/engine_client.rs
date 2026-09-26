//! 現在の接続設定・API キーから LLM クライアントと執筆エンジンを作る。
//!
//! 設定や API キーが変わっても、呼び出しのたびに現在の値から作り直すだけなので、
//! 古い接続先やキーを使い続けることはない（キャッシュを持たない）。

use std::sync::Arc;

use kataribe_engine::{Engine, GenerationSettings, LlmSettings};
use kataribe_llm::{ClientConfig, ModelInfo, OpenAiCompatClient};

use crate::error::CommandError;

/// 接続設定と API キーから LLM クライアントを作る。
pub fn build_chat_client(
    llm: &LlmSettings,
    api_key: Option<String>,
) -> Result<OpenAiCompatClient, CommandError> {
    let config = ClientConfig {
        base_url: llm.base_url.clone(),
        api_key,
        model: llm.model.clone(),
        ..ClientConfig::default()
    };
    OpenAiCompatClient::new(config).map_err(CommandError::from)
}

/// 接続設定・生成設定・API キーから執筆エンジンを作る。
pub fn build_engine(
    llm: &LlmSettings,
    generation: &GenerationSettings,
    api_key: Option<String>,
) -> Result<Engine, CommandError> {
    let client = build_chat_client(llm, api_key)?;
    Engine::new(Arc::new(client), generation.clone()).map_err(CommandError::from)
}

/// LLM サーバーが公開しているモデルの一覧。
pub async fn list_models(
    llm: &LlmSettings,
    api_key: Option<String>,
) -> Result<Vec<ModelInfo>, CommandError> {
    let client = build_chat_client(llm, api_key)?;
    client.list_models().await.map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;

    #[test]
    fn build_chat_client_rejects_a_base_url_without_a_scheme() {
        let llm = LlmSettings {
            base_url: "localhost:1234/v1".to_owned(),
            model: String::new(),
        };

        let error = build_chat_client(&llm, None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Llm);
    }

    #[test]
    fn build_engine_succeeds_with_a_valid_connection_setting() {
        let llm = LlmSettings::default();
        let generation = GenerationSettings::default();

        assert!(build_engine(&llm, &generation, None).is_ok());
    }
}
