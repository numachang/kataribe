//! kataribe のヘッドレス実行(CLI)。処理は `kataribe_cli::run` に置く。

use std::process::ExitCode;

use kataribe_cli::StdConsole;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let console = StdConsole::new();
    kataribe_cli::run(std::env::args_os(), &console).await
}
