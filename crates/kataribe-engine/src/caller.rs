//! LLM の呼び出し。ストリーミングの中継、キャンセル、思考の制御、構造化出力をここに閉じ込める。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

use futures_util::StreamExt;
use kataribe_llm::{
    ChatEvent, ChatModel, ChatRequest, Completion, FinishReason, LlmError, Message, ResponseFormat,
    Sampling,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;

use crate::error::{EngineError, Result};
use crate::events::{EventSink, GenerationEvent, NoticeLevel, notice};
use crate::prompt::Prompt;
use crate::settings::GenerationSettings;

/// JSON の解析に失敗したときに生成し直す回数。
const JSON_ATTEMPTS: u32 = 2;

/// 思考を止めるために、応答の書き出しとして渡す空の思考ブロック。
/// Qwen 系など `<think>` を使うチャットテンプレートは、これで思考を飛ばして本文から書き始める。
const EMPTY_THINKING: &str = "<think>\n\n</think>\n\n";

/// 1 回の生成の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Output {
    pub text: String,
    /// 出力の上限（`max_tokens`）に達して途中で切れた。
    pub truncated: bool,
}

/// 1 つのタスクの間、LLM 呼び出しを受け持つ。
pub(crate) struct Caller<'a> {
    model: &'a dyn ChatModel,
    sink: &'a dyn EventSink,
    cancel: &'a CancellationToken,
    settings: &'a GenerationSettings,
    /// サーバーが JSON Schema 指定を受け付けるか（エンジンの寿命の間、覚えておく）。
    structured_output: &'a AtomicBool,
    step_index: AtomicU32,
    step_total: AtomicU32,
}

impl<'a> Caller<'a> {
    pub fn new(
        model: &'a dyn ChatModel,
        sink: &'a dyn EventSink,
        cancel: &'a CancellationToken,
        settings: &'a GenerationSettings,
        structured_output: &'a AtomicBool,
    ) -> Self {
        Self {
            model,
            sink,
            cancel,
            settings,
            structured_output,
            step_index: AtomicU32::new(0),
            step_total: AtomicU32::new(1),
        }
    }

    pub fn sink(&self) -> &dyn EventSink {
        self.sink
    }

    /// これから行う LLM 呼び出しの回数の見込みを増やす（進捗表示のため）。
    pub fn expect_steps(&self, additional: u32) {
        let done = self.step_index.load(Ordering::Relaxed);
        let total = self.step_total.load(Ordering::Relaxed).max(done);
        self.step_total.store(total + additional, Ordering::Relaxed);
    }

    /// 文章を生成する。本文の断片はそのままイベントとして流す。
    pub async fn text(&self, label: &str, prompt: &Prompt, max_tokens: u32) -> Result<Output> {
        let completion = self
            .complete(label, self.request(prompt, max_tokens, None))
            .await?;
        let truncated = completion.finish.reason == FinishReason::Length;
        Ok(Output {
            text: completion.content,
            truncated,
        })
    }

