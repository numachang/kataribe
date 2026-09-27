//! kataribe のヘッドレス実行(CLI)。GUI と同じ執筆エンジン(`kataribe_engine`)を画面なしで動かす。
//!
//! `main.rs` は薄く保ち、実際の処理はここに置く。標準出力・標準エラー出力は
//! [`output::Console`] を介して行うので、テストでは実際の端末に触れずに検証できる。

mod args;
mod commands;
mod output;
mod settings;
mod stage;
mod task_spec;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::Parser;

use args::{Cli, Command};
pub use output::{BufferConsole, Console, StdConsole};
/// テストから、実際の Ctrl+C の代わりに中止を発火させるための型（[`run_cancellable`] に渡す）。
pub use tokio_util::sync::CancellationToken;

/// 中止(Ctrl+C)を表す終了コード。
const EXIT_CANCELLED: u8 = 130;
/// 失敗を表す終了コード。
const EXIT_FAILURE: u8 = 1;
/// 使い方の誤り(clap が検出したもの、または引数の組み合わせが誤っているもの)を表す終了コード。
const EXIT_USAGE_ERROR: u8 = 2;

/// CLI 全体のエントリポイント。`args` はプログラム名を含む(`std::env::args_os()` と同じ形)。
///
/// Ctrl+C を受けるとサブコマンドの実行を中止する。中止の途中でもう一度 Ctrl+C を受けたら、
/// 応答しなくなった処理（標準入力の読み取りなど）を待ち続けず、その場で終了コード 130 を返す。
pub async fn run<I, T>(args: I, console: &dyn Console) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cancel = CancellationToken::new();
    let ctrl_c_watcher = tokio::spawn(watch_for_ctrl_c(cancel.clone()));

    let code = run_cancellable(args, console, cancel).await;
    ctrl_c_watcher.abort();
    code
}

/// [`run`] の中身。中止の発火手段を外から渡せるようにしたもの。
///
/// 実際の Ctrl+C の代わりに、テストが好きなタイミングで `cancel.cancel()` を呼んで
/// 中止の経路（終了コード 130、何も書き込まないこと）を検証できるようにするために分けている。
pub async fn run_cancellable<I, T>(
    args: I,
    console: &dyn Console,
    cancel: CancellationToken,
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
        outcome = dispatch(&cli, console, &cancel) => outcome,
    };

    match outcome {
        Ok(commands::Outcome::Success) => ExitCode::from(0),
        Ok(commands::Outcome::Cancelled) => ExitCode::from(EXIT_CANCELLED),
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
) -> anyhow::Result<commands::Outcome> {
    match &cli.command {
        Command::New(args) => commands::new::run(args, console),
        Command::Status(args) => commands::status::run(args, &cli.global, console),
        Command::Generate(args) => {
            commands::generate::run(args, &cli.global, console, cancel).await
        }
        Command::Run(args) => commands::run::run(args, &cli.global, console, cancel).await,
        Command::Quality(args) => commands::quality::run(args, console),
        Command::Export(args) => commands::export::run(args, console),
        Command::Models => commands::models::run(&cli.global, console).await,
        Command::ApiKey(args) => commands::api_key::run(args, console),
    }
}

/// Ctrl+C を受け取ったら `cancel` を発火する。プロセス全体で 1 つだけ動かす。
///
/// 標準入力の読み取りなど、中止を見ない処理の途中で Ctrl+C を受けても、その場では止まらない
/// ことがある。もう一度 Ctrl+C を受けたら、後片付けを待たずにその場でプロセスを終了する。
async fn watch_for_ctrl_c(cancel: CancellationToken) {
    if tokio::signal::ctrl_c().await.is_err() {
        return;
    }
    cancel.cancel();
    if tokio::signal::ctrl_c().await.is_ok() {
        std::process::exit(i32::from(EXIT_CANCELLED));
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
    use super::*;

    #[tokio::test]
    async fn unknown_subcommand_is_a_usage_error() {
        let console = BufferConsole::new();
        let code = run(["kataribe-cli", "no-such-command"], &console).await;

        assert_eq!(code, ExitCode::from(EXIT_USAGE_ERROR));
        assert!(!console.stderr().is_empty());
    }

    #[tokio::test]
    async fn help_is_printed_to_stdout_with_exit_code_zero() {
        let console = BufferConsole::new();
        let code = run(["kataribe-cli", "--help"], &console).await;

        assert_eq!(code, ExitCode::from(0));
        assert!(console.stdout().contains("Usage"));
    }

    #[tokio::test]
    async fn already_cancelled_token_skips_the_command_entirely() {
        let console = BufferConsole::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let code = run_cancellable(["kataribe-cli", "models"], &console, cancel).await;

        assert_eq!(code, ExitCode::from(130));
    }
}
