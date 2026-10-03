//! kataribe のヘッドレス実行(CLI)。GUI と同じ執筆エンジン(`kataribe_engine`)を画面なしで動かす。
//!
//! `main.rs` は薄く保ち、実際の処理はここに置く。標準出力・標準エラー出力は
//! [`output::Console`] を介して行うので、テストでは実際の端末に触れずに検証できる。
//!
//! Ctrl+C の監視と、応答しなくなった処理を待たずにプロセスを終了する処理は `main.rs` が行う
//! （ライブラリの中で `std::process::exit` を呼ぶと、変更案の適用の途中で終了しうるうえ、
//! テストもしにくくなるため）。ここで公開する [`run_cancellable`] は、その代わりに実際の
//! Ctrl+C の発火手段（[`CancellationToken`]）を外から受け取るだけにしてある。

mod args;
mod commands;
mod output;
mod settings;
mod stage;
mod structure_args;
mod target_spec;
mod task_spec;
#[cfg(test)]
mod test_support;

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use tokio::sync::Mutex;

use args::{Cli, Command};
pub use output::{BufferConsole, Console, StdConsole};
/// テストから、実際の Ctrl+C の代わりに中止を発火させるための型（[`run_cancellable`] に渡す）。
pub use tokio_util::sync::CancellationToken;

/// 中止(Ctrl+C)を表す終了コード。
pub const EXIT_CANCELLED: u8 = 130;
/// 失敗を表す終了コード。
const EXIT_FAILURE: u8 = 1;
/// 使い方の誤り(clap が検出したもの、または引数の組み合わせが誤っているもの)を表す終了コード。
const EXIT_USAGE_ERROR: u8 = 2;

/// 変更案の適用中であることを示す、プロセス全体で共有する合図。
///
/// Ctrl+C を 2 回受けたときは、応答しなくなった処理を待たずにその場でプロセスを終了してよいが、
/// 変更案の適用（`ChangeSet::apply` → `ProjectStore::apply_changes`）の途中だけは、原稿を失わないよう
/// 完了を待ってから終了したい。`main.rs` はこの型を作って [`run_cancellable`] に渡し、2 回目の
/// Ctrl+C を受けたときに [`ApplyGuard::wait_until_idle`] で完了を待ってから終了する。
#[derive(Debug, Clone)]
pub struct ApplyGuard(Arc<Mutex<()>>);

impl ApplyGuard {
    /// 適用が進行中でない状態から始める。
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(())))
    }

    /// 適用の区間に入る。戻り値を保持している間は [`Self::wait_until_idle`] を待たせる。
    pub(crate) async fn enter(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.0.lock().await
    }

    /// 適用が進行中なら、終わるまで待つ。
    pub async fn wait_until_idle(&self) {
        drop(self.0.lock().await);
    }
}

