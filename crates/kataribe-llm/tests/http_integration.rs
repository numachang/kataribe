//! wiremock を使った統合テスト。実際の LM Studio などへは一切つながない。

use std::time::Duration;

use futures_util::StreamExt;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use kataribe_llm::{
    ChatEvent, ChatModel, ChatRequest, ClientConfig, FinishReason, LlmError, Message, ModelInfo,
    OpenAiCompatClient, ResponseFormat, Sampling, Usage, collect,
};

/// `data:` チャンクの列から、`[DONE]` で終わる SSE 本文を組み立てる。
fn sse_body(chunks: &[serde_json::Value]) -> String {
    let mut body = String::new();
    for chunk in chunks {
        body.push_str("data: ");
        body.push_str(&chunk.to_string());
        body.push_str("\n\n");
    }
    body.push_str("data: [DONE]\n\n");
    body
}

/// テスト用のヘルパーであり、`#[test]` 関数そのものではないため、
/// `clippy.toml` の `allow-unwrap-in-tests` の対象にならない。ここでの `unwrap` は
/// 常に妥当な設定しか渡さないテスト補助という前提で許容する。
#[allow(clippy::unwrap_used)]
fn client_for(
    server: &MockServer,
    config: impl FnOnce(ClientConfig) -> ClientConfig,
) -> OpenAiCompatClient {
    let base = ClientConfig {
        base_url: format!("{}/v1", server.uri()),
        model: "test-model".to_string(),
        ..ClientConfig::default()
    };
    OpenAiCompatClient::new(config(base)).unwrap()
}

fn request_with(messages: Vec<Message>) -> ChatRequest {
    ChatRequest {
        messages,
        ..ChatRequest::default()
    }
}

#[tokio::test]
async fn 正常なストリーミングで本文を組み立てる() {
    let server = MockServer::start().await;
    let chunks = vec![
        json!({"choices": [{"delta": {"role": "assistant"}}]}),
        json!({"choices": [{"delta": {"content": "こん"}}]}),
        json!({"choices": [{"delta": {"content": "にちは"}, "finish_reason": "stop"}]}),
    ];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let stream = client.stream_chat(request_with(vec![Message::user("こんにちは")]));
    let completion = collect(stream, |_| {}).await.unwrap();

    assert_eq!(completion.content, "こんにちは");
    assert_eq!(completion.finish.reason, FinishReason::Stop);
}

#[tokio::test]
async fn 最後の_usage_だけのチャンクを_finished_にまとめる() {
    let server = MockServer::start().await;
    let chunks = vec![
        json!({"choices": [{"delta": {"content": "本文"}, "finish_reason": "stop"}]}),
        json!({"choices": [], "usage": {"prompt_tokens": 12, "completion_tokens": 34}}),
    ];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let stream = client.stream_chat(request_with(vec![Message::user("test")]));
    let completion = collect(stream, |_| {}).await.unwrap();

    assert_eq!(completion.finish.reason, FinishReason::Stop);
    assert_eq!(
        completion.finish.usage,
        Some(Usage {
            prompt_tokens: 12,
            completion_tokens: 34
        })
    );
}

#[tokio::test]
async fn reasoning_content_は本文と分離される() {
    let server = MockServer::start().await;
    let chunks = vec![
        json!({"choices": [{"delta": {"reasoning_content": "考え中"}}]}),
        json!({"choices": [{"delta": {"content": "結論"}, "finish_reason": "stop"}]}),
    ];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let stream = client.stream_chat(request_with(vec![Message::user("test")]));
    let completion = collect(stream, |_| {}).await.unwrap();

    assert_eq!(completion.reasoning, "考え中");
    assert_eq!(completion.content, "結論");
}

