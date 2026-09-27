//! `kataribe_cli::run` を通した結合テスト。
//!
//! wiremock で `OpenAI` 互換の SSE を返す偽サーバーを立て、一時フォルダに対して
//! `new` → `run --until concept` → `status --json` → `generate style --dry-run`
//! の流れを検証する。実際の LM Studio には一切つながない。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

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
/// - `--settings` は一時フォルダの中の設定ファイルを指す。`--settings` を明示したときは
///   ファイルが無いとエラーになる仕様なので、常に空（既定値）の内容で実際に作っておく。
/// - `--api-key-env` は `PATH`(常に非空)を指す。空でない環境変数を用意することで、
///   `resolve_api_key` が資格情報マネージャー(実際の Windows 資格情報)を読みに行く
///   経路を通らないようにする。
fn common_args(server_uri: &str, temp_dir: &std::path::Path) -> Vec<String> {
    let settings_path = temp_dir.join("settings.json");
    if !settings_path.exists() {
        std::fs::write(&settings_path, "{}").unwrap();
    }
    vec![
        "kataribe-cli".to_owned(),
        "--settings".to_owned(),
        settings_path.display().to_string(),
        "--base-url".to_owned(),
        format!("{server_uri}/v1"),
        "--model".to_owned(),
        "test-model".to_owned(),
        "--api-key-env".to_owned(),
        "PATH".to_owned(),
    ]
}

/// `new` を実行して、一時フォルダの中に作品を作る。
async fn create_test_project(project_dir: &std::path::Path) {
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
    let code = kataribe_cli::run(new_args, &BufferConsole::new()).await;
    assert_eq!(code, std::process::ExitCode::from(0), "new に失敗");
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

#[tokio::test]
async fn instruction_without_revise_is_a_usage_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let console = BufferConsole::new();

    let args = [
        "kataribe-cli".to_owned(),
        "generate".to_owned(),
        temp_dir.path().display().to_string(),
        "concept".to_owned(),
        "--instruction".to_owned(),
        "無視されるはず".to_owned(),
    ];
    let code = kataribe_cli::run(args, &console).await;

    assert_eq!(code, std::process::ExitCode::from(2));
    assert!(console.stderr().contains("--instruction"));
}

#[tokio::test]
async fn revise_without_instruction_is_a_usage_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let console = BufferConsole::new();

    let args = [
        "kataribe-cli".to_owned(),
        "generate".to_owned(),
        temp_dir.path().display().to_string(),
        "revise:concept.md".to_owned(),
    ];
    let code = kataribe_cli::run(args, &console).await;

    assert_eq!(code, std::process::ExitCode::from(2));
    assert!(console.stderr().contains("--instruction"));
}

/// 13(a): Ctrl+C の代わりに `cancel` を発火させ、遅い応答の途中で中止したら
/// 終了コード 130 になり、何も書き込まれないことを確かめる。
#[tokio::test]
async fn cancelling_during_a_slow_response_exits_with_130_and_writes_nothing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;

    // 応答をわざと遅らせる（idle_timeout の既定値は 5 分なので、タイムアウトより先に中止が効く）。
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .mount(&server)
        .await;

    let console = BufferConsole::new();
    let cancel = kataribe_cli::CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        trigger.cancel();
    });

    let mut generate_args = common_args(&server.uri(), temp_dir.path());
    generate_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "concept".to_owned(),
    ]);
    let code = kataribe_cli::run_cancellable(generate_args, &console, cancel).await;

    assert_eq!(code, std::process::ExitCode::from(130));
    assert!(
        !project_dir.join("concept.md").is_file(),
        "中止したのに書き込まれている"
    );
}

/// 13(b): `--max-steps` に達したら、それ以上に候補があっても止まる。
#[tokio::test]
async fn run_stops_after_max_steps_even_though_more_work_is_ready() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;

    // 企画の応答しか用意しない。max-steps を超えて 2 つ目の工程を試みれば、
    // マッチする応答が無く engine がエラーになるので、このテストが失敗して気付ける。
    respond_once_with(
        &server,
        "# 企画\n## ログライン\n嵐の洋館で起きる密室殺人。\n",
    )
    .await;

    let console = BufferConsole::new();
    let mut run_args = common_args(&server.uri(), temp_dir.path());
    run_args.extend([
        "run".to_owned(),
        project_dir.display().to_string(),
        "--max-steps".to_owned(),
        "1".to_owned(),
    ]);
    let code = kataribe_cli::run(run_args, &console).await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "run に失敗: {}",
        console.stderr()
    );
    assert!(project_dir.join("concept.md").is_file());
    assert!(
        !project_dir.join("style.md").is_file(),
        "max-steps を超えて生成している"
    );
    assert!(
        console
            .stderr()
            .contains("指定した工程数に達したため停止します")
    );
}

/// 13(b): `--until` の段階まで完了したら、他に取りかかれる工程があっても止まる。
#[tokio::test]
async fn run_stops_at_until_stage_even_though_more_work_is_ready() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;

    respond_once_with(
        &server,
        "# 企画\n## ログライン\n嵐の洋館で起きる密室殺人。\n",
    )
    .await;

    let console = BufferConsole::new();
    let mut run_args = common_args(&server.uri(), temp_dir.path());
    run_args.extend([
        "run".to_owned(),
        project_dir.display().to_string(),
        "--until".to_owned(),
        "concept".to_owned(),
    ]);
    let code = kataribe_cli::run(run_args, &console).await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "run に失敗: {}",
        console.stderr()
    );
    assert!(project_dir.join("concept.md").is_file());
    assert!(
        !project_dir.join("style.md").is_file(),
        "until を超えて生成している"
    );
    assert!(console.stderr().contains("指定した段階まで完了しました"));
}

