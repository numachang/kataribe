// コマンドの一覧（COMMAND_NAMES）は、許可の一覧のテストと共有するため src/command_names.rs に置く
include!("src/command_names.rs");

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMAND_NAMES));
    if let Err(error) = tauri_build::try_build(attributes) {
        println!("cargo:warning=Tauri のビルド設定の生成に失敗しました: {error}");
        std::process::exit(1);
    }
}