    /// JSON Schema に従う JSON を生成し、`T` として解釈する。
    pub async fn json<T>(&self, label: &str, prompt: &Prompt, max_tokens: u32) -> Result<T>
    where
        T: DeserializeOwned + JsonSchema,
    {
        let schema = serde_json::to_value(schemars::schema_for!(T))
            .map_err(|error| EngineError::Prompt(error.to_string()))?;
        let mut last_error = None;
        for _ in 0..JSON_ATTEMPTS {
            let completion = self
                .complete_structured(label, prompt, max_tokens, &schema)
                .await?;
            // 思考を止められないモデルでは、JSON が思考の側に入ってくることがある
            match parse_json::<T>(&completion.content)
                .or_else(|_| parse_json(&completion.reasoning))
            {
                Ok(value) => return Ok(value),
                Err(error) => {
                    notice(
                        self.sink,
                        NoticeLevel::Warning,
                        format!("{error} もう一度生成します。"),
                    );
                    self.expect_steps(1);
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| EngineError::InvalidOutput("JSON がありません。".into())))
    }

    async fn complete_structured(
        &self,
        label: &str,
        prompt: &Prompt,
        max_tokens: u32,
        schema: &serde_json::Value,
    ) -> Result<Completion> {
        if self.structured_output.load(Ordering::Relaxed) {
            let format = ResponseFormat::JsonSchema {
                name: "output".to_owned(),
                schema: schema.clone(),
            };
            match self
                .complete(label, self.request(prompt, max_tokens, Some(format)))
                .await
            {
                Err(EngineError::Llm(LlmError::Status { status: 400, .. })) => {
                    self.expect_steps(1);
                }
                result => return result,
            }
            // 400 の原因が JSON Schema の指定だったと確かめられたときだけ、以後の指定をやめる
            let fallback = self
                .complete_with_schema_instruction(label, prompt, max_tokens, schema)
                .await?;
            self.structured_output.store(false, Ordering::Relaxed);
            notice(
                self.sink,
                NoticeLevel::Info,
                "このサーバーは JSON Schema の指定に対応していないため、以後は指示文で JSON を求めます。",
            );
            return Ok(fallback);
        }
        self.complete_with_schema_instruction(label, prompt, max_tokens, schema)
            .await
    }

    async fn complete_with_schema_instruction(
        &self,
        label: &str,
        prompt: &Prompt,
        max_tokens: u32,
        schema: &serde_json::Value,
    ) -> Result<Completion> {
        let prompt = with_schema_instruction(prompt, schema);
        self.complete(label, self.request(&prompt, max_tokens, None))
            .await
    }

    fn request(
        &self,
        prompt: &Prompt,
        max_tokens: u32,
        format: Option<ResponseFormat>,
    ) -> ChatRequest {
        let mut messages = prompt.to_messages();
        let mut extra = serde_json::Map::new();
        if self.settings.disable_thinking {
            // モデルごとに効く方法が違うため、両方を指定する（LM Studio で gemma と qwen の両方に効く組み合わせ）
            messages.push(Message::assistant(EMPTY_THINKING));
            extra.insert("reasoning_effort".to_owned(), "none".into());
        }
        ChatRequest {
            messages,
            max_tokens: Some(max_tokens),
            sampling: Sampling {
                temperature: Some(self.settings.temperature),
                ..Sampling::default()
            },
            response_format: format,
            extra,
        }
    }

    async fn complete(&self, label: &str, request: ChatRequest) -> Result<Completion> {
        self.start_step(label);
        let started = Instant::now();
        let stream = self
            .model
            .stream_chat(request)
            .take_until(self.cancel.clone().cancelled_owned())
            .boxed();
        let sink = self.sink;
        let result = kataribe_llm::collect(stream, |event| forward(sink, event)).await;
        if self.cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        let completion = result?;
        let usage = completion.finish.usage.as_ref();
        self.sink.emit(GenerationEvent::StepFinished {
            prompt_tokens: usage.map(|usage| usage.prompt_tokens),
            completion_tokens: usage.map(|usage| usage.completion_tokens),
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        });
        if completion.finish.reason == FinishReason::Length {
            notice(
                self.sink,
                NoticeLevel::Warning,
                format!("「{label}」の出力が上限で途中まで切れました。"),
            );
        }
        Ok(completion)
    }

    fn start_step(&self, label: &str) {
        let index = self.step_index.fetch_add(1, Ordering::Relaxed) + 1;
        let total = self
            .step_total
            .fetch_max(index, Ordering::Relaxed)
            .max(index);
        self.sink.emit(GenerationEvent::StepStarted {
            label: label.to_owned(),
            index,
            total,
        });
    }
}

fn forward(sink: &dyn EventSink, event: &ChatEvent) {
    match event {
        ChatEvent::Content(text) => sink.emit(GenerationEvent::Content { text: text.clone() }),
        ChatEvent::Reasoning(text) => sink.emit(GenerationEvent::Reasoning { text: text.clone() }),
        ChatEvent::Finished(_) => {}
    }
}

/// 構造化出力に対応していないサーバー向けに、スキーマを指示文に書き足す。
fn with_schema_instruction(prompt: &Prompt, schema: &serde_json::Value) -> Prompt {
    let schema = serde_json::to_string(schema).unwrap_or_default();
    Prompt {
        system: prompt.system.clone(),
        user: format!(
            "{}\n\n出力は次の JSON Schema に従う JSON だけにすること。コードブロックや説明は付けないこと。\n{schema}",
            prompt.user
        ),
    }
}

/// 出力から JSON を取り出して解釈する。前後の説明文やコードブロックは無視する。
fn parse_json<T: DeserializeOwned>(content: &str) -> Result<T> {
    let json = extract_json_object(content)
        .ok_or_else(|| EngineError::InvalidOutput("出力に JSON が見つかりません。".into()))?;
    serde_json::from_str(json)
        .map_err(|error| EngineError::InvalidOutput(format!("JSON の形が違います（{error}）。")))
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end).then(|| &text[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::IgnoreEvents;
    use kataribe_llm::Role;
    use kataribe_llm::testing::{Script, ScriptedChatModel};
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use std::sync::Mutex;

    #[derive(Debug, Deserialize, JsonSchema, PartialEq)]
    struct Names {
        names: Vec<String>,
    }

    fn prompt() -> Prompt {
        Prompt {
            system: "system".into(),
            user: "user".into(),
        }
    }

    struct Recorder(Mutex<Vec<GenerationEvent>>);

    impl EventSink for Recorder {
        fn emit(&self, event: GenerationEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    struct Fixture {
        cancel: CancellationToken,
        settings: GenerationSettings,
        structured: AtomicBool,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                cancel: CancellationToken::new(),
                settings: GenerationSettings::default(),
                structured: AtomicBool::new(true),
            }
        }

        fn caller<'a>(&'a self, model: &'a dyn ChatModel, sink: &'a dyn EventSink) -> Caller<'a> {
            Caller::new(model, sink, &self.cancel, &self.settings, &self.structured)
        }
    }

