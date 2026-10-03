//! 画面の設定一式。`llm`・`generation` はヘッドレス CLI（kataribe-cli）と同じ設定ファイル
//! （[`kataribe_engine::default_settings_path`]）を共有する。CLI はここで足した `editor` や
//! `recent_projects` を単に無視して読む。
//!
//! `llm`・`generation` は本来 [`kataribe_engine::EngineSettings`] を丸ごと
//! `#[serde(flatten)]` で埋め込みたいところだが、現時点では `EngineSettings` に
//! `ts_rs::TS` の実装が無く、`ts` feature を有効にしてもこの型を導出できない
//! （`LlmSettings` / `GenerationSettings` 自体には実装がある）。そのため、ここでは
//! 同じ 2 つの型をそのまま 2 つのフィールドとして持つ。JSON の形（トップレベルに
//! `llm` / `generation` が並ぶ）は `EngineSettings` を使った場合と変わらないので、
//! 設定ファイルの共有には影響しない。`EngineSettings` に `ts` を実装できたら、
//! `#[serde(flatten)] pub engine: EngineSettings` に置き換えられる。

use std::path::Path;

use kataribe_engine::{GenerationSettings, LlmSettings};
use serde::{Deserialize, Serialize};

use crate::error::CommandError;

/// 「最近使った作品」として覚えておく件数の上限。
const MAX_RECENT_PROJECTS: usize = 10;

/// エディタの文字組み。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum FontStyle {
    Mincho,
    Gothic,
}

/// エディタ画面の見え方の設定。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct EditorPreferences {
    /// 縦書きかどうか。
    pub vertical: bool,
    pub font_style: FontStyle,
    pub font_size: u32,
    pub line_height: f64,
}

impl Default for EditorPreferences {
    fn default() -> Self {
        Self {
            vertical: false,
            font_style: FontStyle::Mincho,
            font_size: 18,
            line_height: 1.9,
        }
    }
}

/// 画面（`src-tauri`）が持つ設定一式。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AppSettings {
    pub llm: LlmSettings,
    pub generation: GenerationSettings,
    pub editor: EditorPreferences,
    /// 最近開いた作品フォルダ。新しい順、重複なし、最大 [`MAX_RECENT_PROJECTS`] 件。
    pub recent_projects: Vec<String>,
}

/// 設定ファイルを読む。ファイルが無い・壊れている場合の扱いは
/// [`kataribe_engine::load_settings`] に従う（無ければ既定値、壊れていればエラー）。
pub fn load(path: &Path) -> Result<AppSettings, CommandError> {
    kataribe_engine::load_settings(path).map_err(CommandError::from)
}

/// 設定ファイルに保存する。
pub fn save(path: &Path, settings: &AppSettings) -> Result<(), CommandError> {
    kataribe_engine::save_settings(path, settings).map_err(CommandError::from)
}

/// `folder` を最近使った作品の先頭に置く。既にあれば取り除いてから先頭に置き直し、
/// [`MAX_RECENT_PROJECTS`] 件を超えた古いものは切り捨てる。
pub fn record_recent_project(recent_projects: &mut Vec<String>, folder: &str) {
    recent_projects.retain(|entry| entry != folder);
    recent_projects.insert(0, folder.to_owned());
    recent_projects.truncate(MAX_RECENT_PROJECTS);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_engine::DraftUnit;
    use pretty_assertions::assert_eq;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let settings = load(&dir.path().join("none.json")).unwrap();
        assert_eq!(settings, AppSettings::default());
    }

    #[test]
    fn settings_round_trip_through_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut settings = AppSettings::default();
        settings.llm.model = "gemma".to_owned();
        settings.generation.draft_unit = DraftUnit::Beat;
        settings.editor.vertical = true;
        settings.editor.font_size = 20;
        settings.recent_projects = vec!["C:/novels/sample".to_owned()];

        save(&path, &settings).unwrap();
        let loaded = load(&path).unwrap();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn llm_and_generation_sit_at_the_top_level_of_the_json_like_the_cli_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        save(&path, &AppSettings::default()).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert!(json.get("llm").is_some(), "json was: {json}");
        assert!(json.get("generation").is_some(), "json was: {json}");
    }

    #[test]
    fn partial_file_fills_missing_values_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"editor":{"font_size":22}}"#).unwrap();

        let loaded = load(&path).unwrap();

        assert_eq!(loaded.editor.font_size, 22);
        assert!(!loaded.editor.vertical);
        assert_eq!(loaded.llm, LlmSettings::default());
        assert_eq!(loaded.generation, GenerationSettings::default());
        assert_eq!(loaded.recent_projects, Vec::<String>::new());
    }

    #[test]
    fn a_cli_only_file_without_screen_settings_still_loads() {
        // kataribe-cli が保存した、editor・recent_projects を持たないファイルを読める。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"llm":{"base_url":"http://localhost:1234/v1","model":"gemma"},"generation":{"draft_unit":"scene","chars_per_call":1500,"context_tokens":8192,"temperature":0.8,"polish":false,"quality_retries":1}}"#,
        )
        .unwrap();

        let loaded = load(&path).unwrap();

        assert_eq!(loaded.llm.model, "gemma");
        assert_eq!(loaded.editor, EditorPreferences::default());
    }

    #[test]
    fn record_recent_project_adds_new_entry_to_the_front() {
        let mut recent = vec!["b".to_owned(), "a".to_owned()];
        record_recent_project(&mut recent, "c");
        assert_eq!(recent, vec!["c", "b", "a"]);
    }

    #[test]
    fn record_recent_project_moves_an_existing_entry_to_the_front_without_duplicating() {
        let mut recent = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
        record_recent_project(&mut recent, "b");
        assert_eq!(recent, vec!["b", "a", "c"]);
    }

    #[test]
    fn record_recent_project_keeps_at_most_max_entries() {
        let mut recent: Vec<String> = (0..MAX_RECENT_PROJECTS)
            .map(|index| format!("project-{index}"))
            .collect();

        record_recent_project(&mut recent, "newest");

        assert_eq!(recent.len(), MAX_RECENT_PROJECTS);
        assert_eq!(recent.first(), Some(&"newest".to_owned()));
        assert!(!recent.contains(&format!("project-{}", MAX_RECENT_PROJECTS - 1)));
    }
}
