//! `codex exec --json` が 1 行ずつ出す JSON の解釈。

use serde_json::Value;

use crate::error::LlmError;
use crate::event::Usage;

/// JSON Schema を断られたときの `code`。
const INVALID_SCHEMA_CODE: &str = "invalid_json_schema";
/// エラーの説明に載せる行の長さの上限。
const MAX_CLIPPED_CHARS: usize = 200;

/// 1 行が表す出来事のうち、使うもの。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum LineEvent {
    /// モデルの発言。途中の前置きのこともあるので、最後のものを本文にする。
    AgentMessage(String),
    /// 推論の内容。
    Reasoning(String),
    /// 生成の終わり。
    TurnCompleted(Option<Usage>),
    /// 失敗の報告。あとで本文が届いて成功することもあるので、これだけでは失敗にしない。
    Error(Failure),
    /// 生成の失敗。
    TurnFailed(Failure),
    /// 使わない行（開始の通知・コマンド実行の項目など）。
    Ignored,
}

/// Codex が報告した失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Failure {
    /// 利用者に見せる説明。
    pub message: String,
    /// JSON Schema を断られた失敗か（指示文で JSON を求める方法に切り替えられる）。
    pub schema_rejected: bool,
}

/// 1 行を解釈する。JSON として読めない行はエラーにする。
pub(super) fn parse_line(line: &str) -> Result<LineEvent, LlmError> {
    let value: Value = serde_json::from_str(line).map_err(|error| {
        LlmError::Codex(format!(
            "出力を JSON として読めません（{error}）: {}",
            clip(line)
        ))
    })?;
    Ok(match value["type"].as_str() {
        Some("item.completed") => item_event(&value["item"]),
        Some("turn.completed") => LineEvent::TurnCompleted(usage(&value["usage"])),
        Some("error") => LineEvent::Error(failure(value["message"].as_str())),
        Some("turn.failed") => LineEvent::TurnFailed(failure(value["error"]["message"].as_str())),
        _ => LineEvent::Ignored,
    })
}

fn item_event(item: &Value) -> LineEvent {
    let text = item["text"].as_str().unwrap_or_default().to_owned();
    match item["type"].as_str() {
        Some("agent_message") => LineEvent::AgentMessage(text),
        Some("reasoning") if !text.is_empty() => LineEvent::Reasoning(text),
        _ => LineEvent::Ignored,
    }
}

/// `message` は、API のエラーの JSON を文字列にしたものか、ふつうの文章。
fn failure(message: Option<&str>) -> Failure {
    let message = message.unwrap_or("Codex が理由を報告せずに失敗しました。");
    let Ok(body) = serde_json::from_str::<Value>(message) else {
        return Failure {
            message: message.to_owned(),
            schema_rejected: false,
        };
    };
    Failure {
        message: body["error"]["message"]
            .as_str()
            .unwrap_or(message)
            .to_owned(),
        schema_rejected: body["error"]["code"] == INVALID_SCHEMA_CODE,
    }
}

/// `input_tokens` にはキャッシュから読んだ分（`cached_input_tokens`）も含まれ、`output_tokens` には
/// 推論の分も含まれる（実測で、キャッシュの分が入力を超えず、推論の分を足すと出力が合わない）。
fn usage(usage: &Value) -> Option<Usage> {
    Some(Usage {
        prompt_tokens: u32::try_from(usage["input_tokens"].as_u64()?).ok()?,
        completion_tokens: u32::try_from(usage["output_tokens"].as_u64()?).ok()?,
    })
}

fn clip(line: &str) -> String {
    line.chars().take(MAX_CLIPPED_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // 実際の `codex exec --json` の出力から、使う部分を残して短くしたもの
    const AGENT_MESSAGE: &str =
        r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"雨"}}"#;
    const TURN_COMPLETED: &str = r#"{"type":"turn.completed","usage":{"input_tokens":14692,"cached_input_tokens":8320,"cache_write_input_tokens":0,"output_tokens":2589,"reasoning_output_tokens":359}}"#;
    const SCHEMA_ERROR: &str = r#"{"type":"error","message":"{\"type\":\"error\",\"error\":{\"message\":\"Invalid schema for response_format\",\"type\":\"invalid_request_error\",\"param\":\"text.format.schema\",\"code\":\"invalid_json_schema\"},\"status\":400}"}"#;

    #[test]
    fn agent_messages_and_reasoning_are_read() {
        assert_eq!(
            parse_line(AGENT_MESSAGE).unwrap(),
            LineEvent::AgentMessage("雨".to_owned())
        );
        assert_eq!(
            parse_line(r#"{"type":"item.completed","item":{"type":"reasoning","text":"考える"}}"#)
                .unwrap(),
            LineEvent::Reasoning("考える".to_owned())
        );
    }

    #[test]
    fn other_items_and_notifications_are_ignored() {
        for line in [
            r#"{"type":"thread.started","thread_id":"t"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.completed","item":{"type":"error","message":"一時的な失敗"}}"#,
            r#"{"type":"item.completed","item":{"type":"command_execution","command":"ls"}}"#,
        ] {
            assert_eq!(parse_line(line).unwrap(), LineEvent::Ignored, "{line}");
        }
    }

    #[test]
    fn a_completed_turn_reports_input_and_output_tokens_including_cache_and_reasoning() {
        assert_eq!(
            parse_line(TURN_COMPLETED).unwrap(),
            LineEvent::TurnCompleted(Some(Usage {
                prompt_tokens: 14_692,
                completion_tokens: 2589,
            }))
        );
    }

    #[test]
    fn a_rejected_schema_is_recognized_from_the_embedded_json() {
        assert_eq!(
            parse_line(SCHEMA_ERROR).unwrap(),
            LineEvent::Error(Failure {
                message: "Invalid schema for response_format".to_owned(),
                schema_rejected: true,
            })
        );
    }

    #[test]
    fn a_plain_failure_keeps_its_text() {
        let line = r#"{"type":"turn.failed","error":{"message":"usage limit reached"}}"#;

        assert_eq!(
            parse_line(line).unwrap(),
            LineEvent::TurnFailed(Failure {
                message: "usage limit reached".to_owned(),
                schema_rejected: false,
            })
        );
    }

    #[test]
    fn a_line_that_is_not_json_is_an_error() {
        assert!(matches!(
            parse_line("ERROR: something went wrong"),
            Err(LlmError::Codex(_))
        ));
    }
}
