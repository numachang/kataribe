//! ログの初期化。保存先は Tauri のアプリログフォルダ（`AppHandle::path().app_log_dir()`）。
//!
//! フォルダが作れない・開けない場合は標準エラー出力にログを出す（ログが完全に消えるよりまし）。

use std::fs::{File, OpenOptions};
use std::sync::Mutex;

use tauri::{Manager, Runtime};
use tracing_subscriber::EnvFilter;

const LOG_FILE_NAME: &str = "kataribe.log";

/// tracing の購読者を初期化する。リリースビルドでも `warn` 以上のログは残す。
pub fn init<R: Runtime>(app: &tauri::App<R>) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_error| EnvFilter::new(default_level()));

    match open_log_file(app) {
        Some(file) => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .init(),
        None => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
}

fn default_level() -> &'static str {
    if cfg!(debug_assertions) {
        "info"
    } else {
        "warn"
    }
}

fn open_log_file<R: Runtime>(app: &tauri::App<R>) -> Option<File> {
    let dir = app.path().app_log_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG_FILE_NAME))
        .ok()
}
