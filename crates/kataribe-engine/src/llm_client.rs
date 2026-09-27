//! 接続設定から LLM を作る。GUI と CLI が同じ組み立て方を使う。

use std::sync::Arc;

use kataribe_llm::{
    ChatModel, ClaudeCodeConfig, ClaudeCodeModel, ClientConfig, ModelInfo, OpenAiCompatClient,
};

use crate::error::Result;
use crate::settings::{LlmProvider, LlmSettings};

/// 接続設定と API キーから、生成に使う LLM を作る。API キーは OpenAI 互換 API のときだけ使う。
pub fn build_chat_model(llm: &LlmSettings, api_key: Option<String>) -> Result<Arc<dyn ChatModel>> {
    Ok(match llm.provider {
        LlmProvider::OpenaiCompatible => Arc::new(openai_compatible_client(llm, api_key)?),
        LlmProvider::ClaudeCode => Arc::new(ClaudeCodeModel::new(claude_code_config(llm))),
    })
}

/// 選べるモデルの一覧。OpenAI 互換 API ならサーバーに問い合わせ、Claude Code ならログインしているかを
/// 確かめてからモデルの別名を返す。接続テストを兼ねる。
pub async fn list_models(llm: &LlmSettings, api_key: Option<String>) -> Result<Vec<ModelInfo>> {
    Ok(match llm.provider {
        LlmProvider::OpenaiCompatible => {
            openai_compatible_client(llm, api_key)?
                .list_models()
                .await?
        }
        LlmProvider::ClaudeCode => {
            ClaudeCodeModel::new(claude_code_config(llm))
                .list_models()
                .await?
        }
    })
}

fn openai_compatible_client(
    llm: &LlmSettings,
    api_key: Option<String>,
) -> Result<OpenAiCompatClient> {
    Ok(OpenAiCompatClient::new(ClientConfig {
        base_url: llm.base_url.clone(),
        api_key,
        model: llm.model.clone(),
        ..ClientConfig::default()
    })?)
}

/// 空欄の項目は Claude Code の既定（`claude` コマンドと `sonnet`）にする。
fn claude_code_config(llm: &LlmSettings) -> ClaudeCodeConfig {
    let defaults = ClaudeCodeConfig::default();
    let or_default = |value: &str, default: &str| {
        let value = value.trim();
        if value.is_empty() { default } else { value }.to_owned()
    };
    ClaudeCodeConfig {
        program: or_default(&llm.claude_command, &defaults.program),
        model: or_default(&llm.claude_model, &defaults.model),
        ..defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn claude_code(model: &str, command: &str) -> LlmSettings {
        LlmSettings {
            provider: LlmProvider::ClaudeCode,
            claude_model: model.to_owned(),
            claude_command: command.to_owned(),
            ..LlmSettings::default()
        }
    }

    #[test]
    fn claude_code_uses_its_defaults_for_blank_fields() {
        assert_eq!(
            claude_code_config(&claude_code("", " ")),
            ClaudeCodeConfig::default()
        );
    }

    #[test]
    fn claude_code_uses_its_own_model_and_command() {
        let llm = LlmSettings {
            model: "gemma-3-27b".to_owned(),
            ..claude_code("opus", "C:/tools/claude.exe")
        };
        assert_eq!(
            claude_code_config(&llm),
            ClaudeCodeConfig {
                program: "C:/tools/claude.exe".to_owned(),
                model: "opus".to_owned(),
                ..ClaudeCodeConfig::default()
            }
        );
    }

    #[tokio::test]
    async fn listing_claude_code_models_runs_the_configured_command() {
        let llm = claude_code("", "C:/no/such/folder/claude.exe");

        let error = list_models(&llm, None).await.unwrap_err();

        assert!(
            error.to_string().contains("C:/no/such/folder/claude.exe"),
            "{error}"
        );
    }

    #[test]
    fn an_invalid_server_address_is_rejected_for_openai_compatible_apis() {
        let llm = LlmSettings {
            base_url: "localhost:1234/v1".to_owned(),
            ..LlmSettings::default()
        };
        assert!(build_chat_model(&llm, None).is_err());
    }

    #[test]
    fn claude_code_needs_no_api_key_or_server_address() {
        let llm = LlmSettings {
            base_url: String::new(),
            ..claude_code("", "")
        };
        assert!(build_chat_model(&llm, None).is_ok());
    }
}