#[tokio::test]
async fn response_format_と_extra_がリクエスト本体に反映される() {
    let server = MockServer::start().await;
    let chunks = vec![json!({"choices": [{"delta": {"content": "{}"}, "finish_reason": "stop"}]})];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let mut extra = serde_json::Map::new();
    extra.insert("top_k".to_string(), json!(40));
    let request = ChatRequest {
        messages: vec![Message::system("規則"), Message::user("キャストを作って")],
        sampling: Sampling {
            temperature: Some(0.7),
            ..Sampling::default()
        },
        response_format: Some(ResponseFormat::JsonSchema {
            name: "cast".to_string(),
            schema: json!({"type": "object"}),
        }),
        extra,
        ..ChatRequest::default()
    };
    collect(client.stream_chat(request), |_| {}).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();

    assert_eq!(body["model"], "test-model");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["temperature"], 0.7);
    assert_eq!(body["top_k"], 40);
    assert_eq!(
        body["response_format"],
        json!({
            "type": "json_schema",
            "json_schema": {"name": "cast", "strict": true, "schema": {"type": "object"}}
        })
    );
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["content"], "キャストを作って");
}

#[tokio::test]
async fn api_key_を設定すると_authorization_ヘッダーが付く() {
    let server = MockServer::start().await;
    let chunks = vec![json!({"choices": [{"delta": {"content": "ok"}, "finish_reason": "stop"}]})];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, |mut c| {
        c.api_key = Some("sk-secret".to_string());
        c
    });
    collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    let auth = requests[0].headers.get("authorization").unwrap();
    assert_eq!(auth, "Bearer sk-secret");
}

#[tokio::test]
async fn api_key_を設定しなければ_authorization_ヘッダーは付かない() {
    let server = MockServer::start().await;
    let chunks = vec![json!({"choices": [{"delta": {"content": "ok"}, "finish_reason": "stop"}]})];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].headers.get("authorization").is_none());
}

#[tokio::test]
async fn ステータス_400_では再試行しない() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let result = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await;

    match result {
        Err(LlmError::Status { status, .. }) => assert_eq!(status, 400),
        other => panic!("Status エラーを期待したが {other:?} だった"),
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn ステータス_503_の後に成功すれば再試行で成功する() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    let chunks =
        vec![json!({"choices": [{"delta": {"content": "復帰"}, "finish_reason": "stop"}]})];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .with_priority(2)
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let completion = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(completion.content, "復帰");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn ストリーム中の_error_通知はエラーとして返す() {
    let server = MockServer::start().await;
    let body = sse_body(&[json!({"choices": [{"delta": {"content": "途中まで"}}]})])
        .trim_end_matches("data: [DONE]\n\n")
        .to_string()
        + &format!(
            "data: {}\n\n",
            json!({"error": {"message": "内部で問題が発生しました"}})
        );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let result = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await;

    match result {
        Err(LlmError::Protocol(message)) => {
            assert!(message.contains("内部で問題が発生しました"));
        }
        other => panic!("Protocol エラーを期待したが {other:?} だった"),
    }
}

#[tokio::test]
async fn list_models_は_lm_studio_拡張がなければ標準の一覧だけを返す() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "model-a"}, {"id": "model-b"}]
        })))
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let models = client.list_models().await.unwrap();

    assert_eq!(
        models,
        vec![
            ModelInfo {
                id: "model-a".to_string(),
                context_length: None
            },
            ModelInfo {
                id: "model-b".to_string(),
                context_length: None
            },
        ]
    );
}

#[tokio::test]
async fn list_models_は_lm_studio_拡張から文脈長を埋める() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "model-a"}, {"id": "model-b"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v0/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {"id": "model-a", "loaded_context_length": 8192, "max_context_length": 32768},
                {"id": "model-b", "max_context_length": 4096},
            ]
        })))
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let models = client.list_models().await.unwrap();

    assert_eq!(
        models,
        vec![
            ModelInfo {
                id: "model-a".to_string(),
                context_length: Some(8192)
            },
            ModelInfo {
                id: "model-b".to_string(),
                context_length: Some(4096)
            },
        ]
    );
}

#[tokio::test]
async fn 応答ヘッダーが来ないと_idle_timeout_でタイムアウトする() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(300)))
        .mount(&server)
        .await;

    let client = client_for(&server, |mut c| {
        c.idle_timeout = Duration::from_millis(50);
        c
    });
    let result = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await;

    assert!(matches!(result, Err(LlmError::Timeout)));
}

