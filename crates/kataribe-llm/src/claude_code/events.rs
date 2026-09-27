//! `claude -p --output-format stream-json` が 1 行ずつ出す JSON の解釈。

use serde_json::Value;

use crate::error::LlmError;
use crate::event::{FinishReason, Usage};

/// stream-json の 1 行が表す出来事のうち、使うもの。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum LineEvent {
    /// 本文の断片。
    Text(String),
    /// 思考の断片。
    Thinking(String),
    /// 生成の終わり（`type: "result"` の行）。
    Result(ResultLine),
    /// 使わない行（初期化・状態の通知・まとめのメッセージなど）。
    Ignored,
}

/// 生成の終わりに届く結果。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResultLine {
    /// Claude Code がエラーで終わったか（未ログイン・利用上限など）。
    pub is_error: bool,
    /// 最終的な応答の全文（JSON Schema を指定したときは、その JSON）。エラーならその説明。
    pub text: String,
    pub reason: FinishReason,
    pub usage: Option<Usage>,
}

/// 1 行を解釈する。JSON として読めない行はエラーにする。
pub(super) fn parse_line(line: &str) -> Result<LineEvent, LlmError> {
    let value: Value = serde_json::from_str(line).map_err(|error| {
        LlmError::ClaudeCode(format!(
            "出力を JSON として読めません（{error}）: {}",
            clip(line)
        ))
    })?;
    Ok(match value["type"].as_str() {
        Some("stream_event") => delta_event(&value["event"]),
        Some("result") => LineEvent::Result(result_line(&value)),
        _ => LineEvent::Ignored,
    })
}

fn delta_event(event: &Value) -> LineEvent {
    if event["type"] != "content_block_delta" {
        return LineEvent::Ignored;
    }
    let delta = &event["delta"];
    match (
        delta["type"].as_str(),
        delta["text"].as_str(),
        delta["thinking"].as_str(),
    ) {
        (Some("text_delta"), Some(text), _) => LineEvent::Text(text.to_owned()),
        (Some("thinking_delta"), _, Some(thinking)) if !thinking.is_empty() => {
            LineEvent::Thinking(thinking.to_owned())
        }
        _ => LineEvent::Ignored,
    }
}

fn result_line(value: &Value) -> ResultLine {
    let subtype = value["subtype"].as_str().unwrap_or_default();
    let is_error = value["is_error"].as_bool().unwrap_or(false) || subtype.starts_with("error");
    ResultLine {
        is_error,
        text: if is_error {
            error_text(value, subtype)
        } else {
            success_text(value)
        },
        reason: finish_reason(value["stop_reason"].as_str()),
        usage: usage(&value["usage"]),
    }
}

/// 応答の全文。JSON Schema を指定したときは `structured_output` の JSON。
fn success_text(value: &Value) -> String {
    match &value["structured_output"] {
        Value::Null => value["result"].as_str().unwrap_or_default().to_owned(),
        structured => structured.to_string(),
    }
}

/// エラーの説明。`result` が無いエラー（`error_max_turns` など）は `errors` に説明が入るので、
/// それも無ければ `subtype` の名前をそのまま使う。
fn error_text(value: &Value, subtype: &str) -> String {
    if let Some(result) = value["result"].as_str().filter(|result| !result.is_empty()) {
        return result.to_owned();
    }
    let errors: Vec<&str> = value["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if errors.is_empty() {
        subtype.to_owned()
    } else {
        errors.join("\n")
    }
}

/// Anthropic の `stop_reason` を、共通の終了理由に変換する。
/// JSON Schema を指定したときは、構造化出力のためのツール呼び出し（`tool_use`）で終わる。
fn finish_reason(stop_reason: Option<&str>) -> FinishReason {
    match stop_reason {
        Some("end_turn" | "stop_sequence" | "tool_use") | None => FinishReason::Stop,
        Some("max_tokens") => FinishReason::Length,
        Some("refusal") => FinishReason::ContentFilter,
        Some(other) => FinishReason::Other(other.to_owned()),
    }
}

/// Claude Code はプロンプトキャッシュを使い、`input_tokens` にはキャッシュに書いた分・読んだ分が
/// 入らないので、それらも足して入力のトークン数にする。
fn usage(usage: &Value) -> Option<Usage> {
    let cached_tokens = ["cache_creation_input_tokens", "cache_read_input_tokens"]
        .iter()
        .filter_map(|key| usage[*key].as_u64())
        .sum::<u64>();
    let prompt_tokens = u32::try_from(usage["input_tokens"].as_u64()? + cached_tokens).ok()?;
    let completion_tokens = u32::try_from(usage["output_tokens"].as_u64()?).ok()?;
    Some(Usage {
        prompt_tokens,
        completion_tokens,
    })
}

