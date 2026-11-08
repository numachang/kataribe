//! `codex exec` を起動して出力を読む部分を、決まった出力を返す偽の `codex`（バッチファイル）で確かめる。
//! 本物の `codex` は呼ばない（利用者の利用枠を使わないため）。
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt as _;
use kataribe_llm::{
    ChatEvent, ChatModel, ChatRequest, CodexConfig, CodexModel, FinishReason, LlmError, Message,
    ResponseFormat, collect,
};
use tempfile::TempDir;

const THREAD_STARTED: &str = r#"{"type":"thread.started","thread_id":"t"}"#;
const TURN_STARTED: &str = r#"{"type":"turn.started"}"#;
const PREAMBLE: &str = r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"確認します"}}"#;
const MESSAGE: &str = r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"雨が降っていた。"}}"#;
const REASONING: &str =
    r#"{"type":"item.completed","item":{"type":"reasoning","text":"静かな場面にする"}}"#;
const ERROR_ITEM: &str =
    r#"{"type":"item.completed","item":{"type":"error","message":"一時的な失敗"}}"#;
const TURN_COMPLETED: &str = r#"{"type":"turn.completed","usage":{"input_tokens":14692,"cached_input_tokens":8320,"output_tokens":2589,"reasoning_output_tokens":359}}"#;
const RECONNECTING: &str = r#"{"type":"error","message":"Reconnecting... 1/5"}"#;
const TURN_FAILED: &str = r#"{"type":"turn.failed","error":{"message":"usage limit reached"}}"#;
const SCHEMA_ERROR: &str = r#"{"type":"error","message":"{\"type\":\"error\",\"error\":{\"message\":\"Invalid schema for response_format\",\"type\":\"invalid_request_error\",\"param\":\"text.format.schema\",\"code\":\"invalid_json_schema\"},\"status\":400}"}"#;
const SCHEMA_FAILED: &str = r#"{"type":"turn.failed","error":{"message":"{\"type\":\"error\",\"error\":{\"message\":\"Invalid schema for response_format\",\"type\":\"invalid_request_error\",\"param\":\"text.format.schema\",\"code\":\"invalid_json_schema\"},\"status\":400}"}}"#;

/// `output` をそのまま標準出力に出し、`stderr` を標準エラー出力に出して `exit_code` で終わる偽の `codex`。
fn fake_codex(dir: &TempDir, output: &[&str], stderr: &str, exit_code: i32) -> String {
    let output_path = dir.path().join("output.jsonl");
    std::fs::write(&output_path, output.join("\n") + "\n").unwrap();
    let mut commands = format!("type \"{}\"\r\n", output_path.display());
    if !stderr.is_empty() {
        write!(commands, "echo {stderr} 1>&2\r\n").unwrap();
    }
    write!(commands, "exit /b {exit_code}\r\n").unwrap();
    fake_codex_script(dir, &commands)
}

/// `commands` を実行するバッチファイルを、偽の `codex` として置く。
fn fake_codex_script(dir: &TempDir, commands: &str) -> String {
    let script = dir.path().join("fake-codex.cmd");
    std::fs::write(&script, format!("@echo off\r\n{commands}")).unwrap();
    script.display().to_string()
}

/// `seconds` 秒ほど待つバッチファイルのコマンド（`timeout` は標準入力がないと使えないため ping で待つ）。
fn wait_command(seconds: u32) -> String {
    format!("ping -n {} 127.0.0.1 >nul\r\n", seconds + 1)
}

fn model(program: &str) -> CodexModel {
    CodexModel::new(CodexConfig {
        program: program.to_owned(),
        model: "gpt-6.1-sol".to_owned(),
        reasoning_effort: "high".to_owned(),
        ..CodexConfig::default()
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

fn json_request() -> ChatRequest {
    ChatRequest {
        response_format: Some(ResponseFormat::JsonSchema {
            name: "output".to_owned(),
            schema: serde_json::json!({"type": "object", "additionalProperties": false}),
        }),
        ..request()
    }
}

#[tokio::test]
async fn the_last_agent_message_becomes_the_content_and_the_usage_is_reported() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(
        &dir,
        &[THREAD_STARTED, TURN_STARTED, MESSAGE, TURN_COMPLETED],
        "",
        0,
    );

    let completion = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.content, "雨が降っていた。");
    assert_eq!(completion.finish.reason, FinishReason::Stop);
    let usage = completion.finish.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 14_692);
    assert_eq!(usage.completion_tokens, 2589);
}

