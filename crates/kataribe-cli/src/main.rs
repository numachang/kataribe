//! kataribe のヘッドレス実行(CLI)。処理は `kataribe_cli::run_cancellable` に置く。
//!
//! Ctrl+C の監視と、応答しなくなった処理を待たずに終了する処理はここに置く。ライブラリの
//! 中で `std::process::exit` を呼ぶと、変更案の適用（原稿への書き込み）の途中で終了しうるうえ、
//! テストもしにくくなるため（`kataribe_cli::run_cancellable` はトークンを受け取るだけにしてある）。

use std::process::ExitCode;

use kataribe_cli::{ApplyGuard, CancellationToken, EXIT_CANCELLED, StdConsole};

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
    let cancel = CancellationToken::new();
    let apply_guard = ApplyGuard::new();
    let ctrl_c_watcher = tokio::spawn(watch_for_ctrl_c(cancel.clone(), apply_guard.clone()));

    let code =
        kataribe_cli::run_cancellable(std::env::args_os(), &console, cancel, &apply_guard).await;
    ctrl_c_watcher.abort();
    code
}

/// Ctrl+C を受け取ったら `cancel` を発火する。プロセス全体で 1 つだけ動かす。
///
/// 標準入力の読み取りなど、中止を見ない処理の途中で Ctrl+C を受けても、その場では止まらない
/// ことがある。もう一度 Ctrl+C を受けたら、後片付けを待たずにその場でプロセスを終了する。
/// ただし、変更案の適用（原稿への書き込み）の途中だけは、原稿を失わないよう、
/// `apply_guard` を通じて完了を待ってから終了する。
async fn watch_for_ctrl_c(cancel: CancellationToken, apply_guard: ApplyGuard) {
    if tokio::signal::ctrl_c().await.is_err() {
        return;
    }
    cancel.cancel();
    if tokio::signal::ctrl_c().await.is_ok() {
        apply_guard.wait_until_idle().await;
        std::process::exit(i32::from(EXIT_CANCELLED));
    }
}