/// エラーの説明に載せる、行の先頭部分。
fn clip(line: &str) -> String {
    const MAX_CHARS: usize = 200;
    line.chars().take(MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // 実際の `claude -p` の出力から、使う部分を残して短くしたもの
    const TEXT_DELTA: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"こんにちは"}},"session_id":"s"}"#;
    const THINKING_DELTA: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"考える"}}}"#;
    const EMPTY_THINKING_DELTA: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":""}}}"#;
    const INIT: &str =
        r#"{"type":"system","subtype":"init","cwd":"C:\\tmp","tools":[],"model":"claude-sonnet"}"#;
    const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"stop_reason":"end_turn","result":"こんにちは","usage":{"input_tokens":756,"output_tokens":80}}"#;
    const STRUCTURED_RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"stop_reason":"tool_use","result":"{\"name\":\"凛\"}","structured_output":{"name":"凛"},"usage":{"input_tokens":10,"output_tokens":5}}"#;
    const ERROR_RESULT: &str = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login","usage":{"input_tokens":0,"output_tokens":0}}"#;

    #[test]
    fn text_and_thinking_deltas_become_fragments() {
        assert_eq!(
            parse_line(TEXT_DELTA).unwrap(),
            LineEvent::Text("こんにちは".to_owned())
        );
        assert_eq!(
            parse_line(THINKING_DELTA).unwrap(),
            LineEvent::Thinking("考える".to_owned())
        );
    }

    #[test]
    fn empty_thinking_and_other_lines_are_ignored() {
        assert_eq!(
            parse_line(EMPTY_THINKING_DELTA).unwrap(),
            LineEvent::Ignored
        );
        assert_eq!(parse_line(INIT).unwrap(), LineEvent::Ignored);
    }

    #[test]
    fn result_carries_the_full_text_reason_and_usage() {
        assert_eq!(
            parse_line(RESULT).unwrap(),
            LineEvent::Result(ResultLine {
                is_error: false,
                text: "こんにちは".to_owned(),
                reason: FinishReason::Stop,
                usage: Some(Usage {
                    prompt_tokens: 756,
                    completion_tokens: 80,
                }),
            })
        );
    }

    #[test]
    fn an_error_without_a_result_is_explained_by_its_errors() {
        let line = r#"{"type":"result","subtype":"error_max_structured_output_retries","is_error":true,"errors":["出力が JSON Schema に合いません","再試行の上限に達しました"],"usage":{"input_tokens":1,"output_tokens":1}}"#;

        let LineEvent::Result(result) = parse_line(line).unwrap() else {
            panic!("結果の行のはず");
        };

        assert!(result.is_error);
        assert_eq!(
            result.text,
            "出力が JSON Schema に合いません\n再試行の上限に達しました"
        );
    }

    #[test]
    fn an_error_subtype_is_an_error_even_without_the_flag() {
        let line = r#"{"type":"result","subtype":"error_during_execution"}"#;

        let LineEvent::Result(result) = parse_line(line).unwrap() else {
            panic!("結果の行のはず");
        };

        assert!(result.is_error);
        assert_eq!(result.text, "error_during_execution");
    }

    #[test]
    fn a_successful_result_without_text_is_empty() {
        let line = r#"{"type":"result","subtype":"success","is_error":false}"#;

        let LineEvent::Result(result) = parse_line(line).unwrap() else {
            panic!("結果の行のはず");
        };

        assert!(!result.is_error);
        assert_eq!(result.text, "");
    }

    #[test]
    fn cached_input_tokens_are_counted_as_input() {
        let line = r#"{"type":"result","is_error":false,"stop_reason":"end_turn","result":"雨","usage":{"input_tokens":10,"cache_creation_input_tokens":2000,"cache_read_input_tokens":6000,"output_tokens":80}}"#;

        let LineEvent::Result(result) = parse_line(line).unwrap() else {
            panic!("結果の行のはず");
        };

        assert_eq!(
            result.usage,
            Some(Usage {
                prompt_tokens: 8010,
                completion_tokens: 80,
            })
        );
    }

    #[test]
    fn structured_output_is_returned_as_json_text() {
        let LineEvent::Result(result) = parse_line(STRUCTURED_RESULT).unwrap() else {
            panic!("結果の行のはず");
        };
        assert_eq!(result.text, r#"{"name":"凛"}"#);
        assert_eq!(result.reason, FinishReason::Stop);
    }

    #[test]
    fn error_result_is_marked_as_an_error() {
        let LineEvent::Result(result) = parse_line(ERROR_RESULT).unwrap() else {
            panic!("結果の行のはず");
        };
        assert!(result.is_error);
        assert_eq!(result.text, "Not logged in · Please run /login");
    }

    #[test]
    fn max_tokens_means_the_output_was_cut() {
        assert_eq!(finish_reason(Some("max_tokens")), FinishReason::Length);
    }

    #[test]
    fn a_line_that_is_not_json_is_an_error() {
        assert!(matches!(
            parse_line("Error: something went wrong"),
            Err(LlmError::ClaudeCode(_))
        ));
    }
}