    #[test]
    fn json_is_extracted_from_surrounding_text() {
        let parsed: Names = parse_json("はい。\n```json\n{\"names\": [\"凛\"]}\n```").unwrap();
        assert_eq!(parsed.names, vec!["凛"]);
    }

    #[test]
    fn missing_json_is_an_invalid_output_error() {
        assert!(matches!(
            parse_json::<Names>("なし"),
            Err(EngineError::InvalidOutput(_))
        ));
    }

    #[tokio::test]
    async fn text_streams_content_events_and_numbers_steps() {
        let model = ScriptedChatModel::new([Script::reply(["雨", "が降る"])]);
        let recorder = Recorder(Mutex::new(Vec::new()));
        let fixture = Fixture::new();

        let output = fixture
            .caller(&model, &recorder)
            .text("本文", &prompt(), 100)
            .await
            .unwrap();

        assert_eq!(output.text, "雨が降る");
        assert!(!output.truncated);
        let events = recorder.0.into_inner().unwrap();
        assert_eq!(
            events[0],
            GenerationEvent::StepStarted {
                label: "本文".into(),
                index: 1,
                total: 1
            }
        );
        assert_eq!(events[1], GenerationEvent::Content { text: "雨".into() });
        assert!(matches!(
            events.last(),
            Some(GenerationEvent::StepFinished { .. })
        ));
    }

    #[tokio::test]
    async fn thinking_is_disabled_with_both_known_mechanisms() {
        let model = ScriptedChatModel::new([Script::reply(["本文"])]);
        let fixture = Fixture::new();

        fixture
            .caller(&model, &IgnoreEvents)
            .text("本文", &prompt(), 100)
            .await
            .unwrap();

        let request = &model.requests()[0];
        let last = request.messages.last().unwrap();
        assert_eq!(
            (last.role, last.content.as_str()),
            (Role::Assistant, EMPTY_THINKING)
        );
        assert_eq!(request.extra["reasoning_effort"], "none");
    }

