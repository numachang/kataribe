//! `kataribe_cli::run_cancellable` を通した結合テスト。
//!
//! wiremock で `OpenAI` 互換の SSE を返す偽サーバーを立て、一時フォルダに対して
//! `new` → `run --until concept` → `status --json` → `generate style --dry-run`
//! の流れを検証する。実際の LM Studio には一切つながない。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use kataribe_cli::{ApplyGuard, BufferConsole, CancellationToken};
use serde_json::{Value, json};
use tokio::sync::Notify;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// `kataribe_cli::run_cancellable` を、中止しない前提の呼び出しのために簡単にしたもの。
/// 中止そのものを確かめるテストは、引き続き `run_cancellable` を直接使う。
async fn run<I, T>(args: I, console: &BufferConsole) -> std::process::ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    kataribe_cli::run_cancellable(args, console, CancellationToken::new(), &ApplyGuard::new()).await
}

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
    let code = run(new_args, &BufferConsole::new()).await;
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
    let code = run(new_args, &new_console).await;
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
    let code = run(run_args, &run_console).await;
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
    let code = run(status_args, &status_console).await;
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
    let code = run(generate_args, &generate_console).await;
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
        run(new_args, &console).await,
        std::process::ExitCode::from(0)
    );

    // 企画が無いのに文体ガイドを生成しようとすると失敗する(LLM は 1 度も呼ばれない)。
    let mut generate_args = common_args(&server.uri(), temp_dir.path());
    generate_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "style".to_owned(),
    ]);
    let code = run(generate_args, &console).await;

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
    let code = run(args, &console).await;

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
    let code = run(args, &console).await;

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
    let code = run(args, &console).await;

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
    // 中止しなければ本当にファイルへ書き込まれるはずの、実 SSE 本文を持たせる。そうしないと、
    // 応答が空のままでも(中止が効いていなくても)concept.md が作られず、この後の
    // 「何も書き込まれない」確認が中止の効果ではなく偶然で通ってしまう。
    let chunks = vec![json!({
        "choices": [{"delta": {"content": "# 企画\n## ログライン\n中止しなければ書かれる内容。\n"}, "finish_reason": "stop"}]
    })];
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_raw(sse_body(&chunks), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let console = BufferConsole::new();
    let cancel = CancellationToken::new();
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
    let code =
        kataribe_cli::run_cancellable(generate_args, &console, cancel, &ApplyGuard::new()).await;

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
    let code = run(run_args, &console).await;

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
    let code = run(run_args, &console).await;

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
        run(first_args, &BufferConsole::new()).await,
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
    let code = run(dry_run_args, &console).await;

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
        run(first_args, &BufferConsole::new()).await,
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
    let code = run(second_args, &console).await;

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
    let response_body = sse_body(&chunks);
    // 固定の待ち時間に頼ると、遅い実行環境では外部の書き込みが応答より先に終わる保証がない
    // （逆に速すぎる環境では意味のある競合にならない）。偽サーバーが実際にリクエストを受けた
    // 瞬間を合図にすることで、タイミングに関係なく「応答が返る前に外部で書き換わる」状況を
    // 確実に再現する。
    let request_received = Arc::new(Notify::new());
    let signal_on_request = request_received.clone();
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(move |_request: &Request| {
            signal_on_request.notify_one();
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(300))
                .set_body_raw(response_body.clone(), "text/event-stream")
        })
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let concept_path = project_dir.join("concept.md");
    let external_write_path = concept_path.clone();
    tokio::spawn(async move {
        request_received.notified().await;
        std::fs::write(&external_write_path, "外部から書かれた内容").unwrap();
    });

    let console = BufferConsole::new();
    let mut generate_args = common_args(&server.uri(), temp_dir.path());
    generate_args.extend([
        "generate".to_owned(),
        project_dir.display().to_string(),
        "concept".to_owned(),
    ]);
    let code = run(generate_args, &console).await;

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
    let code = run(args, &console).await;

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
    let code = run(args, &console).await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    assert!(output_path.is_file());
}

/// リクエストのたびに `contents` を順番に返す偽サーバーを立てる。`run` は工程ごとに別々の
/// `engine.generate` 呼び出しをするので、1 回のモックでは工程間の区切りを検証できない
/// （`respond_once_with` を 2 回呼んでも、どちらが先に消費されるかは wiremock の実装に依存する）。
async fn respond_with_sequence(server: &MockServer, contents: &[&str]) {
    let contents: Vec<String> = contents
        .iter()
        .map(|content| (*content).to_owned())
        .collect();
    let next_index = std::sync::atomic::AtomicUsize::new(0);
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(move |_request: &Request| {
            let index = next_index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let content = contents.get(index).cloned().unwrap_or_default();
            let chunks = vec![
                json!({"choices": [{"delta": {"content": content}, "finish_reason": "stop"}]}),
            ];
            ResponseTemplate::new(200).set_body_raw(sse_body(&chunks), "text/event-stream")
        })
        .mount(server)
        .await;
}

