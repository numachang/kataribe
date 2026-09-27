//! `OpenAI` 互換のストリーミングチャンク（SSE の `data:` ペイロード 1 件分の JSON）を解釈する。
//!
//! HTTP から独立させてあるので、実際の通信なしに単体テストできる。

use serde::Deserialize;

use crate::error::LlmError;
use crate::event::{ChatEvent, FinishReason, Usage};
use crate::think_filter::ThinkTagFilter;

#[derive(Debug, Default, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<ChunkUsage>,
}

#[derive(Debug, Default, Deserialize)]
struct ChunkChoice {
    #[serde(default)]
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ChunkUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

/// `data:` ペイロード 1 件を解釈した結果。
#[derive(Debug, Default)]
pub(crate) struct ParsedChunk {
    pub events: Vec<Result<ChatEvent, LlmError>>,
    pub finish_reason: Option<FinishReason>,
    pub usage: Option<Usage>,
}

impl ParsedChunk {
    fn error(error: LlmError) -> Self {
        Self {
            events: vec![Err(error)],
            ..Default::default()
        }
    }

    /// このペイロードの処理で、ストリームを打ち切るべきエラーが出たか。
    pub fn is_error(&self) -> bool {
        matches!(self.events.last(), Some(Err(_)))
    }
}

/// SSE の `data:` ペイロード 1 件を解釈する。
///
/// `think` はストリーム全体で 1 つのインスタンスを使い回すこと。`<think>…</think>` が
/// 複数のチャンクにまたがっても正しく分離できるよう、状態をチャンクをまたいで保持している。
pub(crate) fn parse_chunk_payload(payload: &str, think: &mut ThinkTagFilter) -> ParsedChunk {
    let value: serde_json::Value = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(error) => {
            return ParsedChunk::error(LlmError::Protocol(format!(
                "応答チャンクを解釈できません: {error}"
            )));
        }
    };

    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("不明なエラー");
        return ParsedChunk::error(LlmError::Protocol(format!(
            "LLM サーバーがストリーム中にエラーを返しました: {message}"
        )));
    }

    let chunk: ChatCompletionChunk = match serde_json::from_value(value) {
        Ok(chunk) => chunk,
        Err(error) => {
            return ParsedChunk::error(LlmError::Protocol(format!(
                "応答チャンクの形式が不正です: {error}"
            )));
        }
    };

    let mut result = ParsedChunk::default();

    if let Some(choice) = chunk.choices.into_iter().next() {
        if let Some(reasoning) = choice.delta.reasoning_content.or(choice.delta.reasoning)
            && !reasoning.is_empty()
        {
            result.events.push(Ok(ChatEvent::Reasoning(reasoning)));
        }
        if let Some(content) = choice.delta.content {
            result
                .events
                .extend(think.push(&content).into_iter().map(Ok));
        }
        if let Some(reason) = choice.finish_reason {
            result.finish_reason = Some(FinishReason::from_wire(&reason));
        }
    }

    if let Some(usage) = chunk.usage {
        result.usage = Some(Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LlmError` は `PartialEq` を実装しないので、テストでは `Ok` の中身だけを比べる。
    fn unwrap_events(parsed: ParsedChunk) -> Vec<ChatEvent> {
        parsed.events.into_iter().map(Result::unwrap).collect()
    }

    #[test]
    fn 本文の差分だけを持つチャンクを解釈する() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload(
            r#"{"choices":[{"delta":{"content":"こんにちは"}}]}"#,
            &mut think,
        );
        assert!(parsed.finish_reason.is_none());
        assert!(parsed.usage.is_none());
        assert_eq!(
            unwrap_events(parsed),
            vec![ChatEvent::Content("こんにちは".to_string())]
        );
    }

    #[test]
    fn reasoning_content_は推論として扱う() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload(
            r#"{"choices":[{"delta":{"reasoning_content":"考え中"}}]}"#,
            &mut think,
        );
        assert_eq!(
            unwrap_events(parsed),
            vec![ChatEvent::Reasoning("考え中".to_string())]
        );
    }

    #[test]
    fn reasoning_フィールドでも推論として扱う() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload(
            r#"{"choices":[{"delta":{"reasoning":"考え中"}}]}"#,
            &mut think,
        );
        assert_eq!(
            unwrap_events(parsed),
            vec![ChatEvent::Reasoning("考え中".to_string())]
        );
    }

    #[test]
    fn finish_reason_を持つチャンクを解釈する() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload(
            r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#,
            &mut think,
        );
        assert!(parsed.events.is_empty());
        assert_eq!(parsed.finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn 未知の_finish_reason_は_other_になる() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload(
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            &mut think,
        );
        assert_eq!(
            parsed.finish_reason,
            Some(FinishReason::Other("tool_calls".to_string()))
        );
    }

    #[test]
    fn choices_が空で_usage_だけのチャンクを解釈する() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload(
            r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":34}}"#,
            &mut think,
        );
        assert!(parsed.events.is_empty());
        assert_eq!(
            parsed.usage,
            Some(Usage {
                prompt_tokens: 12,
                completion_tokens: 34
            })
        );
    }

    #[test]
    fn ストリーム中の_error_通知をプロトコルエラーに変換する() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload(r#"{"error":{"message":"内部エラー"}}"#, &mut think);
        assert!(parsed.is_error());
        match &parsed.events[0] {
            Err(LlmError::Protocol(message)) => assert!(message.contains("内部エラー")),
            other => panic!("Protocol エラーを期待したが {other:?} だった"),
        }
    }

    #[test]
    fn 壊れた_json_はプロトコルエラーになる() {
        let mut think = ThinkTagFilter::default();
        let parsed = parse_chunk_payload("{not json", &mut think);
        assert!(parsed.is_error());
    }

    #[test]
    fn think_タグがチャンクをまたいでも同じフィルタで正しく分離できる() {
        let mut think = ThinkTagFilter::default();
        let first = parse_chunk_payload(
            r#"{"choices":[{"delta":{"content":"<think>考え"}}]}"#,
            &mut think,
        );
        let second = parse_chunk_payload(
            r#"{"choices":[{"delta":{"content":"中</think>本文"}}]}"#,
            &mut think,
        );
        assert_eq!(
            unwrap_events(first),
            vec![ChatEvent::Reasoning("考え".to_string())]
        );
        assert_eq!(
            unwrap_events(second),
            vec![
                ChatEvent::Reasoning("中".to_string()),
                ChatEvent::Content("本文".to_string()),
            ]
        );
    }
}
