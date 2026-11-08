//! 接続設定から LLM を作る。GUI と CLI が同じ組み立て方を使う。

use std::sync::Arc;

use kataribe_llm::{
    ChatModel, ClaudeCodeConfig, ClaudeCodeModel, ClientConfig, CodexConfig, CodexModel, ModelInfo,
    OpenAiCompatClient,
};

use crate::error::Result;
use crate::settings::{LlmProvider, LlmSettings};

/// 接続設定と API キーから、生成に使う LLM を作る。API キーは OpenAI 互換 API のときだけ使う。
pub fn build_chat_model(llm: &LlmSettings, api_key: Option<String>) -> Result<Arc<dyn ChatModel>> {
    Ok(match llm.provider {
        LlmProvider::OpenaiCompatible => Arc::new(openai_compatible_client(llm, api_key)?),
        LlmProvider::ClaudeCode => Arc::new(ClaudeCodeModel::new(claude_code_config(llm))),
        LlmProvider::Codex => Arc::new(CodexModel::new(codex_config(llm))),
    })
}

/// 選べるモデルの一覧。OpenAI 互換 API ならサーバーに問い合わせ、Claude Code と Codex ならログインしているかを
/// 確かめてからモデルを返す。接続テストを兼ねる。
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
        LlmProvider::Codex => CodexModel::new(codex_config(llm)).list_models().await?,
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

fn or_default(value: &str, default: &str) -> String {
    let value = value.trim();
    if value.is_empty() { default } else { value }.to_owned()
}

/// 空欄の項目は Claude Code の既定（`claude` コマンドと `sonnet`）にする。
fn claude_code_config(llm: &LlmSettings) -> ClaudeCodeConfig {
    let defaults = ClaudeCodeConfig::default();
    ClaudeCodeConfig {
        program: or_default(&llm.claude_command, &defaults.program),
        model: or_default(&llm.claude_model, &defaults.model),
        ..defaults
    }
}

/// 空欄のコマンドは `codex` に、空欄のモデルと推論の強さは指定なし（Codex の既定）にする。
fn codex_config(llm: &LlmSettings) -> CodexConfig {
    let defaults = CodexConfig::default();
    CodexConfig {
        program: or_default(&llm.codex_command, &defaults.program),
        model: llm.codex_model.trim().to_owned(),
        reasoning_effort: llm.codex_reasoning_effort.trim().to_owned(),
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

    fn codex(model: &str, command: &str, effort: &str) -> LlmSettings {
        LlmSettings {
            provider: LlmProvider::Codex,
            codex_model: model.to_owned(),
            codex_command: command.to_owned(),
            codex_reasoning_effort: effort.to_owned(),
            ..LlmSettings::default()
        }
    }

    #[test]
    fn codex_leaves_blank_model_and_effort_to_codex() {
        assert_eq!(codex_config(&codex(" ", "", " ")), CodexConfig::default());
    }

    #[test]
    fn codex_uses_its_own_command_model_and_effort() {
        let llm = LlmSettings {
            claude_model: "opus".to_owned(),
            ..codex("gpt-6.1-sol", "C:/tools/codex.exe", "high")
        };
        assert_eq!(
            codex_config(&llm),
            CodexConfig {
                program: "C:/tools/codex.exe".to_owned(),
                model: "gpt-6.1-sol".to_owned(),
                reasoning_effort: "high".to_owned(),
                ..CodexConfig::default()
            }
        );
    }

    #[tokio::test]
    async fn listing_codex_models_runs_the_configured_command() {
        let llm = codex("", "C:/no/such/folder/codex.exe", "");

        let error = list_models(&llm, None).await.unwrap_err();

        assert!(
            error.to_string().contains("C:/no/such/folder/codex.exe"),
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
