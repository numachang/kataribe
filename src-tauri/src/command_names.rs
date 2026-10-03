// このファイルは build.rs からも `include!` で読む（権限の生成と、許可の一覧のテストで同じ一覧を使うため）。
// そのため、ファイル先頭の `//!` の doc コメントは書けない。

/// このアプリが画面へ公開する Tauri コマンドの名前。
///
/// build.rs がこの一覧から、コマンドごとの権限（`allow-<コマンド名>` / `deny-<コマンド名>`）を生成する。
/// コマンドを足したら、次の 2 か所にも足すこと（足し忘れはテストが知らせる）。
/// - `lib.rs` の `generate_handler!`（コマンドの登録）
/// - `capabilities/default.json`（許可。無いと画面から呼んだときに「not allowed by ACL」になる）
pub const COMMAND_NAMES: &[&str] = &[
    "load_settings",
    "save_settings",
    "load_project_settings",
    "save_project_settings",
    "set_api_key",
    "has_api_key",
    "list_models",
    "list_genres",
    "create_project",
    "open_project",
    "close_project",
    "overview",
    "pipeline",
    "read_document",
    "write_document",
    "parse_document",
    "text_stats",
    "parse_ruby",
    "analyze_quality",
    "generate",
    "cancel_generation",
    "apply_change_set",
    "plan_structure_edit",
    "suggest_character_id",
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::COMMAND_NAMES;

    const LIB_SOURCE: &str = include_str!("lib.rs");
    const DEFAULT_CAPABILITY: &str = include_str!("../capabilities/default.json");

    fn command_names() -> BTreeSet<String> {
        COMMAND_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    }

    /// `generate_handler![...]` に並んでいる `commands::<名前>` の名前。
    fn registered_commands() -> BTreeSet<String> {
        let start = LIB_SOURCE
            .find("generate_handler![")
            .expect("lib.rs に generate_handler! があるはず");
        let end = start
            + LIB_SOURCE[start..]
                .find(']')
                .expect("generate_handler! の終わり");
        LIB_SOURCE[start..end]
            .split("commands::")
            .skip(1)
            .map(|entry| {
                entry
                    .trim_end_matches([',', ' ', '\n', '\r'])
                    .trim()
                    .to_owned()
            })
            .collect()
    }

    /// `capabilities/default.json` で許可しているこのアプリのコマンド（`allow-<名前>`）の名前。
    fn allowed_commands() -> BTreeSet<String> {
        let capability: serde_json::Value =
            serde_json::from_str(DEFAULT_CAPABILITY).expect("default.json は JSON のはず");
        capability["permissions"]
            .as_array()
            .expect("permissions は配列のはず")
            .iter()
            .filter_map(serde_json::Value::as_str)
            // `core:` や `dialog:` など、プラグインの権限は対象外
            .filter(|permission| !permission.contains(':'))
            .filter_map(|permission| permission.strip_prefix("allow-"))
            .map(|name| name.replace('-', "_"))
            .collect()
    }

    #[test]
    fn every_listed_command_is_registered_in_the_handler() {
        assert_eq!(registered_commands(), command_names());
    }

    #[test]
    fn every_listed_command_is_allowed_for_the_main_window() {
        assert_eq!(allowed_commands(), command_names());
    }
}