/// 13(g): `run` が続けて 2 つの工程を生成したとき、標準出力側にも工程の区切りが入ること。
/// 1 つ目の工程の本文はわざと改行で終わらせず、区切りの前に改行を補ってから空行を入れる
/// 規則も確かめる。
#[tokio::test]
async fn run_separates_stdout_between_consecutive_pipeline_steps_with_a_blank_line() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;

    respond_with_sequence(
        &server,
        &[
            "# 企画\n## ログライン\n嵐の洋館で起きる密室殺人。",
            "# 文体ガイド\n## 文体見本\n　雨の音が響く。\n",
        ],
    )
    .await;

    let console = BufferConsole::new();
    let mut run_args = common_args(&server.uri(), temp_dir.path());
    run_args.extend([
        "run".to_owned(),
        project_dir.display().to_string(),
        "--until".to_owned(),
        "style".to_owned(),
    ]);
    let code = run(run_args, &console).await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "run に失敗: {}",
        console.stderr()
    );
    let stdout = console.stdout();
    let concept_end = stdout
        .find("密室殺人。")
        .expect("企画の内容が出力されるはず")
        + "密室殺人。".len();
    let style_start = stdout
        .find("雨の音が響く。")
        .expect("文体ガイドの内容が出力されるはず");
    assert!(
        stdout[concept_end..style_start].contains("\n\n"),
        "工程の間に空行が入るはず: {stdout:?}"
    );
}

#[tokio::test]
async fn the_project_settings_choose_the_model_unless_the_command_line_overrides_it() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    let folder = project_dir.display().to_string();
    let server = MockServer::start().await;
    create_test_project(&project_dir).await;
    let settings_path = temp_dir.path().join("settings.json");
    std::fs::write(&settings_path, "{}").unwrap();
    let with_options = |options: &[&str]| -> Vec<String> {
        let mut args = vec![
            "kataribe-cli".to_owned(),
            "--settings".to_owned(),
            settings_path.display().to_string(),
            "--base-url".to_owned(),
            format!("{}/v1", server.uri()),
            "--api-key-env".to_owned(),
            "PATH".to_owned(),
        ];
        args.extend(options.iter().map(|option| (*option).to_owned()));
        args
    };

    let console = BufferConsole::new();
    let save = with_options(&[
        "--model",
        "project-model",
        "project-settings",
        &folder,
        "--save",
    ]);
    assert_eq!(
        run(save, &console).await,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );

    for options in [
        vec!["generate", &folder, "concept", "--dry-run"],
        vec![
            "--model",
            "cli-model",
            "generate",
            &folder,
            "concept",
            "--dry-run",
        ],
    ] {
        respond_once_with(
            &server,
            "# 企画\n## ログライン\n嵐の洋館で起きる密室殺人。\n",
        )
        .await;
        let console = BufferConsole::new();
        let code = run(with_options(&options), &console).await;
        assert_eq!(
            code,
            std::process::ExitCode::from(0),
            "{}",
            console.stderr()
        );
    }

    let models: Vec<String> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| {
            request.body_json::<Value>().unwrap()["model"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(models, vec!["project-model", "cli-model"]);
}

// ---- add / remove（人物・世界観の資料・シーンを自分で足したり消したりする） ----

/// `kataribe-cli <引数>` を実行する。LLM も設定ファイルも使わないコマンド用。
async fn run_plain(args: &[&str], console: &BufferConsole) -> std::process::ExitCode {
    let args = std::iter::once("kataribe-cli").chain(args.iter().copied());
    run(args, console).await
}

fn path_arg(path: &std::path::Path) -> String {
    path.display().to_string()
}

const CHAPTER_WITH_TWO_SCENES: &str = "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 二人が洋館に着く。\n    pov: 霧島 凛\n    characters:\n      - 霧島 凛\n  - id: s02\n    title: 閉ざされた書斎\n    summary: 書斎で死体が見つかる。\n    characters:\n      - 凛\n---\nストーリーライン\n";

/// 作品を作り、章立てを 1 つ置く。
async fn project_with_a_chapter(project_dir: &std::path::Path) {
    create_test_project(project_dir).await;
    std::fs::write(
        project_dir.join("plot/chapters/01.md"),
        CHAPTER_WITH_TWO_SCENES,
    )
    .unwrap();
}

#[tokio::test]
async fn add_character_makes_a_romaji_id_from_the_reading_and_reports_the_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;
    let console = BufferConsole::new();

    let code = run_plain(
        &[
            "add",
            "character",
            &path_arg(&project_dir),
            "--name",
            "霧島 凛",
            "--reading",
            "きりしま りん",
            "--role",
            "主人公",
            "--body",
            "## 口調\n静かに話す。\n",
        ],
        &console,
    )
    .await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    assert!(
        console
            .stderr()
            .contains("書き込み: characters/kirishima-rin.md")
    );
    let text = std::fs::read_to_string(project_dir.join("characters/kirishima-rin.md")).unwrap();
    assert!(text.contains("name: 霧島 凛"), "{text}");
    assert!(text.contains("role: 主人公"), "{text}");
    assert!(text.contains("order: 1"), "{text}");
    assert!(text.ends_with("## 口調\n静かに話す。\n"), "{text}");
}

