//! 他 crate のテストで使う、台本どおりに応答する偽の [`ChatModel`]。
//!
//! `testing` feature でのみコンパイルされる。

use std::collections::VecDeque;
use std::sync::Mutex;

use futures_util::StreamExt;

use crate::chat_model::{ChatModel, ChatStream};
use crate::error::LlmError;
use crate::event::{ChatEvent, Finish, FinishReason, Usage};
use crate::request::ChatRequest;

#[derive(Debug)]
enum ScriptStep {
    Content(String),
    Reasoning(String),
}

#[derive(Debug)]
enum ScriptOutcome {
    Finished(Finish),
    Error(LlmError),
}

/// [`ScriptedChatModel`] に読み込ませる、1 回の `stream_chat` 呼び出し分の応答台本。
///
/// # 例
/// ```
/// use kataribe_llm::testing::Script;
/// use kataribe_llm::FinishReason;
///
/// let script = Script::reply(["こん", "にちは"]).finish_reason(FinishReason::Stop);
/// ```
#[derive(Debug)]
#[must_use = "Script はそのまま ScriptedChatModel::new に渡して使う"]
pub struct Script {
    steps: Vec<ScriptStep>,
    outcome: ScriptOutcome,
}

impl Script {
    /// 本文の断片を順に流し、`FinishReason::Stop` で終える台本を作る。
    pub fn reply<I, S>(chunks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            steps: chunks
                .into_iter()
                .map(|chunk| ScriptStep::Content(chunk.into()))
                .collect(),
            outcome: ScriptOutcome::Finished(Finish {
                reason: FinishReason::Stop,
                usage: None,
            }),
        }
    }

    /// 台本の先頭に、推論の断片を追加する（実際のモデルは本文より先に推論を出すため）。
    pub fn with_reasoning<I, S>(mut self, chunks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut reasoning_steps: Vec<ScriptStep> = chunks
            .into_iter()
            .map(|chunk| ScriptStep::Reasoning(chunk.into()))
            .collect();
        reasoning_steps.extend(self.steps);
        self.steps = reasoning_steps;
        self
    }

    /// 終了理由を指定する（既定は [`FinishReason::Stop`]）。エラー台本には効果がない。
    pub fn finish_reason(mut self, reason: FinishReason) -> Self {
        if let ScriptOutcome::Finished(finish) = &mut self.outcome {
            finish.reason = reason;
        }
        self
    }

    /// トークン使用量を指定する。エラー台本には効果がない。
    pub fn usage(mut self, usage: Usage) -> Self {
        if let ScriptOutcome::Finished(finish) = &mut self.outcome {
            finish.usage = Some(usage);
        }
        self
    }

    /// 本文・推論を 1 つも返さず、エラーで終わる台本を作る。
    pub fn error(error: LlmError) -> Self {
        Self {
            steps: Vec::new(),
            outcome: ScriptOutcome::Error(error),
        }
    }

    fn into_events(self) -> Vec<Result<ChatEvent, LlmError>> {
        let mut events: Vec<Result<ChatEvent, LlmError>> = self
            .steps
            .into_iter()
            .map(|step| {
                Ok(match step {
                    ScriptStep::Content(text) => ChatEvent::Content(text),
                    ScriptStep::Reasoning(text) => ChatEvent::Reasoning(text),
                })
            })
            .collect();

        match self.outcome {
            ScriptOutcome::Finished(finish) => events.push(Ok(ChatEvent::Finished(finish))),
            ScriptOutcome::Error(error) => events.push(Err(error)),
        }

        events
    }
}

/// 台本どおりに応答し、受け取った [`ChatRequest`] を記録する偽の [`ChatModel`]。
///
/// `stream_chat` が呼ばれるたびに、読み込んだ台本を 1 つずつ先頭から消費する。
/// 台本が尽きた状態で呼ばれると、エラーのストリームを返す。
#[derive(Debug, Default)]
pub struct ScriptedChatModel {
    scripts: Mutex<VecDeque<Script>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl ScriptedChatModel {
    /// 台本を読み込んだモデルを作る。
    pub fn new(scripts: impl IntoIterator<Item = Script>) -> Self {
        Self {
            scripts: Mutex::new(scripts.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// これまでに `stream_chat` へ渡された [`ChatRequest`] を、呼ばれた順に返す。
    pub fn requests(&self) -> Vec<ChatRequest> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ChatModel for ScriptedChatModel {
    fn describe(&self) -> String {
        "台本どおりに応答するテスト用のモデル".to_owned()
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request);

        let script = self
            .scripts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front();

        let events = match script {
            Some(script) => script.into_events(),
            None => vec![Err(LlmError::Protocol(
                "ScriptedChatModel: 台本が尽きました。".to_string(),
            ))],
        };

        futures_util::stream::iter(events).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect::collect;
    use crate::message::Message;

    #[tokio::test]
    async fn 台本どおりに本文と推論を返す() {
        let model =
            ScriptedChatModel::new([Script::reply(["こん", "にちは"]).with_reasoning(["考え中"])]);
        let request = ChatRequest {
            messages: vec![Message::user("test")],
            ..Default::default()
        };

        let completion = collect(model.stream_chat(request), |_| {}).await.unwrap();
        assert_eq!(completion.content, "こんにちは");
        assert_eq!(completion.reasoning, "考え中");
        assert_eq!(completion.finish.reason, FinishReason::Stop);
    }

    #[tokio::test]
    async fn 台本のエラーがそのまま返る() {
        let model = ScriptedChatModel::new([Script::error(LlmError::Timeout)]);
        let result = collect(model.stream_chat(ChatRequest::default()), |_| {}).await;
        assert!(matches!(result, Err(LlmError::Timeout)));
    }

    #[tokio::test]
    async fn 台本が尽きるとエラーになる() {
        let model = ScriptedChatModel::new([]);
        let result = collect(model.stream_chat(ChatRequest::default()), |_| {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn 受け取ったリクエストを記録する() {
        let model = ScriptedChatModel::new([Script::reply(["ok"])]);
        let request = ChatRequest {
            messages: vec![Message::user("覚えて")],
            ..Default::default()
        };
        let _ = collect(model.stream_chat(request.clone()), |_| {}).await;

        assert_eq!(model.requests(), vec![request]);
    }
}