#[tokio::test]
async fn earlier_agent_messages_are_not_mixed_into_the_content() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[PREAMBLE, MESSAGE, TURN_COMPLETED], "", 0);

    let completion = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.content, "雨が降っていた。");
}

#[tokio::test]
async fn reasoning_becomes_reasoning_and_not_content() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[REASONING, MESSAGE, TURN_COMPLETED], "", 0);

    let completion = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.reasoning, "静かな場面にする");
    assert_eq!(completion.content, "雨が降っていた。");
}

#[tokio::test]
async fn error_items_and_stderr_logs_do_not_fail_a_turn_that_completes() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(
        &dir,
        &[ERROR_ITEM, RECONNECTING, MESSAGE, TURN_COMPLETED],
        "ERROR something was logged",
        0,
    );

    let completion = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.content, "雨が降っていた。");
}

#[tokio::test]
async fn a_failed_turn_becomes_an_error_with_its_reason() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[MESSAGE, TURN_FAILED], "", 1);

    let error = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    assert!(
        matches!(&error, LlmError::Codex(message) if message.contains("usage limit reached")),
        "{error}"
    );
}

#[tokio::test]
async fn ending_without_a_completed_turn_is_an_error_that_explains_itself() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[MESSAGE], "boom", 3);

    let error = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("boom"), "{message}");
    assert!(message.contains('3'), "{message}");
}

#[tokio::test]
async fn ending_without_a_completed_turn_prefers_the_reported_error() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[RECONNECTING], "noise", 1);

    let error = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    assert!(error.to_string().contains("Reconnecting"), "{error}");
}

#[tokio::test]
async fn a_completed_turn_without_a_message_is_an_error() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[TURN_COMPLETED], "", 0);

    let error = collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap_err();

    assert!(error.to_string().contains("本文"), "{error}");
}

#[tokio::test]
async fn a_rejected_schema_is_reported_like_a_server_refusal() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[SCHEMA_ERROR, SCHEMA_FAILED], "", 1);

    let error = collect(model(&program).stream_chat(json_request()), |_| {})
        .await
        .unwrap_err();

    assert!(
        matches!(&error, LlmError::Status { status: 400, body } if body.contains("Invalid schema")),
        "{error}"
    );
}