#[tokio::test]
async fn add_with_dry_run_prints_the_change_and_writes_nothing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;
    let console = BufferConsole::new();

    let code = run_plain(
        &[
            "add",
            "world",
            &path_arg(&project_dir),
            "--title",
            "用語集",
            "--body",
            "霧：朝に出る。",
            "--dry-run",
        ],
        &console,
    )
    .await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    assert!(
        console.stdout().contains("=== world/doc.md ==="),
        "{}",
        console.stdout()
    );
    assert!(
        console.stdout().contains("# 用語集"),
        "{}",
        console.stdout()
    );
    assert!(!project_dir.join("world/doc.md").exists());
    assert!(!console.stderr().contains("書き込み:"));
}

#[tokio::test]
async fn add_world_reads_the_body_from_a_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;
    let body_file = temp_dir.path().join("body.txt");
    std::fs::write(&body_file, "霧：朝に出る。\r\n").unwrap();
    let console = BufferConsole::new();

    let code = run_plain(
        &[
            "add",
            "world",
            &path_arg(&project_dir),
            "--title",
            "用語集",
            "--name",
            "glossary",
            "--body-file",
            &path_arg(&body_file),
        ],
        &console,
    )
    .await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("world/glossary.md")).unwrap(),
        "# 用語集\n\n霧：朝に出る。\n"
    );
}

#[tokio::test]
async fn add_scene_inserts_before_the_given_scene_with_a_new_id() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    project_with_a_chapter(&project_dir).await;
    let console = BufferConsole::new();

    let code = run_plain(
        &[
            "add",
            "scene",
            &path_arg(&project_dir),
            "01",
            "--title",
            "屋根裏の足音",
            "--pov",
            "霧島 凛",
            "--characters",
            "霧島 凛,佐藤 健二",
            "--target-chars",
            "1500",
            "--before",
            "s02",
        ],
        &console,
    )
    .await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    assert!(console.stderr().contains("書き込み: plot/chapters/01.md"));
    let text = std::fs::read_to_string(project_dir.join("plot/chapters/01.md")).unwrap();
    let positions: Vec<usize> = ["id: s01", "id: s03", "id: s02"]
        .iter()
        .map(|id| {
            text.find(id)
                .unwrap_or_else(|| panic!("{id} が無い: {text}"))
        })
        .collect();
    assert!(positions.is_sorted(), "{text}");
    assert!(text.contains("佐藤 健二"), "{text}");
    assert!(text.contains("target_chars: 1500"), "{text}");
}

#[tokio::test]
async fn remove_character_moves_the_file_to_the_trash_and_names_the_scenes_that_mention_them() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    project_with_a_chapter(&project_dir).await;
    std::fs::write(
        project_dir.join("characters/rin.md"),
        "---\nname: 霧島 凛\n---\n本文\n",
    )
    .unwrap();
    let console = BufferConsole::new();

    let code = run_plain(
        &["remove", &path_arg(&project_dir), "character:rin"],
        &console,
    )
    .await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    let stderr = console.stderr();
    assert!(stderr.contains("characters/rin.md"), "{stderr}");
    assert!(stderr.contains(".kataribe/trash/"), "{stderr}");
    assert!(
        stderr.contains("s01「洋館への道」（視点・登場人物）"),
        "{stderr}"
    );
    assert!(
        stderr.contains("s02「閉ざされた書斎」（登場人物）"),
        "{stderr}"
    );
    assert!(stderr.contains("ゴミ箱へ: characters/rin.md"), "{stderr}");
    assert!(!project_dir.join("characters/rin.md").exists());
    let trashed = std::fs::read_dir(project_dir.join(".kataribe/trash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(trashed.join("characters/rin.md").is_file());
}

#[tokio::test]
async fn remove_scene_says_how_much_text_goes_to_the_trash() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    project_with_a_chapter(&project_dir).await;
    std::fs::create_dir_all(project_dir.join("manuscript/01")).unwrap();
    std::fs::write(
        project_dir.join("manuscript/01/s02.txt"),
        "書斎の扉は閉じていた。\n",
    )
    .unwrap();
    let console = BufferConsole::new();

    let code = run_plain(
        &["remove", &path_arg(&project_dir), "scene:01/s02"],
        &console,
    )
    .await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    let stderr = console.stderr();
    assert!(
        stderr.contains("manuscript/01/s02.txt（11 字）"),
        "{stderr}"
    );
    assert!(
        stderr.contains("ゴミ箱へ: manuscript/01/s02.txt"),
        "{stderr}"
    );
    assert!(stderr.contains("書き込み: plot/chapters/01.md"), "{stderr}");
    assert!(!project_dir.join("manuscript/01/s02.txt").exists());
    let chapter = std::fs::read_to_string(project_dir.join("plot/chapters/01.md")).unwrap();
    assert!(!chapter.contains("id: s02"), "{chapter}");
}

