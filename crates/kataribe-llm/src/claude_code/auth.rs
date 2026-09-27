//! `claude auth status` で、Claude Code にログインしているかを確かめる（利用枠は使わない）。

use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;

use super::{claude_command, spawn_error};
use crate::error::LlmError;

/// `claude auth status` は手元の設定を読むだけなので、これより長くかかるなら固まっているとみなす。
const STATUS_TIMEOUT: Duration = Duration::from_secs(30);

const NOT_LOGGED_IN: &str = "Claude Code にログインしていません。ターミナルで claude を起動し、/login でログインしてください。";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    logged_in: bool,
}

/// `program` の Claude Code にログインしていれば `Ok`。
pub(super) async fn ensure_logged_in(program: &str) -> Result<(), LlmError> {
    let mut command = claude_command(program);
    command
        .args(["auth", "status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = tokio::time::timeout(STATUS_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            LlmError::ClaudeCode("claude コマンドがログイン状態の確認に応答しません。".to_owned())
        })?
        .map_err(|error| spawn_error(program, &error))?;
    // ログインしていないときも JSON は出るが、終了コードは 0 でないことがあるので、先に中身を見る
    let stdout = String::from_utf8_lossy(&output.stdout);
    match parse_logged_in(&stdout) {
        Some(true) => Ok(()),
        Some(false) => Err(LlmError::ClaudeCode(NOT_LOGGED_IN.to_owned())),
        None => Err(unreadable_status(&output)),
    }
}

/// `claude auth status --json` の出力からログインしているかを読む。読めなければ `None`。
fn parse_logged_in(stdout: &str) -> Option<bool> {
    serde_json::from_str::<AuthStatus>(stdout.trim())
        .ok()
        .map(|status| status.logged_in)
}

fn unreadable_status(output: &std::process::Output) -> LlmError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    LlmError::ClaudeCode(if detail.is_empty() {
        "claude コマンドからログイン状態を読めません。".to_owned()
    } else {
        format!("claude コマンドからログイン状態を読めません: {detail}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logged_in_status_is_read() {
        let stdout = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}"#;
        assert_eq!(parse_logged_in(stdout), Some(true));
    }

    #[test]
    fn a_logged_out_status_is_read() {
        assert_eq!(parse_logged_in("{\"loggedIn\":false}\n"), Some(false));
    }

    #[test]
    fn output_that_is_not_a_status_is_unreadable() {
        assert_eq!(parse_logged_in("error: unknown command 'auth'"), None);
        assert_eq!(parse_logged_in(r#"{"authMethod":"none"}"#), None);
    }
}
