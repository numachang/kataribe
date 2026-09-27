//! エンジンの設定。GUI と CLI が同じ設定ファイルを共有する。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};

/// 本文を生成する単位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DraftUnit {
    /// 章の残りのシーンを 1 回でまとめて書く。
    Chapter,
    /// 1 シーンずつ書く。長いシーンは複数回に分けて書き継ぐ。
    Scene,
    /// シーンを展開（ビート）に分け、1 ビートずつ書く。ローカル LLM で書き比べて最も安定したので既定にする
    /// （分量が目標どおりに収まり、書き継ぎで前の段落を繰り返しにくい）。
    #[default]
    Beat,
}

/// LLM サーバーへの接続設定。API キーは OS の資格情報ストアに別に保存する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct LlmSettings {
    pub base_url: String,
    pub model: String,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1234/v1".to_owned(),
            model: String::new(),
        }
    }
}

/// 生成のしかたの設定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GenerationSettings {
    pub draft_unit: DraftUnit,
    /// 1 回の生成で書かせる目安の文字数。
    pub chars_per_call: u32,
    /// モデルに渡せる文脈の長さ（トークン）。
    pub context_tokens: u32,
    pub temperature: f64,
    /// 本文を書いたあとに推敲パスをかけるか。
    pub polish: bool,
    /// 品質チェックで重大な問題が見つかったときの再生成回数。
    pub quality_retries: u32,
    /// 推論モデルの「思考」を止める。ローカル LLM では思考に出力の上限を使い切って
    /// 本文が空になることがあるため、既定で止める。クラウドの API でエラーになる場合は切る。
    pub disable_thinking: bool,
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            draft_unit: DraftUnit::Beat,
            chars_per_call: 1500,
            context_tokens: 16_384,
            temperature: 0.8,
            polish: false,
            quality_retries: 1,
            disable_thinking: true,
        }
    }
}

impl GenerationSettings {
    /// 不正な値を安全な範囲に収める（設定ファイルが手で書き換えられても動くように）。
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.chars_per_call = self.chars_per_call.clamp(200, 20_000);
        self.context_tokens = self.context_tokens.clamp(2048, 1_000_000);
        self.temperature = if self.temperature.is_finite() {
            self.temperature.clamp(0.0, 2.0)
        } else {
            GenerationSettings::default().temperature
        };
        self.quality_retries = self.quality_retries.min(5);
        self
    }
}

/// エンジンが使う設定一式。アプリはこれに画面の設定を足して同じファイルに保存する。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct EngineSettings {
    pub llm: LlmSettings,
    pub generation: GenerationSettings,
}

/// 設定ファイルの既定の場所（Tauri のアプリ設定フォルダと同じ）。
pub fn default_settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(crate::APP_IDENTIFIER).join("settings.json"))
}

/// JSON の設定ファイルを読む。ファイルが無ければ既定値を返す。
/// `T` は `EngineSettings` か、それを含むアプリの設定。未知の項目は無視する。
pub fn load_settings<T>(path: &Path) -> Result<T>
where
    T: Default + for<'de> Deserialize<'de>,
{
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(error) => return Err(settings_error(path, &error)),
    };
    serde_json::from_str(&text).map_err(|error| settings_error(path, &error))
}

/// JSON の設定ファイルを書く。書き込み途中で落ちても壊れないよう、一時ファイルから置き換える。
pub fn save_settings<T: Serialize>(path: &Path, settings: &T) -> Result<()> {
    let json =
        serde_json::to_string_pretty(settings).map_err(|error| settings_error(path, &error))?;
    let parent = path
        .parent()
        .ok_or_else(|| settings_error(path, &"親フォルダがありません"))?;
    std::fs::create_dir_all(parent).map_err(|error| settings_error(path, &error))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, json).map_err(|error| settings_error(path, &error))?;
    std::fs::rename(&temporary, path).map_err(|error| settings_error(path, &error))
}

fn settings_error(path: &Path, reason: &dyn std::fmt::Display) -> EngineError {
    EngineError::Settings {
        path: path.display().to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let settings: EngineSettings = load_settings(&dir.path().join("none.json")).unwrap();
        assert_eq!(settings, EngineSettings::default());
    }

    #[test]
    fn settings_round_trip_through_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let mut settings = EngineSettings::default();
        settings.llm.model = "gemma".to_owned();
        settings.generation.draft_unit = DraftUnit::Beat;

        save_settings(&path, &settings).unwrap();
        let loaded: EngineSettings = load_settings(&path).unwrap();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn partial_file_fills_missing_values_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"generation":{"chars_per_call":900},"editor":{"x":1}}"#,
        )
        .unwrap();

        let loaded: EngineSettings = load_settings(&path).unwrap();

        assert_eq!(loaded.generation.chars_per_call, 900);
        assert_eq!(loaded.generation.draft_unit, DraftUnit::Beat);
        assert_eq!(loaded.llm, LlmSettings::default());
    }

    #[test]
    fn broken_file_reports_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{").unwrap();

        let error = load_settings::<EngineSettings>(&path).unwrap_err();

        assert!(error.to_string().contains("settings.json"));
    }

    #[test]
    fn sanitized_clamps_out_of_range_values() {
        let settings = GenerationSettings {
            chars_per_call: 1,
            context_tokens: 10,
            temperature: f64::NAN,
            quality_retries: 99,
            ..GenerationSettings::default()
        }
        .sanitized();

        assert_eq!(settings.chars_per_call, 200);
        assert_eq!(settings.context_tokens, 2048);
        assert!(
            (settings.temperature - GenerationSettings::default().temperature).abs() < f64::EPSILON
        );
        assert_eq!(settings.quality_retries, 5);
    }
}