#[tokio::test]
async fn list_models_も応答が来ないと_idle_timeout_でタイムアウトする() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(300)))
        .mount(&server)
        .await;

    let client = client_for(&server, |mut c| {
        c.idle_timeout = Duration::from_millis(50);
        c
    });
    let result = client.list_models().await;

    assert!(matches!(result, Err(LlmError::Timeout)));
}

#[tokio::test]
async fn ステータス_429_の後に成功すれば再試行で成功する() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    let chunks =
        vec![json!({"choices": [{"delta": {"content": "復帰"}, "finish_reason": "stop"}]})];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .with_priority(2)
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let completion = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(completion.content, "復帰");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn 再試行回数を使い切るとエラーのまま終わる() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .mount(&server)
        .await;

    let client = client_for(&server, |mut c| {
        c.max_retries = 1;
        c
    });
    let result = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await;

    match result {
        Err(LlmError::Status { status, .. }) => assert_eq!(status, 503),
        other => panic!("Status エラーを期待したが {other:?} だった"),
    }
    // 初回の試行 + 再試行 1 回 = 2 回でそれ以上は再試行しない。
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn done_マーカーが無くてもストリーム終了で_finished_を出す() {
    let server = MockServer::start().await;
    // [DONE] を送らず、通常どおり（Content-Length が実体と一致した状態で）接続を閉じる。
    let body = format!(
        "data: {}\n\n",
        json!({"choices": [{"delta": {"content": "最後まで"}, "finish_reason": "stop"}]})
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;

    let client = client_for(&server, |c| c);
    let completion = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(completion.content, "最後まで");
    assert_eq!(completion.finish.reason, FinishReason::Stop);
}

/// 生の TCP で応答を返す簡易サーバーを起動し、その `base_url` を返す。
///
/// `wiremock` は常に整合の取れた HTTP 応答しか作れないため、「接続はできるが
/// 応答が得られない（＝接続失敗）」や「本文の途中で接続が切れる」を再現するには、
/// ソケットを直接あつかう必要がある。
///
/// `responses` の要素ごとに 1 つの接続を受け付ける。`None` ならバイト列を書かずに
/// 即座にソケットを閉じる（接続はできたが応答が得られない状況を模す）。`Some` なら
/// そのバイト列をそのまま書いてから閉じる。`responses` を使い切った後の接続は、
/// 最後の要素を繰り返し使う（reqwest/hyper が内部で余分に接続し直すことがあっても
/// 応答できるようにするため）。
///
/// 応答を書く前にリクエストのヘッダーを読み切っておく。読まずに接続を閉じると、
/// 受信バッファにデータが残ったままの切断とみなされ、OS が正常な切断（FIN）ではなく
/// 強制切断（RST）を送ることがあり、クライアント側から見て意図と違う失敗になるため。
#[allow(clippy::unwrap_used)]
fn spawn_raw_http_server(responses: Vec<Option<Vec<u8>>>) -> String {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let addr = std_listener.local_addr().unwrap();
    let listener = TcpListener::from_std(std_listener).unwrap();
    tokio::spawn(async move {
        let mut index = 0usize;
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            read_request_headers(&mut socket).await;

            let response = responses
                .get(index)
                .or_else(|| responses.last())
                .cloned()
                .flatten();
            index += 1;
            if let Some(bytes) = response {
                let _ = socket.write_all(&bytes).await;
                let _ = socket.shutdown().await;
            }
            // None の場合は何も書かずに socket を drop し、接続を切る。
        }
    });
    format!("http://{addr}/v1")
}

/// ソケットから、リクエストヘッダーの終端（空行）が来るまで読み捨てる。
async fn read_request_headers(socket: &mut tokio::net::TcpStream) {
    let mut buffer = [0u8; 1024];
    loop {
        match socket.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(n) if buffer[..n].windows(4).any(|w| w == b"\r\n\r\n") => return,
            Ok(_) => {}
        }
    }
}