impl Default for ApplyGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// [`run_cancellable`] の中身。`args` はプログラム名を含む(`std::env::args_os()` と同じ形)。
///
/// `cancel` が発火する（または既に発火済みの）と、実行中のサブコマンドを中止する。
/// `apply_guard` は変更案を適用するサブコマンド（`generate` / `run`）に引き渡され、
/// 適用している間だけ保持される。
pub async fn run_cancellable<I, T>(
    args: I,
    console: &dyn Console,
    cancel: CancellationToken,
    apply_guard: &ApplyGuard,
) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => return report_usage_error(console, &error),
    };
    if let Err(message) = cli.command.validate() {
        let _ = console.eprint(&format!("{message}\n"));
        return ExitCode::from(EXIT_USAGE_ERROR);
    }

    // サブコマンドの処理と「中止された」を競わせる。中止が勝てば、まだ途中の処理はここで
    // drop されて打ち切られる（LLM へのストリームも、drop で HTTP 接続ごと閉じる）。
    // どのサブコマンドでも同じ経路を通るので、個々のコマンドが `cancel` を見ていなくても
    // （例: models のようにネットワーク呼び出しだけの単純なコマンド）中止が効く。
    let outcome = tokio::select! {
        biased;
        () = cancel.cancelled() => Ok(commands::Outcome::Cancelled),
        outcome = dispatch(&cli, console, &cancel, apply_guard) => outcome,
    };

    match outcome {
        Ok(commands::Outcome::Success) => ExitCode::from(0),
        Ok(commands::Outcome::Cancelled) => {
            // 中止の経路はいくつもある（この `select!` 自体が先に中止を受け取ることもあれば、
            // 各サブコマンドが `EngineError::Cancelled` や標準入力の読み取り後に気付くこともある）。
            // メッセージはここで一度だけ出す。
            let _ = console.eprint("中止しました。\n");
            ExitCode::from(EXIT_CANCELLED)
        }
        Err(error) => {
            let _ = console.eprint(&format!("{error:#}\n"));
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

async fn dispatch(
    cli: &Cli,
    console: &dyn Console,
    cancel: &CancellationToken,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<commands::Outcome> {
    match &cli.command {
        Command::New(args) => commands::new::run(args, console),
        Command::Status(args) => commands::status::run(args, &cli.global, console),
        Command::Generate(args) => {
            commands::generate::run(args, &cli.global, console, cancel, apply_guard).await
        }
        Command::Run(args) => {
            commands::run::run(args, &cli.global, console, cancel, apply_guard).await
        }
        Command::Add(args) => commands::add::run(args, console, apply_guard).await,
        Command::Remove(args) => commands::remove::run(args, console, apply_guard).await,
        Command::Move(args) => commands::reorder::run(args, console, apply_guard).await,
        Command::Quality(args) => commands::quality::run(args, console),
        Command::Export(args) => commands::export::run(args, console),
        Command::Models => commands::models::run(&cli.global, console).await,
        Command::ProjectSettings(args) => {
            commands::project_settings::run(args, &cli.global, console)
        }
        Command::ApiKey(args) => commands::api_key::run(args, console, cancel),
    }
}

/// clap が検出した使い方の誤り(または `--help` / `--version`)を表示し、終了コードを返す。
fn report_usage_error(console: &dyn Console, error: &clap::Error) -> ExitCode {
    let rendered = error.render().to_string();
    if error.use_stderr() {
        let _ = console.eprint(&rendered);
    } else {
        let _ = console.print(&rendered);
    }
    let code = u8::try_from(error.exit_code()).unwrap_or(EXIT_USAGE_ERROR);
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::Notify;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    use super::*;

    /// 実行中のサブコマンドに触れないテストで使う、常に発火しないトークンと未使用の合図。
    fn no_cancel() -> (CancellationToken, ApplyGuard) {
        (CancellationToken::new(), ApplyGuard::new())
    }

    #[tokio::test]
    async fn unknown_subcommand_is_a_usage_error() {
        let console = BufferConsole::new();
        let (cancel, apply_guard) = no_cancel();
        let code = run_cancellable(
            ["kataribe-cli", "no-such-command"],
            &console,
            cancel,
            &apply_guard,
        )
        .await;

        assert_eq!(code, ExitCode::from(EXIT_USAGE_ERROR));
        assert_ne!(console.stderr(), "");
    }

    #[tokio::test]
    async fn help_is_printed_to_stdout_with_exit_code_zero() {
        let console = BufferConsole::new();
        let (cancel, apply_guard) = no_cancel();
        let code =
            run_cancellable(["kataribe-cli", "--help"], &console, cancel, &apply_guard).await;

        assert_eq!(code, ExitCode::from(0));
        assert!(console.stdout().contains("Usage"));
    }

    #[tokio::test]
    async fn already_cancelled_token_skips_the_command_entirely() {
        let console = BufferConsole::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let code = run_cancellable(
            ["kataribe-cli", "models"],
            &console,
            cancel,
            &ApplyGuard::new(),
        )
        .await;

        assert_eq!(code, ExitCode::from(130));
        assert!(console.stderr().contains("中止しました"));
    }

    /// 既に中止済みのトークンを渡すだけでは、実行中に中止を受け取った経路は確かめられない。
    /// ここでは、応答を遅らせる偽サーバーへ実際にリクエストが届いてから中止を発火させることで、
    /// サブコマンドの実行中に Ctrl+C 相当の中止を受け取った経路を検証する。
    #[tokio::test]
    async fn cancelling_while_a_subcommand_is_running_stops_it_with_exit_code_130() {
        let server = MockServer::start().await;
        let received = Arc::new(Notify::new());
        let signal_on_request = received.clone();
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(move |_request: &Request| {
                signal_on_request.notify_one();
                ResponseTemplate::new(200).set_delay(Duration::from_secs(5))
            })
            .mount(&server)
            .await;

        let settings_dir = tempfile::tempdir().unwrap();
        let settings_path = settings_dir.path().join("settings.json");
        std::fs::write(&settings_path, "{}").unwrap();

        let console = BufferConsole::new();
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        tokio::spawn(async move {
            received.notified().await;
            trigger.cancel();
        });

        let code = run_cancellable(
            [
                "kataribe-cli".to_owned(),
                "--settings".to_owned(),
                settings_path.display().to_string(),
                "--base-url".to_owned(),
                format!("{}/v1", server.uri()),
                "--model".to_owned(),
                "test-model".to_owned(),
                "--api-key-env".to_owned(),
                "PATH".to_owned(),
                "models".to_owned(),
            ],
            &console,
            cancel,
            &ApplyGuard::new(),
        )
        .await;

        assert_eq!(code, ExitCode::from(130));
    }
}
