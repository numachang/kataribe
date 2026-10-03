//! kataribe のデスクトップアプリ本体。
//!
//! この crate は engine（`kataribe-engine`）・project（`kataribe-project`）・
//! text（`kataribe-text`）を Tauri コマンドとして公開するだけの薄い層にする。
//! コマンドの実体は [`commands`] にあり、判断そのものは他のモジュールの、
//! Tauri に依存しない関数が行う（単体テストのため）。

// 一覧そのものは build.rs が使う。ここでは、登録・許可の一覧と食い違っていないかをテストで確かめる
#[cfg(test)]
mod command_names;
mod commands;
mod engine_client;
mod error;
mod files;
mod generation;
mod hashed_file;
mod launch_options;
mod logging;
mod projects;
mod settings;
mod state;
mod structure;

use tauri::Manager;

use crate::settings::AppSettings;
use crate::state::AppState;

/// アプリを起動し、ウィンドウが閉じられるまで動かす。
pub fn run() {
    if let Err(error) = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            logging::init(app);
            app.manage(build_initial_state()?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_settings,
            commands::save_settings,
            commands::load_project_settings,
            commands::save_project_settings,
            commands::set_api_key,
            commands::has_api_key,
            commands::list_models,
            commands::list_genres,
            commands::create_project,
            commands::open_project,
            commands::close_project,
            commands::overview,
            commands::pipeline,
            commands::read_document,
            commands::write_document,
            commands::parse_document,
            commands::text_stats,
            commands::parse_ruby,
            commands::analyze_quality,
            commands::generate,
            commands::cancel_generation,
            commands::apply_change_set,
            commands::plan_structure_edit,
            commands::suggest_character_id,
        ])
        .run(tauri::generate_context!())
    {
        tracing::error!(%error, "アプリケーションの起動に失敗しました");
        std::process::exit(1);
    }
}

/// 起動時の状態を作る。設定ファイルは `--settings` の指定、無ければ既定の場所（CLI と共有）を使う。
/// 設定ファイルを読み込めない場合は既定値で続ける
/// （利用者が最初の起動でいきなり使えなくなるのを避ける。詳細はログに残す）。
fn build_initial_state() -> Result<AppState, Box<dyn std::error::Error>> {
    let settings_path = match launch_options::settings_path_from_args(std::env::args_os().skip(1))?
    {
        Some(path) => path,
        None => kataribe_engine::default_settings_path().ok_or_else(|| {
            "設定ファイルの置き場所を決められません（ホームディレクトリが見つかりません）。"
                .to_owned()
        })?,
    };
    let settings = settings::load(&settings_path).unwrap_or_else(|error| {
        tracing::warn!(%error, "設定ファイルを読み込めなかったため、既定値で起動します");
        AppSettings::default()
    });
    Ok(AppState::new(settings_path, settings))
}