#[tokio::test]
async fn 接続に失敗した後に成功すれば再試行で成功する() {
    let success_body = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"choices": [{"delta": {"content": "復帰"}, "finish_reason": "stop"}]})
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{success_body}",
        success_body.len(),
    );

    // 1 回目は何も応答せずに切断し（接続失敗）、2 回目で正しく応答する。
    let base_url = spawn_raw_http_server(vec![None, Some(response.into_bytes())]);
    let client = OpenAiCompatClient::new(ClientConfig {
        base_url,
        model: "test-model".to_string(),
        ..ClientConfig::default()
    })
    .unwrap();

    let completion = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(completion.content, "復帰");
}

#[tokio::test]
async fn 本文の途中で接続が切れるとエラーになる() {
    let partial = format!(
        "data: {}\n\n",
        json!({"choices": [{"delta": {"content": "途中まで"}}]})
    );
    // 実際に送るより大きい Content-Length を宣言し、約束したバイト数に届かないまま
    // 接続が切れる状況を作る。
    let declared_length = partial.len() + 200;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {declared_length}\r\n\r\n{partial}"
    );

    let base_url = spawn_raw_http_server(vec![Some(response.into_bytes())]);
    let client = OpenAiCompatClient::new(ClientConfig {
        base_url,
        model: "test-model".to_string(),
        ..ClientConfig::default()
    })
    .unwrap();

    let mut stream = client.stream_chat(request_with(vec![Message::user("hi")]));

    match stream.next().await {
        Some(Ok(ChatEvent::Content(text))) => assert_eq!(text, "途中まで"),
        other => panic!("Content イベントを期待したが {other:?} だった"),
    }
    match stream.next().await {
        Some(Err(LlmError::Connection { .. })) => {}
        other => panic!("Connection エラーを期待したが {other:?} だった"),
    }
    assert!(stream.next().await.is_none());
}

/// 秘密に見立てた認証情報とクエリを、テスト用サーバーの URL に書き足す。
fn with_secrets(base_url: &str) -> String {
    format!(
        "{}?api_key=query-secret",
        base_url.replacen("http://", "http://user:password-secret@", 1)
    )
}

/// エラーの文言を、元になったエラーの文言までつなげて返す（CLI は `{error:#}` でここまで出す）。
fn error_text_with_sources(error: &LlmError) -> String {
    let mut text = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        text.push_str(" / ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn assert_no_secrets(error: &LlmError, server: &str) {
    let text = error_text_with_sources(error);
    assert!(text.contains(server), "接続先のホスト名が無い: {text}");
    assert!(
        !text.contains("password-secret"),
        "認証情報が出ている: {text}"
    );
    assert!(!text.contains("query-secret"), "クエリが出ている: {text}");
}

#[tokio::test]
async fn 接続できないときのエラーに_url_の認証情報やクエリを出さない() {
    // 何も応答せずに切断し続けるサーバー
    let base_url = spawn_raw_http_server(vec![None]);
    let server = base_url
        .trim_start_matches("http://")
        .trim_end_matches("/v1")
        .to_owned();
    let client = OpenAiCompatClient::new(ClientConfig {
        base_url: with_secrets(&base_url),
        model: "test-model".to_string(),
        max_retries: 0,
        ..ClientConfig::default()
    })
    .unwrap();

    let error = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap_err();

    assert!(matches!(error, LlmError::Connection { .. }), "{error:?}");
    assert_no_secrets(&error, &server);
}

#[tokio::test]
async fn 本文の途中で切れたときのエラーに_url_の認証情報やクエリを出さない() {
    let partial = format!(
        "data: {}\n\n",
        json!({"choices": [{"delta": {"content": "途中まで"}}]})
    );
    let declared_length = partial.len() + 200;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {declared_length}\r\n\r\n{partial}"
    );
    let base_url = spawn_raw_http_server(vec![Some(response.into_bytes())]);
    let server = base_url
        .trim_start_matches("http://")
        .trim_end_matches("/v1")
        .to_owned();
    let client = OpenAiCompatClient::new(ClientConfig {
        base_url: with_secrets(&base_url),
        model: "test-model".to_string(),
        ..ClientConfig::default()
    })
    .unwrap();

    let error = collect(
        client.stream_chat(request_with(vec![Message::user("hi")])),
        |_| {},
    )
    .await
    .unwrap_err();

    assert!(matches!(error, LlmError::Connection { .. }), "{error:?}");
    assert_no_secrets(&error, &server);
}