#[tokio::test]
async fn the_json_is_returned_as_the_content_when_a_schema_is_requested() {
    let dir = TempDir::new().unwrap();
    let json_message =
        r#"{"type":"item.completed","item":{"type":"agent_message","text":"{\"name\":\"凛\"}"}}"#;
    let program = fake_codex(&dir, &[json_message, TURN_COMPLETED], "", 0);

    let completion = collect(model(&program).stream_chat(json_request()), |_| {})
        .await
        .unwrap();

    assert_eq!(completion.content, r#"{"name":"凛"}"#);
}

/// 受け取った引数と標準入力をファイルに書いてから、完了の出力を返す偽の `codex`。
fn recording_codex(dir: &TempDir) -> String {
    let output_path = dir.path().join("output.jsonl");
    std::fs::write(&output_path, format!("{MESSAGE}\n{TURN_COMPLETED}\n")).unwrap();
    fake_codex_script(
        dir,
        &format!(
            "echo %*> \"{args}\"\r\nfindstr \"^\" > \"{stdin}\"\r\ntype \"{output}\"\r\n",
            args = dir.path().join("args.txt").display(),
            stdin = dir.path().join("stdin.txt").display(),
            output = output_path.display(),
        ),
    )
}

#[tokio::test]
async fn the_prompt_goes_to_stdin_and_the_system_prompt_to_a_file() {
    let dir = TempDir::new().unwrap();
    let program = recording_codex(&dir);

    collect(model(&program).stream_chat(request()), |_| {})
        .await
        .unwrap();

    let args = std::fs::read_to_string(dir.path().join("args.txt")).unwrap();
    assert!(args.contains("model_instructions_file="), "{args}");
    assert!(args.contains("gpt-6.1-sol"), "{args}");
    assert!(args.contains("model_reasoning_effort="), "{args}");
    assert!(!args.contains("--output-schema"), "{args}");
    let stdin = std::fs::read_to_string(dir.path().join("stdin.txt")).unwrap();
    assert!(stdin.contains("雨の場面を書いてください。"), "{stdin}");
}

#[tokio::test]
async fn a_requested_schema_is_passed_as_a_file() {
    let dir = TempDir::new().unwrap();
    let program = recording_codex(&dir);

    collect(model(&program).stream_chat(json_request()), |_| {})
        .await
        .unwrap();

    let args = std::fs::read_to_string(dir.path().join("args.txt")).unwrap();
    assert!(args.contains("--output-schema"), "{args}");
}

#[tokio::test]
async fn silence_longer_than_the_idle_timeout_is_an_error() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex_script(&dir, &wait_command(5));
    let model = CodexModel::new(CodexConfig {
        program,
        idle_timeout: Duration::from_millis(500),
        ..CodexConfig::default()
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
    std::fs::write(&output_path, format!("{REASONING}\n")).unwrap();
    let marker = dir.path().join("still-running.txt");
    let program = fake_codex_script(
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
        Some(Ok(ChatEvent::Reasoning(_)))
    ));

    drop(stream);
    tokio::time::sleep(Duration::from_secs(3)).await;

    assert!(!marker.exists());
}

/// `codex.cmd`（npm 版）のように、バッチファイルの下で本体が動いている場合。
#[tokio::test]
async fn dropping_the_stream_also_stops_the_process_a_batch_file_started() {
    let dir = TempDir::new().unwrap();
    let output_path = dir.path().join("output.jsonl");
    std::fs::write(&output_path, format!("{REASONING}\n")).unwrap();
    let marker = dir.path().join("still-running.txt");
    let inner = dir.path().join("inner.cmd");
    std::fs::write(
        &inner,
        format!(
            "@echo off\r\n{}echo still running> \"{}\"\r\n",
            wait_command(2),
            marker.display()
        ),
    )
    .unwrap();
    let program = fake_codex_script(
        &dir,
        &format!(
            "type \"{}\"\r\ncmd /c \"{}\"\r\n",
            output_path.display(),
            inner.display()
        ),
    );
    let mut stream = model(&program).stream_chat(request());
    assert!(matches!(
        stream.next().await,
        Some(Ok(ChatEvent::Reasoning(_)))
    ));

    drop(stream);
    tokio::time::sleep(Duration::from_secs(4)).await;

    assert!(!marker.exists());
}

#[tokio::test]
async fn a_missing_codex_command_explains_how_to_fix_it() {
    let missing = Path::new("C:/no/such/folder/codex.exe")
        .display()
        .to_string();

    let mut stream = model(&missing).stream_chat(request());
    let first = stream.next().await.unwrap();

    assert!(
        matches!(&first, Err(LlmError::Codex(message)) if message.contains("見つかりません")),
        "{first:?}"
    );
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn models_are_listed_when_logged_in() {
    let dir = TempDir::new().unwrap();
    let catalog_path = dir.path().join("catalog.json");
    std::fs::write(
        &catalog_path,
        r#"{"models":[{"slug":"gpt-6.1-sol","visibility":"list","context_window":500000},{"slug":"hidden","visibility":"hide","context_window":1}]}"#,
    )
    .unwrap();
    // 引数の 1 つ目で、ログインの確認（login）とモデルの一覧（debug）を出し分ける
    let program = fake_codex_script(
        &dir,
        &format!(
            "if \"%1\"==\"login\" (echo Logged in using ChatGPT& exit /b 0)\r\ntype \"{}\"\r\n",
            catalog_path.display()
        ),
    );

    let models = model(&program).list_models().await.unwrap();

    let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
    assert_eq!(ids, vec!["gpt-6.1-sol"]);
    assert_eq!(models[0].context_length, Some(500_000));
}

#[tokio::test]
async fn listing_models_asks_to_log_in_when_logged_out() {
    let dir = TempDir::new().unwrap();
    let program = fake_codex(&dir, &[], "Not logged in", 1);

    let error = model(&program).list_models().await.unwrap_err();

    assert!(
        matches!(&error, LlmError::Codex(message) if message.contains("codex login")),
        "{error}"
    );
}

#[tokio::test]
async fn listing_models_reports_a_missing_codex_command() {
    let error = model("C:/no/such/folder/codex.exe")
        .list_models()
        .await
        .unwrap_err();

    assert!(error.to_string().contains("見つかりません"), "{error}");
}

#[tokio::test]
async fn nothing_is_started_until_the_stream_is_polled() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("started.txt");
    let program = fake_codex_script(&dir, &format!("echo started> \"{}\"\r\n", marker.display()));

    let stream = model(&program).stream_chat(request());
    drop(stream);

    assert!(!marker.exists());
}
