use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};
use serde::Deserialize;

use crate::chat_model::{ChatModel, ChatStream};
use crate::chunk::parse_chunk_payload;
use crate::config::ClientConfig;
use crate::error::{LlmError, truncate_body};
use crate::event::{ChatEvent, Finish, FinishReason, Usage};
use crate::model_info::ModelInfo;
use crate::request::{ChatRequest, WireChatRequest};
use crate::sse::{SseDecoder, SseEvent};
use crate::think_filter::ThinkTagFilter;

/// 429 (Too Many Requests) を表す HTTP ステータスコード。
const TOO_MANY_REQUESTS: u16 = 429;
const INITIAL_BACKOFF: Duration = Duration::from_millis(500);

/// `OpenAI` 互換 Chat Completions API のストリーミングクライアント。
///
/// 主な利用先は LM Studio。Ollama・`KoboldCpp`・llama.cpp server・`OpenRouter`・`OpenAI` など、
/// 同じ API 形式に合わせているサーバーであれば概ねそのまま使える。
#[derive(Debug, Clone)]
pub struct OpenAiCompatClient {
    http: reqwest::Client,
    config: Arc<ClientConfig>,
}

impl OpenAiCompatClient {
    /// クライアントを作る。`config.base_url` が http(s) の URL でなければ
    /// [`LlmError::InvalidConfig`] を返す。
    pub fn new(config: ClientConfig) -> Result<Self, LlmError> {
        let base_url = config.base_url.trim_end_matches('/').to_string();
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            return Err(LlmError::InvalidConfig(format!(
                "base_url は http または https の URL である必要があります: {base_url}"
            )));
        }

        let http = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|error| {
                LlmError::InvalidConfig(format!("HTTP クライアントを初期化できません: {error}"))
            })?;

        let mut config = config;
        config.base_url = base_url;

        Ok(Self {
            http,
            config: Arc::new(config),
        })
    }

    /// サーバーが公開しているモデルの一覧を取得する。
    ///
    /// LM Studio の拡張 API（`{origin}/api/v0/models`）が使えれば、文脈長も埋める。
    /// 拡張 API の取得に失敗しても無視して、標準の一覧だけを返す。
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        let url = format!("{}/models", self.config.base_url);
        let response = send_within_idle_timeout(
            self.authorize(self.http.get(&url)),
            self.config.idle_timeout,
            &self.config.base_url,
        )
        .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Status {
                status: status.as_u16(),
                body: truncate_body(&body),
            });
        }

        let payload: ModelsListResponse = response.json().await.map_err(|error| {
            LlmError::Protocol(format!("モデル一覧の応答を解釈できません: {error}"))
        })?;
        let mut models: Vec<ModelInfo> = payload
            .data
            .into_iter()
            .map(|entry| ModelInfo {
                id: entry.id,
                context_length: None,
            })
            .collect();

        if let Some(origin) = self.config.base_url.strip_suffix("/v1")
            && let Some(context_lengths) = self.fetch_lm_studio_context_lengths(origin).await
        {
            for model in &mut models {
                if let Some(context_length) = context_lengths.get(&model.id) {
                    model.context_length = *context_length;
                }
            }
        }

        Ok(models)
    }

    /// LM Studio 拡張 API からモデルごとの文脈長を取得する。失敗したら `None`。
    async fn fetch_lm_studio_context_lengths(
        &self,
        origin: &str,
    ) -> Option<HashMap<String, Option<u32>>> {
        let url = format!("{origin}/api/v0/models");
        let response = tokio::time::timeout(
            self.config.idle_timeout,
            self.authorize(self.http.get(&url)).send(),
        )
        .await
        .ok()?
        .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let payload: LmStudioModelsResponse = response.json().await.ok()?;
        Some(
            payload
                .data
                .into_iter()
                .map(|entry| {
                    let context_length = entry.loaded_context_length.or(entry.max_context_length);
                    (entry.id, context_length)
                })
                .collect(),
        )
    }

    fn authorize(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.config.api_key {
            Some(api_key) => builder.bearer_auth(api_key),
            None => builder,
        }
    }
}