    #[tokio::test]
    async fn thinking_controls_are_omitted_when_disabled_in_settings() {
        let model = ScriptedChatModel::new([Script::reply(["本文"])]);
        let mut fixture = Fixture::new();
        fixture.settings.disable_thinking = false;

        fixture
            .caller(&model, &IgnoreEvents)
            .text("本文", &prompt(), 100)
            .await
            .unwrap();

        let request = &model.requests()[0];
        assert_eq!(request.messages.last().unwrap().role, Role::User);
        assert!(request.extra.is_empty());
    }

    #[tokio::test]
    async fn truncated_output_is_reported() {
        let model =
            ScriptedChatModel::new([Script::reply(["途中"]).finish_reason(FinishReason::Length)]);
        let recorder = Recorder(Mutex::new(Vec::new()));
        let fixture = Fixture::new();

        let output = fixture
            .caller(&model, &recorder)
            .text("本文", &prompt(), 100)
            .await
            .unwrap();

        assert!(output.truncated);
        let events = recorder.0.into_inner().unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            GenerationEvent::Notice {
                level: NoticeLevel::Warning,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn json_falls_back_to_prompt_instructions_when_schema_is_rejected() {
        let model = ScriptedChatModel::new([
            Script::error(LlmError::Status {
                status: 400,
                body: "response_format not supported".into(),
            }),
            Script::reply([r#"{"names": ["凛", "健二"]}"#]),
        ]);
        let fixture = Fixture::new();

        let names: Names = fixture
            .caller(&model, &IgnoreEvents)
            .json("人物", &prompt(), 100)
            .await
            .unwrap();

        assert_eq!(names.names, vec!["凛", "健二"]);
        assert!(!fixture.structured.load(Ordering::Relaxed));
        let requests = model.requests();
        assert!(requests[0].response_format.is_some());
        assert!(requests[1].response_format.is_none());
        assert!(requests[1].messages[1].content.contains("JSON Schema"));
    }

    #[tokio::test]
    async fn unrelated_bad_request_keeps_structured_output_enabled() {
        let bad_request = || LlmError::Status {
            status: 400,
            body: "context length exceeded".into(),
        };
        let model =
            ScriptedChatModel::new([Script::error(bad_request()), Script::error(bad_request())]);
        let fixture = Fixture::new();

        let result = fixture
            .caller(&model, &IgnoreEvents)
            .json::<Names>("人物", &prompt(), 100)
            .await;

        assert!(result.is_err());
        assert!(fixture.structured.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn json_is_recovered_from_reasoning_when_content_is_empty() {
        let model =
            ScriptedChatModel::new([Script::reply([""]).with_reasoning([r#"{"names": ["凛"]}"#])]);
        let fixture = Fixture::new();

        let names: Names = fixture
            .caller(&model, &IgnoreEvents)
            .json("人物", &prompt(), 100)
            .await
            .unwrap();

        assert_eq!(names.names, vec!["凛"]);
    }

    #[tokio::test]
    async fn json_is_regenerated_once_when_unparseable() {
        let model = ScriptedChatModel::new([
            Script::reply(["壊れた出力"]),
            Script::reply([r#"{"names": []}"#]),
        ]);
        let fixture = Fixture::new();

        let names: Names = fixture
            .caller(&model, &IgnoreEvents)
            .json("人物", &prompt(), 100)
            .await
            .unwrap();

        assert_eq!(names.names, Vec::<String>::new());
        assert_eq!(model.requests().len(), 2);
    }

    #[tokio::test]
    async fn cancelled_token_stops_generation() {
        let model = ScriptedChatModel::new([Script::reply(["本文"])]);
        let fixture = Fixture::new();
        fixture.cancel.cancel();

        let result = fixture
            .caller(&model, &IgnoreEvents)
            .text("本文", &prompt(), 100)
            .await;

        assert!(matches!(result, Err(EngineError::Cancelled)));
    }
}
