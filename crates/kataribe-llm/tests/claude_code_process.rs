//! `claude -p` を起動して出力を読む部分を、決まった出力を返す偽の `claude`（バッチファイル）で確かめる。
//! 本物の `claude` は呼ばない（利用者の Claude の利用枠を使わないため）。
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt as _;
use kataribe_llm::{
    ChatEvent, ChatModel, ChatRequest, ClaudeCodeConfig, ClaudeCodeModel, FinishReason, LlmError,
    Message, ResponseFormat, collect,
};
use tempfile::TempDir;

const TEXT_DELTA_1: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"雨が"}}}"#;
const TEXT_DELTA_2: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"降っていた。"}}}"#;
const THINKING_DELTA: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"静かな場面にする"}}}"#;
const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"stop_reason":"end_turn","result":"雨が降っていた。","usage":{"input_tokens":120,"output_tokens":8}}"#;
const STRUCTURED_RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"stop_reason":"tool_use","result":"{\"name\":\"凛\"}","structured_output":{"name":"凛"},"usage":{"input_tokens":10,"output_tokens":5}}"#;
const ERROR_RESULT: &str = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login","usage":{"input_tokens":0,"output_tokens":0}}"#;

/// `output` をそのまま標準出力に出し、`stderr` を標準エラー出力に出して `exit_code` で終わる偽の `claude`。
fn fake_claude(dir: &TempDir, output: &[&str], stderr: &str, exit_code: i32) -> String {
    let output_path = dir.path().join("output.jsonl");
    std::fs::write(&output_path, output.join("\n") + "\n").unwrap();
    let mut commands = format!("type \"{}\"\r\n", output_path.display());
    if !stderr.is_empty() {
        write!(commands, "echo {stderr} 1>&2\r\n").unwrap();
    }
    write!(commands, "exit /b {exit_code}\r\n").unwrap();
    fake_claude_script(dir, &commands)
}

/// `commands` を実行するバッチファイルを、偽の `claude` として置く。
fn fake_claude_script(dir: &TempDir, commands: &str) -> String {
    let script = dir.path().join("fake-claude.cmd");
    std::fs::write(&script, format!("@echo off\r\n{commands}")).unwrap();
    script.display().to_string()
}

/// `seconds` 秒ほど待つバッチファイルのコマンド（`timeout` は標準入力がないと使えないため ping で待つ）。
fn wait_command(seconds: u32) -> String {
    format!("ping -n {} 127.0.0.1 >nul\r\n", seconds + 1)
}

fn model(program: &str) -> ClaudeCodeModel {
    ClaudeCodeModel::new(ClaudeCodeConfig {
        program: program.to_owned(),
        ..ClaudeCodeConfig::default()
    })
}

fn request() -> ChatRequest {
    ChatRequest {
        messages: vec![
            Message::system("あなたは小説家です。"),
            Message::user("雨の場面を書いてください。"),
        ],
        ..ChatRequest::default()
    }
}

#[tokio::test]
async fn streamed_text_becomes_content_and_the_result_finishes_the_stream() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[TEXT_DELTA_1, TEXT_DELTA_2, RESULT], "", 0);

    let completion = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.content, "雨が降っていた。");
    assert_eq!(completion.finish.reason, FinishReason::Stop);
    assert_eq!(completion.finish.usage.unwrap().prompt_tokens, 120);
}

#[tokio::test]
async fn thinking_becomes_reasoning_and_not_content() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(
        &dir,
        &[THINKING_DELTA, TEXT_DELTA_1, TEXT_DELTA_2, RESULT],
        "",
        0,
    );

    let completion = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.reasoning, "静かな場面にする");
    assert_eq!(completion.content, "雨が降っていた。");
}

#[tokio::test]
async fn the_result_becomes_content_when_no_text_was_streamed() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[RESULT], "", 0);

    let completion = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.content, "雨が降っていた。");
}

#[tokio::test]
async fn streamed_text_that_differs_from_the_result_is_an_error() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[TEXT_DELTA_1, TEXT_DELTA_1, RESULT], "", 0);

    let error = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    assert!(error.to_string().contains("一致しない"), "{error}");
}