impl ChatModel for OpenAiCompatClient {
    fn stream_chat(&self, request: ChatRequest) -> ChatStream {
        let state = StreamState {
            http: self.http.clone(),
            config: Arc::clone(&self.config),
            request,
            phase: Phase::Connecting,
        };
        futures_util::stream::unfold(state, advance).boxed()
    }
}

#[derive(Deserialize)]
struct ModelsListResponse {
    data: Vec<ModelsListEntry>,
}

#[derive(Deserialize)]
struct ModelsListEntry {
    id: String,
}

#[derive(Deserialize)]
struct LmStudioModelsResponse {
    data: Vec<LmStudioModelEntry>,
}

#[derive(Deserialize)]
struct LmStudioModelEntry {
    id: String,
    #[serde(default)]
    loaded_context_length: Option<u32>,
    #[serde(default)]
    max_context_length: Option<u32>,
}

/// [`futures_util::stream::unfold`] で使う、ストリームの内部状態。
struct StreamState {
    http: reqwest::Client,
    config: Arc<ClientConfig>,
    request: ChatRequest,
    phase: Phase,
}

enum Phase {
    /// まだ接続していない。次にポーリングされたら接続を試みる。
    Connecting,
    /// 接続に成功し、応答本文を読み進めている。
    Streaming(Box<StreamingState>),
    /// 終了した（正常終了・エラーのどちらでも、これ以上イベントは出ない）。
    Done,
}

struct StreamingState {
    bytes: BoxStream<'static, reqwest::Result<Vec<u8>>>,
    sse: SseDecoder,
    think: ThinkTagFilter,
    pending: VecDeque<Result<ChatEvent, LlmError>>,
    finish_reason: Option<FinishReason>,
    usage: Option<Usage>,
}

/// ストリームを 1 段階進める。
///
/// 設計上の判断: 再試行の対象は「接続を確立し、2xx の応答を得るまで」に限る。
/// いったん本文の読み取りを始めた後に接続が切れた場合は、そのままエラーとして
/// 終える（黙って全体を再送すると、既に読めた分の扱いが曖昧になるため）。
async fn advance(mut state: StreamState) -> Option<(Result<ChatEvent, LlmError>, StreamState)> {
    loop {
        match &mut state.phase {
            Phase::Connecting => match connect(&state.http, &state.config, &state.request).await {
                Ok(bytes) => {
                    state.phase = Phase::Streaming(Box::new(StreamingState {
                        bytes,
                        sse: SseDecoder::default(),
                        think: ThinkTagFilter::default(),
                        pending: VecDeque::new(),
                        finish_reason: None,
                        usage: None,
                    }));
                }
                Err(error) => {
                    state.phase = Phase::Done;
                    return Some((Err(error), state));
                }
            },
            Phase::Streaming(streaming) => {
                if let Some(event) = streaming.pending.pop_front() {
                    if matches!(event, Err(_) | Ok(ChatEvent::Finished(_))) {
                        state.phase = Phase::Done;
                    }
                    return Some((event, state));
                }

                match tokio::time::timeout(state.config.idle_timeout, streaming.bytes.next()).await
                {
                    Err(_elapsed) => {
                        state.phase = Phase::Done;
                        return Some((Err(LlmError::Timeout), state));
                    }
                    Ok(None) => {
                        // 改行で終わらない末尾の行が残っていれば、それも最後の出来事として処理する。
                        let sse = std::mem::take(&mut streaming.sse);
                        if !process_raw_events(sse.finish(), streaming) {
                            finalize_stream(streaming);
                        }
                    }
                    Ok(Some(Err(source))) => {
                        streaming.pending.push_back(Err(LlmError::connection(
                            state.config.base_url.clone(),
                            source,
                        )));
                    }
                    Ok(Some(Ok(chunk))) => {
                        let raw_events = streaming.sse.push(&chunk);
                        process_raw_events(raw_events, streaming);
                    }
                }
            }
            Phase::Done => return None,
        }
    }
}