/// 13(c): dry-run では、既存ファイルの内容もハッシュ（＝内容そのもの）も変わらない。
/// また、変更案は一度しか表示されない（ストリーミング表示と二重にならない）。
#[tokio::test]
async fn dry_run_leaves_an_existing_file_completely_unchanged() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;

    respond_once_with(&server, "# 企画\n## ログライン\n最初の内容。\n").await;
    let mut first_args = common_args(&server.uri(), temp_dir.path());
    first_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "concept".to_owned(),
    ]);
    assert_eq!(
        kataribe_cli::run(first_args, &BufferConsole::new()).await,
        std::process::ExitCode::from(0)
    );
    let original = std::fs::read_to_string(project_dir.join("concept.md")).unwrap();

    respond_once_with(
        &server,
        "# 企画\n## ログライン\n書き換えられたはずの内容。\n",
    )
    .await;
    let console = BufferConsole::new();
    let mut dry_run_args = common_args(&server.uri(), temp_dir.path());
    dry_run_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "concept".to_owned(),
        "--dry-run".to_owned(),
    ]);
    let code = kataribe_cli::run(dry_run_args, &console).await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    let after = std::fs::read_to_string(project_dir.join("concept.md")).unwrap();
    assert_eq!(after, original, "dry-run なのに書き換わっている");
    assert_eq!(
        console
            .stdout()
            .matches("書き換えられたはずの内容。")
            .count(),
        1,
        "変更案は一度だけ表示されるはず: {}",
        console.stdout()
    );
}

/// 13(d): dry-run でない generate は既存ファイルを置き換え、バックアップを作る。
#[tokio::test]
async fn generate_without_dry_run_replaces_the_file_and_backs_it_up() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;

    respond_once_with(&server, "# 企画\n## ログライン\n最初の内容。\n").await;
    let mut first_args = common_args(&server.uri(), temp_dir.path());
    first_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "concept".to_owned(),
    ]);
    assert_eq!(
        kataribe_cli::run(first_args, &BufferConsole::new()).await,
        std::process::ExitCode::from(0)
    );

    respond_once_with(&server, "# 企画\n## ログライン\n書き換えた内容。\n").await;
    let console = BufferConsole::new();
    let mut second_args = common_args(&server.uri(), temp_dir.path());
    second_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "concept".to_owned(),
    ]);
    let code = kataribe_cli::run(second_args, &console).await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    let content = std::fs::read_to_string(project_dir.join("concept.md")).unwrap();
    assert!(content.contains("書き換えた内容。"));

    let project = kataribe_project::Project::open(&project_dir).unwrap();
    let backups = project
        .store()
        .backups(&kataribe_project::RelPath::new("concept.md").unwrap())
        .unwrap();
    assert!(!backups.is_empty(), "上書き時にバックアップが作られるはず");
}

/// 13(e): 生成中に外部で編集されて競合したら、何も書き込まずに失敗し、
/// 生成した内容を標準出力に表示して失わないようにする。
#[tokio::test]
async fn external_edit_during_generation_causes_a_conflict_and_writes_nothing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;

    let chunks = vec![json!({
        "choices": [{"delta": {"content": "# 企画\n## ログライン\n生成された内容。\n"}, "finish_reason": "stop"}]
    })];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(300))
                .set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let concept_path = project_dir.join("concept.md");
    let external_write_path = concept_path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        std::fs::write(&external_write_path, "外部から書かれた内容").unwrap();
    });

    let console = BufferConsole::new();
    let mut generate_args = common_args(&server.uri(), temp_dir.path());
    generate_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "concept".to_owned(),
    ]);
    let code = kataribe_cli::run(generate_args, &console).await;

    assert_eq!(code, std::process::ExitCode::from(1));
    assert_eq!(
        std::fs::read_to_string(&concept_path).unwrap(),
        "外部から書かれた内容",
        "競合したのに書き込まれている"
    );
    assert!(
        console.stdout().contains("生成された内容。"),
        "失った生成内容が表示されるはず: {}",
        console.stdout()
    );
}

/// 13(f): 書き出し先が作品フォルダの中を指していたら拒否する。
#[tokio::test]
async fn export_refuses_a_destination_inside_the_project_folder() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;

    let console = BufferConsole::new();
    let args = [
        "kataribe-cli".to_owned(),
        "export".to_owned(),
        project_dir.display().to_string(),
        "--output".to_owned(),
        project_dir.join("novel.txt").display().to_string(),
    ];
    let code = kataribe_cli::run(args, &console).await;

    assert_eq!(code, std::process::ExitCode::from(1));
    assert!(console.stderr().contains("作品フォルダの中"));
    assert!(!project_dir.join("novel.txt").exists());
}

/// 13(f): 作品フォルダの外なら、一時ファイル経由でアトミックに書き出せる。
#[tokio::test]
async fn export_writes_to_a_destination_outside_the_project_folder() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;
    let output_path = temp_dir.path().join("novel.txt");

    let console = BufferConsole::new();
    let args = [
        "kataribe-cli".to_owned(),
        "export".to_owned(),
        project_dir.display().to_string(),
        "--output".to_owned(),
        output_path.display().to_string(),
    ];
    let code = kataribe_cli::run(args, &console).await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    assert!(output_path.is_file());
}
