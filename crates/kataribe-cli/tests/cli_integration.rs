//! `kataribe_cli::run` を通した結合テスト。
//!
//! wiremock で `OpenAI` 互換の SSE を返す偽サーバーを立て、一時フォルダに対して
//! `new` → `run --until concept` → `status --json` → `generate style --dry-run`
//! の流れを検証する。実際の LM Studio には一切つながない。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use kataribe_cli::BufferConsole;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// `data:` チャンクの列から、`[DONE]` で終わる SSE 本文を組み立てる
/// (kataribe-llm の結合テストと同じ組み立て方)。
fn sse_body(chunks: &[Value]) -> String {
    let mut body = String::new();
    for chunk in chunks {
        body.push_str("data: ");
        body.push_str(&chunk.to_string());
        body.push_str("\n\n");
    }
    body.push_str("data: [DONE]\n\n");
    body
}

async fn respond_once_with(server: &MockServer, content: &str) {
    let chunks =
        vec![json!({"choices": [{"delta": {"content": content}, "finish_reason": "stop"}]})];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .up_to_n_times(1)
        .mount(server)
        .await;
}

/// 実 LM Studio や実行環境の設定ファイル・資格情報マネージャーに一切触れないための
/// 共通のグローバルオプション。
///
/// - `--settings` は存在しない場所を指し、常に既定値から始める。
/// - `--api-key-env` は `PATH`(常に非空)を指す。空でない環境変数を用意することで、
///   `resolve_api_key` が資格情報マネージャー(実際の Windows 資格情報)を読みに行く
///   経路を通らないようにする。
fn common_args(server_uri: &str, temp_dir: &std::path::Path) -> Vec<String> {
    vec![
        "kataribe-cli".to_owned(),
        "--settings".to_owned(),
        temp_dir.join("no-such-settings.json").display().to_string(),
        "--base-url".to_owned(),
        format!("{server_uri}/v1"),
        "--model".to_owned(),
        "test-model".to_owned(),
        "--api-key-env".to_owned(),
        "PATH".to_owned(),
    ]
}

#[tokio::test]
async fn full_flow_creates_a_project_and_generates_concept_then_previews_style() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;

    // 1. 新しい作品を作る。
    let new_console = BufferConsole::new();
    let new_args = [
        "kataribe-cli".to_owned(),
        "new".to_owned(),
        project_dir.display().to_string(),
        "--title".to_owned(),
        "みさき館の殺人".to_owned(),
        "--length".to_owned(),
        "6000".to_owned(),
        "--idea".to_owned(),
        "嵐で孤立した洋館で起きる密室殺人。".to_owned(),
    ];
    let code = kataribe_cli::run(new_args, &new_console).await;
    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "new に失敗: {}",
        new_console.stderr()
    );
    assert!(project_dir.join("kataribe.yaml").is_file());

    // 2. 企画が済むまで run する(この時点で必要な LLM 呼び出しは 1 回だけ)。
    respond_once_with(
        &server,
        "# 企画\n## ログライン\n嵐の洋館で起きる密室殺人。\n",
    )
    .await;
    let run_console = BufferConsole::new();
    let mut run_args = common_args(&server.uri(), temp_dir.path());
    run_args.extend([
        "run".to_owned(),
        project_dir.display().to_string(),
        "--until".to_owned(),
        "concept".to_owned(),
    ]);
    let code = kataribe_cli::run(run_args, &run_console).await;
    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "run に失敗: {}",
        run_console.stderr()
    );

    let concept = std::fs::read_to_string(project_dir.join("concept.md")).unwrap();
    assert!(
        concept.contains("嵐の洋館で起きる密室殺人。"),
        "concept was: {concept}"
    );

    // 3. status --json で企画の工程が済んだことを確認する。
    let status_console = BufferConsole::new();
    let mut status_args = common_args(&server.uri(), temp_dir.path());
    status_args.extend([
        "status".to_owned(),
        project_dir.display().to_string(),
        "--json".to_owned(),
    ]);
    let code = kataribe_cli::run(status_args, &status_console).await;
    assert_eq!(code, std::process::ExitCode::from(0));
    let payload: Value = serde_json::from_str(&status_console.stdout()).unwrap();
    let concept_step = payload["pipeline"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["task"]["kind"] == "concept")
        .unwrap();
    assert_eq!(concept_step["state"], "done");

    // 4. 文体ガイドはドライランで確認するだけにし、ファイルには書き込まない。
    respond_once_with(
        &server,
        "# 文体ガイド\n## 文体見本\n　雨の音が、屋敷の壁を叩いていた。\n",
    )
    .await;
    let generate_console = BufferConsole::new();
    let mut generate_args = common_args(&server.uri(), temp_dir.path());
    generate_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "style".to_owned(),
        "--dry-run".to_owned(),
    ]);
    let code = kataribe_cli::run(generate_args, &generate_console).await;
    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "generate に失敗: {}",
        generate_console.stderr()
    );
    assert!(
        generate_console
            .stdout()
            .contains("雨の音が、屋敷の壁を叩いていた。"),
        "stdout was: {}",
        generate_console.stdout()
    );
    assert!(
        !project_dir.join("style.md").is_file(),
        "dry-run なのに書き込まれている"
    );
}

#[tokio::test]
async fn generate_reports_missing_prerequisites_as_a_failure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    let console = BufferConsole::new();

    let new_args = [
        "kataribe-cli".to_owned(),
        "new".to_owned(),
        project_dir.display().to_string(),
        "--title".to_owned(),
        "みさき館の殺人".to_owned(),
        "--length".to_owned(),
        "6000".to_owned(),
        "--idea".to_owned(),
        "嵐で孤立した洋館で起きる密室殺人。".to_owned(),
    ];
    assert_eq!(
        kataribe_cli::run(new_args, &console).await,
        std::process::ExitCode::from(0)
    );

    // 企画が無いのに文体ガイドを生成しようとすると失敗する(LLM は 1 度も呼ばれない)。
    let mut generate_args = common_args(&server.uri(), temp_dir.path());
    generate_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "style".to_owned(),
    ]);
    let code = kataribe_cli::run(generate_args, &console).await;

    assert_eq!(code, std::process::ExitCode::from(1));
    assert!(console.stderr().contains("先に企画を生成してください"));
}

#[tokio::test]
async fn unknown_task_argument_is_a_usage_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let console = BufferConsole::new();

    let args = [
        "kataribe-cli".to_owned(),
        "generate".to_owned(),
        temp_dir.path().display().to_string(),
        "no-such-task".to_owned(),
    ];
    let code = kataribe_cli::run(args, &console).await;

    assert_eq!(code, std::process::ExitCode::from(2));
    assert!(console.stderr().contains("不明な工程です"));
}
