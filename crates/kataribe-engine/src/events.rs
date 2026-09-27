//! 生成中の経過を画面や CLI に伝えるイベント。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum GenerationEvent {
    /// LLM の呼び出しを 1 回始めた。`index` は 1 始まり。
    StepStarted {
        label: String,
        index: u32,
        total: u32,
    },
    /// 本文として採用される出力の断片。
    Content {
        text: String,
    },
    /// 推論モデルの思考の断片（本文には入らない）。
    Reasoning {
        text: String,
    },
    StepFinished {
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
        // JSON では数値として届くので、TypeScript でも bigint ではなく number にする
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        elapsed_ms: u64,
    },
    Notice {
        level: NoticeLevel,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum NoticeLevel {
    Info,
    Warning,
}

/// イベントの受け取り先。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: GenerationEvent);
}

impl<F> EventSink for F
where
    F: Fn(GenerationEvent) + Send + Sync,
{
    fn emit(&self, event: GenerationEvent) {
        self(event);
    }
}

/// イベントを捨てる受け取り先。
#[derive(Debug, Clone, Copy, Default)]
pub struct IgnoreEvents;

impl EventSink for IgnoreEvents {
    fn emit(&self, _event: GenerationEvent) {}
}

pub(crate) fn notice(sink: &dyn EventSink, level: NoticeLevel, message: impl Into<String>) {
    sink.emit(GenerationEvent::Notice {
        level,
        message: message.into(),
    });
}