#[tokio::test]
async fn only_the_json_becomes_content_when_a_schema_is_requested() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[TEXT_DELTA_1, STRUCTURED_RESULT], "", 0);
    let request = ChatRequest {
        response_format: Some(ResponseFormat::JsonSchema {
            name: "character".to_owned(),
            schema: serde_json::json!({"type": "object"}),
        }),
        ..request()
    };

    let completion = collect(model(&program).stream_chat(request), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.content, r#"{"name":"凛"}"#);
}

#[tokio::test]
async fn an_error_result_becomes_an_error() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[ERROR_RESULT], "", 1);

    let error = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    assert!(
        matches!(&error, LlmError::ClaudeCode(message) if message.contains("Not logged in")),
        "{error}"
    );
}

#[tokio::test]
async fn an_unreadable_line_ends_the_stream_with_an_error() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[TEXT_DELTA_1, "not json", RESULT], "", 0);
    let mut stream = model(&program).stream_chat(request());

    assert!(matches!(
        stream.next().await,
        Some(Ok(ChatEvent::Content(_)))
    ));
    let second = stream.next().await.unwrap();
    assert!(
        matches!(&second, Err(error) if error.to_string().contains("not json")),
        "{second:?}"
    );
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn ending_without_a_result_reports_the_error_output() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[TEXT_DELTA_1], "usage limit reached", 3);

    let error = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("usage limit reached"), "{message}");
    assert!(message.contains('3'), "{message}");
}

#[tokio::test]
async fn silence_longer_than_the_idle_timeout_is_an_error() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude_script(&dir, &wait_command(5));
    let model = ClaudeCodeModel::new(ClaudeCodeConfig {
        program,
        idle_timeout: Duration::from_millis(500),
        ..ClaudeCodeConfig::default()
    });

    let error = collect(model.stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    assert!(error.to_string().contains("出力がありません"), "{error}");
}

#[tokio::test]
async fn dropping_the_stream_stops_the_started_command() {
    let dir = TempDir::new().unwrap();
    let output_path = dir.path().join("output.jsonl");
    std::fs::write(&output_path, format!("{TEXT_DELTA_1}\n")).unwrap();
    let marker = dir.path().join("still-running.txt");
    let program = fake_claude_script(
        &dir,
        &format!(
            "type \"{}\"\r\n{}echo still running> \"{}\"\r\n",
            output_path.display(),
            wait_command(1),
            marker.display()
        ),
    );
    let mut stream = model(&program).stream_chat(request());
    assert!(matches!(
        stream.next().await,
        Some(Ok(ChatEvent::Content(_)))
    ));

    drop(stream);
    tokio::time::sleep(Duration::from_secs(3)).await;

    assert!(!marker.exists());
}

#[tokio::test]
async fn a_missing_claude_command_explains_how_to_fix_it() {
    let missing = Path::new("C:/no/such/folder/claude.exe")
        .display()
        .to_string();

    let mut stream = model(&missing).stream_chat(request());
    let first = stream.next().await.unwrap();

    assert!(
        matches!(&first, Err(LlmError::ClaudeCode(message)) if message.contains("見つかりません")),
        "{first:?}"
    );
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn models_are_listed_when_logged_in() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(
        &dir,
        &[r#"{"loggedIn":true,"authMethod":"claude.ai"}"#],
        "",
        0,
    );

    let models = model(&program).list_models().await.unwrap();

    let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
    assert_eq!(ids, vec!["sonnet", "opus", "haiku"]);
}

#[tokio::test]
async fn listing_models_asks_to_log_in_when_logged_out() {
    let dir = TempDir::new().unwrap();
    let program = fake_claude(&dir, &[r#"{"loggedIn":false}"#], "", 1);

    let error = model(&program).list_models().await.unwrap_err();

    assert!(
        matches!(&error, LlmError::ClaudeCode(message) if message.contains("/login")),
        "{error}"
    );
}

#[tokio::test]
async fn listing_models_reports_a_missing_claude_command() {
    let error = model("C:/no/such/folder/claude.exe")
        .list_models()
        .await
        .unwrap_err();

    assert!(error.to_string().contains("見つかりません"), "{error}");
}

#[tokio::test]
async fn nothing_is_started_until_the_stream_is_polled() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("started.txt");
    let program = fake_claude_script(&dir, &format!("echo started> \"{}\"\r\n", marker.display()));

    let stream = model(&program).stream_chat(request());
    drop(stream);

    assert!(!marker.exists());
}
