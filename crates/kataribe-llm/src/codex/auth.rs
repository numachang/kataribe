//! `codex login status` でログインを確かめ、`codex debug models` で選べるモデルを読む（利用枠は使わない）。

use std::process::{Output, Stdio};
use std::time::Duration;

use serde::Deserialize;

use super::TOOL;
use crate::cli_process::command_for;
use crate::error::LlmError;
use crate::model_info::ModelInfo;

/// どちらのコマンドも手元の情報を読むだけなので、これより長くかかるなら固まっているとみなす。
const STATUS_TIMEOUT: Duration = Duration::from_secs(30);
/// 利用者が選べるモデルの印（これ以外は内部用）。
const LISTED_VISIBILITY: &str = "list";

const NOT_LOGGED_IN: &str =
    "Codex にログインしていません。ターミナルで codex login を実行してください。";

/// `program` の Codex にログインしていれば `Ok`。
pub(super) async fn ensure_logged_in(program: &str) -> Result<(), LlmError> {
    let output = run(program, &["login", "status"], "ログイン状態の確認").await?;
    if output.status.success() {
        return Ok(());
    }
    // ログインしていないときは終了コードが 1 で、エラー出力に「Not logged in」と出る（実測）
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("Not logged in") {
        return Err(LlmError::Codex(NOT_LOGGED_IN.to_owned()));
    }
    Err(LlmError::Codex(format!(
        "codex コマンドからログイン状態を読めません: {}",
        stderr.trim()
    )))
}

/// 利用者が選べるモデルの一覧。
pub(super) async fn available_models(program: &str) -> Result<Vec<ModelInfo>, LlmError> {
    let output = run(program, &["debug", "models"], "モデルの一覧の取得").await?;
    parse_models(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        LlmError::Codex(format!(
            "codex コマンドからモデルの一覧を読めません: {}",
            stderr.trim()
        ))
    })
}

async fn run(program: &str, args: &[&str], what: &str) -> Result<Output, LlmError> {
    let mut command = command_for(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    tokio::time::timeout(STATUS_TIMEOUT, command.output())
        .await
        .map_err(|_| LlmError::Codex(format!("codex コマンドが{what}に応答しません。")))?
        .map_err(|error| TOOL.spawn_error(program, &error))
}

#[derive(Deserialize)]
struct ModelCatalog {
    models: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct CatalogEntry {
    slug: String,
    visibility: String,
    context_window: Option<u32>,
}

/// `codex debug models` の出力から、選べるモデルを読む。読めなければ `None`。
fn parse_models(stdout: &str) -> Option<Vec<ModelInfo>> {
    let catalog: ModelCatalog = serde_json::from_str(stdout.trim()).ok()?;
    Some(
        catalog
            .models
            .into_iter()
            .filter(|entry| entry.visibility == LISTED_VISIBILITY)
            .map(|entry| ModelInfo {
                id: entry.slug,
                context_length: entry.context_window,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn only_the_listed_models_are_offered_with_their_context_length() {
        let stdout = r#"{"models":[
            {"slug":"gpt-6.1-sol","visibility":"list","context_window":500000},
            {"slug":"gpt-reserve","visibility":"hide","context_window":272000}
        ]}"#;

        assert_eq!(
            parse_models(stdout),
            Some(vec![ModelInfo {
                id: "gpt-6.1-sol".to_owned(),
                context_length: Some(500_000),
            }])
        );
    }

    #[test]
    fn output_that_is_not_a_catalog_is_unreadable() {
        assert_eq!(parse_models("error: unknown command"), None);
    }
}
