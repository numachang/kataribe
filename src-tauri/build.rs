/// このアプリが画面へ公開する Tauri コマンド。
///
/// ここに列挙すると、`tauri_build` がコマンドごとの権限（`allow-<コマンド名>` /
/// `deny-<コマンド名>`）を自動生成する。実際に許可するかどうかは
/// `capabilities/default.json` 側で決める。
const COMMANDS: &[&str] = &[
    "load_settings",
    "save_settings",
    "set_api_key",
    "has_api_key",
    "list_models",
    "list_genres",
    "create_project",
    "open_project",
    "close_project",
    "overview",
    "pipeline",
    "read_file",
    "write_file",
    "text_stats",
    "parse_ruby",
    "analyze_quality",
    "generate",
    "cancel_generation",
    "apply_change_set",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(error) = tauri_build::try_build(attributes) {
        println!("cargo:warning=Tauri のビルド設定の生成に失敗しました: {error}");
        std::process::exit(1);
    }
}