/// SSE デコーダが返した出来事を解釈し、`pending` に積む。
///
/// `[DONE]` またはストリーム中のエラーに達し、ストリームを終えるべきなら `true` を返す
/// （呼び出し側は、それ以上バイト列を読み進めない）。
fn process_raw_events(raw_events: Vec<SseEvent>, streaming: &mut StreamingState) -> bool {
    for raw_event in raw_events {
        match raw_event {
            SseEvent::Done => {
                finalize_stream(streaming);
                return true;
            }
            SseEvent::Data(payload) => {
                let parsed = parse_chunk_payload(&payload, &mut streaming.think);
                let is_error = parsed.is_error();
                streaming.pending.extend(parsed.events);
                if let Some(reason) = parsed.finish_reason {
                    streaming.finish_reason = Some(reason);
                }
                if let Some(usage) = parsed.usage {
                    streaming.usage = Some(usage);
                }
                if is_error {
                    return true;
                }
            }
        }
    }
    false
}

/// 保留中の `<think>` フィルタを吐き出し、[`ChatEvent::Finished`] を積む。
/// これ以降、このストリームからは何も読み出さない。
fn finalize_stream(streaming: &mut StreamingState) {
    let think = std::mem::take(&mut streaming.think);
    streaming.pending.extend(think.finish().into_iter().map(Ok));

    let finish = Finish {
        reason: streaming
            .finish_reason
            .take()
            .unwrap_or(FinishReason::Other("eof".to_string())),
        usage: streaming.usage.take(),
    };
    streaming.pending.push_back(Ok(ChatEvent::Finished(finish)));
}

/// 接続を確立し、応答本文のバイト列ストリームを返す。
/// 接続失敗・5xx・429 は指数バックオフで `config.max_retries` 回まで再試行する。
async fn connect(
    http: &reqwest::Client,
    config: &ClientConfig,
    request: &ChatRequest,
) -> Result<BoxStream<'static, reqwest::Result<Vec<u8>>>, LlmError> {
    let url = format!("{}/chat/completions", config.base_url);
    let wire = WireChatRequest::new(&config.model, request);

    let mut attempt = 0u32;
    loop {
        let mut builder = http.post(&url).json(&wire);
        if let Some(api_key) = &config.api_key {
            builder = builder.bearer_auth(api_key);
        }

        match send_and_open_stream(builder, &config.base_url, config.idle_timeout).await {
            Ok(stream) => return Ok(stream),
            Err(error) if attempt < config.max_retries && is_retryable(&error) => {
                attempt += 1;
                tracing::warn!(attempt, error = %error, "LLM サーバーへの接続を再試行します");
                tokio::time::sleep(backoff_delay(attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn send_and_open_stream(
    builder: reqwest::RequestBuilder,
    base_url: &str,
    idle_timeout: Duration,
) -> Result<BoxStream<'static, reqwest::Result<Vec<u8>>>, LlmError> {
    let response = send_within_idle_timeout(builder, idle_timeout, base_url).await?;

    let status = response.status();
    if status.is_success() {
        Ok(response
            .bytes_stream()
            .map_ok(|bytes| bytes.to_vec())
            .boxed())
    } else {
        let status_code = status.as_u16();
        let body = response.text().await.unwrap_or_default();
        Err(LlmError::Status {
            status: status_code,
            body: truncate_body(&body),
        })
    }
}

/// `builder.send()` を `idle_timeout` で包む。
///
/// 接続は受け付けるが応答を返さないサーバーに対して、応答ヘッダーを無限に待ち続けない
/// ようにするための上限。タイムアウトしたら [`LlmError::Timeout`] にする。
async fn send_within_idle_timeout(
    builder: reqwest::RequestBuilder,
    idle_timeout: Duration,
    base_url: &str,
) -> Result<reqwest::Response, LlmError> {
    tokio::time::timeout(idle_timeout, builder.send())
        .await
        .map_err(|_elapsed| LlmError::Timeout)?
        .map_err(|error| LlmError::connection(base_url.to_string(), error))
}

fn is_retryable(error: &LlmError) -> bool {
    match error {
        LlmError::Connection { .. } => true,
        LlmError::Status { status, .. } => *status == TOO_MANY_REQUESTS || *status >= 500,
        _ => false,
    }
}

fn backoff_delay(attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(10);
    INITIAL_BACKOFF * (1u32 << shift)
}