#[tokio::test]
async fn remove_with_dry_run_prints_the_plan_and_moves_nothing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;
    std::fs::write(project_dir.join("world/glossary.md"), "# 用語集\n").unwrap();
    let console = BufferConsole::new();

    let code = run_plain(
        &[
            "remove",
            &path_arg(&project_dir),
            "world:glossary",
            "--dry-run",
        ],
        &console,
    )
    .await;

    assert_eq!(
        code,
        std::process::ExitCode::from(0),
        "{}",
        console.stderr()
    );
    assert!(
        console
            .stdout()
            .contains("=== ゴミ箱へ移す: world/glossary.md ==="),
        "{}",
        console.stdout()
    );
    assert!(console.stderr().contains(".kataribe/trash/"));
    assert!(project_dir.join("world/glossary.md").is_file());
    assert!(!project_dir.join(".kataribe/trash").exists());
}

#[tokio::test]
async fn add_character_with_a_used_id_fails_and_leaves_the_file_alone() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;
    std::fs::write(
        project_dir.join("characters/rin.md"),
        "---\nname: 先にいた凛\n---\n",
    )
    .unwrap();
    let console = BufferConsole::new();

    let code = run_plain(
        &[
            "add",
            "character",
            &path_arg(&project_dir),
            "--name",
            "別の凛",
            "--id",
            "rin",
        ],
        &console,
    )
    .await;

    assert_eq!(code, std::process::ExitCode::from(1));
    assert!(
        console.stderr().contains("ID「rin」はもう使われています"),
        "{}",
        console.stderr()
    );
    let text = std::fs::read_to_string(project_dir.join("characters/rin.md")).unwrap();
    assert_eq!(text, "---\nname: 先にいた凛\n---\n");
}

#[tokio::test]
async fn remove_refuses_the_world_overview_and_a_missing_target() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-novel");
    create_test_project(&project_dir).await;
    std::fs::write(project_dir.join("world/overview.md"), "# 世界観\n").unwrap();

    let overview = BufferConsole::new();
    let overview_code = run_plain(
        &["remove", &path_arg(&project_dir), "world:overview"],
        &overview,
    )
    .await;
    let missing = BufferConsole::new();
    let missing_code = run_plain(
        &["remove", &path_arg(&project_dir), "character:nobody"],
        &missing,
    )
    .await;

    assert_eq!(overview_code, std::process::ExitCode::from(1));
    assert_eq!(missing_code, std::process::ExitCode::from(1));
    assert!(
        missing.stderr().contains("characters/nobody.md"),
        "{}",
        missing.stderr()
    );
    assert!(project_dir.join("world/overview.md").is_file());
}

#[tokio::test]
async fn bad_arguments_for_add_and_remove_are_usage_errors() {
    let temp_dir = tempfile::tempdir().unwrap();
    let folder = path_arg(temp_dir.path());
    for args in [
        vec!["remove", &folder, "chapter:01"],
        vec!["remove", &folder, "scene:01"],
        vec!["add", "character", &folder],
        vec!["add", "scene", &folder, "1", "--title", "題"],
        vec![
            "add",
            "world",
            &folder,
            "--title",
            "題",
            "--body",
            "a",
            "--body-file",
            "b",
        ],
        vec!["add", "nothing", &folder],
    ] {
        let console = BufferConsole::new();

        let code = run_plain(&args, &console).await;

        assert_eq!(
            code,
            std::process::ExitCode::from(2),
            "{args:?}: {}",
            console.stderr()
        );
    }
}
