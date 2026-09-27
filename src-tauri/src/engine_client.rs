//! 現在の接続設定・API キーから LLM クライアントと執筆エンジンを作る。
//!
//! 設定や API キーが変わっても、呼び出しのたびに現在の値から作り直すだけなので、
//! 古い接続先やキーを使い続けることはない（キャッシュを持たない）。

use kataribe_engine::{ApiKeyStore, Engine, GenerationSettings, LlmSettings};
use kataribe_llm::ModelInfo;

use crate::error::CommandError;

/// 接続先が API キーを使うときだけ、保存済みの API キーを読む（使わないときは資格情報ストアに触らない）。
pub fn load_api_key(
    store: &ApiKeyStore,
    llm: &LlmSettings,
) -> Result<Option<String>, CommandError> {
    if llm.provider.uses_api_key() {
        store.load().map_err(CommandError::from)
    } else {
        Ok(None)
    }
}

/// 接続設定・生成設定・API キーから執筆エンジンを作る。
pub fn build_engine(
    llm: &LlmSettings,
    generation: &GenerationSettings,
    api_key: Option<String>,
) -> Result<Engine, CommandError> {
    let model = kataribe_engine::build_chat_model(llm, api_key)?;
    Engine::new(model, generation.clone()).map_err(CommandError::from)
}

/// 選べるモデルの一覧。
pub async fn list_models(
    llm: &LlmSettings,
    api_key: Option<String>,
) -> Result<Vec<ModelInfo>, CommandError> {
    kataribe_engine::list_models(llm, api_key)
        .await
        .map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;

    #[test]
    fn build_engine_rejects_a_base_url_without_a_scheme() {
        let llm = LlmSettings {
            base_url: "localhost:1234/v1".to_owned(),
            ..LlmSettings::default()
        };

        let error = build_engine(&llm, &GenerationSettings::default(), None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Llm);
    }

    #[test]
    fn build_engine_succeeds_with_a_valid_connection_setting() {
        let llm = LlmSettings::default();
        let generation = GenerationSettings::default();

        assert!(build_engine(&llm, &generation, None).is_ok());
    }
}
