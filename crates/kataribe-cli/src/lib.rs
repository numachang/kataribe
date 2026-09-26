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
use tokio_util::sync::CancellationToken;

use args::{Cli, Command};
pub use output::{BufferConsole, Console, StdConsole};

/// 中止(Ctrl+C)を表す終了コード。
const EXIT_CANCELLED: u8 = 130;
/// 失敗を表す終了コード。
const EXIT_FAILURE: u8 = 1;
/// 使い方の誤り(clap が検出したもの)を表す終了コード。
const EXIT_USAGE_ERROR: u8 = 2;

/// CLI 全体のエントリポイント。`args` はプログラム名を含む(`std::env::args_os()` と同じ形)。
pub async fn run<I, T>(args: I, console: &dyn Console) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => return report_usage_error(console, &error),
    };

    let cancel = CancellationToken::new();
    let ctrl_c_watcher = tokio::spawn(watch_for_ctrl_c(cancel.clone()));

    let outcome = dispatch(&cli, console, &cancel).await;
    ctrl_c_watcher.abort();

    match outcome {
        Ok(commands::Outcome::Success) => ExitCode::from(0),
        Ok(commands::Outcome::Cancelled) => ExitCode::from(EXIT_CANCELLED),
        Err(error) => {
            console.eprint(&format!("{error:#}\n"));
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
async fn watch_for_ctrl_c(cancel: CancellationToken) {
    if tokio::signal::ctrl_c().await.is_ok() {
        cancel.cancel();
    }
}

/// clap が検出した使い方の誤り(または `--help` / `--version`)を表示し、終了コードを返す。
fn report_usage_error(console: &dyn Console, error: &clap::Error) -> ExitCode {
    let rendered = error.render().to_string();
    if error.use_stderr() {
        console.eprint(&rendered);
    } else {
        console.print(&rendered);
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
}
