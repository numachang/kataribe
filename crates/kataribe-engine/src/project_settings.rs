//! 作品ごとの設定（`kataribe.yaml` の `settings`）。書いてある項目だけ、アプリ全体の設定を上書きする。
//!
//! 作品と一緒に持ち運べない値（接続先 URL・`claude` コマンドの場所などその PC に固有の値と、API キー）は持たない。

use std::collections::BTreeMap;

use kataribe_project::{ContentHash, Project, layout};
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};
use crate::settings::{DraftUnit, EngineSettings, LlmProvider};

/// 作品ごとの設定。どの項目も省略でき、省略した項目はアプリ全体の設定を使う。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct ProjectSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<LlmProvider>,
    /// OpenAI 互換 API のモデル。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Claude Code のモデル。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_unit: Option<DraftUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chars_per_call: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polish: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_thinking: Option<bool>,
    /// この版が知らない項目（新しい版で足された設定など）。保存し直しても消さない。
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    unknown: BTreeMap<String, serde_json::Value>,
}

impl ProjectSettings {
    /// 作品の `kataribe.yaml` から読む。`settings` が無ければ、何も上書きしない設定になる。
    pub fn load(project: &Project) -> Result<Self> {
        Ok(Self::load_with_hash(project)?.0)
    }

    /// [`Self::load`] に加えて、読んだときの `kataribe.yaml` のハッシュを返す。
    /// 画面で編集してから保存するまでの間の外での変更を、[`Self::save`] で競合として検出するのに使う。
    pub fn load_with_hash(project: &Project) -> Result<(Self, ContentHash)> {
        let (manifest, hash) = project.manifest_with_hash()?;
        Ok((Self::from_manifest_value(project, manifest.settings)?, hash))
    }

    /// 作品の `kataribe.yaml` に保存し、保存した後のハッシュを返す。
    ///
    /// - `expected`（[`Self::load_with_hash`] で得たハッシュ）を渡すと、そこから変わっていれば競合にする。
    /// - 数値の項目は、使うときと同じ範囲に収めてから保存する。
    /// - この版が知らない項目は、保存されているものを残す。
    /// - 何も上書きしない設定なら、`settings` の項目ごと消す。
    pub fn save(&self, project: &Project, expected: Option<&ContentHash>) -> Result<ContentHash> {
        let known = self.sanitized();
        let (_, hash) = project.update_manifest(expected, |manifest| -> Result<()> {
            let stored = Self::from_manifest_value(project, manifest.settings.take())?;
            let merged = Self {
                unknown: stored.unknown,
                ..known
            };
            manifest.settings = merged.to_manifest_value(project)?;
            Ok(())
        })?;
        Ok(hash)
    }

    /// アプリ全体の設定に、この設定で書いてある項目だけを重ね、値を安全な範囲に収める。
    #[must_use]
    pub fn apply(&self, base: EngineSettings) -> EngineSettings {
        let EngineSettings {
            mut llm,
            mut generation,
        } = base;
        overwrite(&mut llm.provider, self.provider.as_ref());
        overwrite(&mut llm.model, self.model.as_ref());
        overwrite(&mut llm.claude_model, self.claude_model.as_ref());
        overwrite(&mut generation.draft_unit, self.draft_unit.as_ref());
        overwrite(&mut generation.chars_per_call, self.chars_per_call.as_ref());
        overwrite(&mut generation.context_tokens, self.context_tokens.as_ref());
        overwrite(&mut generation.temperature, self.temperature.as_ref());
        overwrite(&mut generation.polish, self.polish.as_ref());
        overwrite(
            &mut generation.quality_retries,
            self.quality_retries.as_ref(),
        );
        overwrite(
            &mut generation.disable_thinking,
            self.disable_thinking.as_ref(),
        );
        EngineSettings {
            llm,
            generation: generation.sanitized(),
        }
    }

    /// 数値の項目を、使うとき（[`Self::apply`]）と同じ範囲に収める。書いていない項目はそのまま。
    fn sanitized(&self) -> Self {
        let generation = self.apply(EngineSettings::default()).generation;
        Self {
            chars_per_call: self.chars_per_call.map(|_| generation.chars_per_call),
            context_tokens: self.context_tokens.map(|_| generation.context_tokens),
            temperature: self.temperature.map(|_| generation.temperature),
            quality_retries: self.quality_retries.map(|_| generation.quality_retries),
            ..self.clone()
        }
    }

    fn from_manifest_value(project: &Project, value: Option<serde_json::Value>) -> Result<Self> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        serde_json::from_value(value).map_err(|error| invalid_settings(project, &error))
    }

    /// `kataribe.yaml` の `settings` に書く値。何も書く項目が無ければ `None`。
    fn to_manifest_value(&self, project: &Project) -> Result<Option<serde_json::Value>> {
        let value =
            serde_json::to_value(self).map_err(|error| invalid_settings(project, &error))?;
        Ok(match value {
            serde_json::Value::Object(object) if object.is_empty() => None,
            other => Some(other),
        })
    }
}

fn overwrite<T: Clone>(target: &mut T, value: Option<&T>) {
    if let Some(value) = value {
        target.clone_from(value);
    }
}

fn invalid_settings(project: &Project, error: &serde_json::Error) -> EngineError {
    EngineError::Settings {
        path: format!(
            "{} の settings",
            project.root().join(layout::MANIFEST).display()
        ),
        reason: format!(
            "{error}（目次の「作品情報」から kataribe.yaml を開いて、settings を直してください）"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{GenerationSettings, LlmSettings};
    use kataribe_project::{FORMAT_VERSION, Manifest, Rating};
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn project_with_settings(settings: Option<serde_json::Value>) -> (TempDir, Project) {
        let dir = TempDir::new().unwrap();
        let project = project_with_settings_in(&dir, settings);
        (dir, project)
    }

    fn project_with_settings_in(dir: &TempDir, settings: Option<serde_json::Value>) -> Project {
        let manifest = Manifest {
            format: FORMAT_VERSION,
            title: "灯台守の娘".to_owned(),
            author: None,
            genre: "general".to_owned(),
            genre_note: None,
            rating: Rating::General,
            target_length: 6000,
            idea: "北の岬の灯台で…".to_owned(),
            settings,
            extra: BTreeMap::from([("memo".to_owned(), serde_json::json!("手で足した項目"))]),
        };
        Project::create(dir.path(), &manifest).unwrap()
    }

    fn claude_code_haiku() -> ProjectSettings {
        ProjectSettings {
            provider: Some(LlmProvider::ClaudeCode),
            claude_model: Some("haiku".to_owned()),
            context_tokens: Some(100_000),
            ..ProjectSettings::default()
        }
    }

    #[test]
    fn only_the_written_items_override_the_app_settings() {
        let base = EngineSettings::default();

        let applied = claude_code_haiku().apply(base.clone());

        assert_eq!(
            applied,
            EngineSettings {
                llm: LlmSettings {
                    provider: LlmProvider::ClaudeCode,
                    claude_model: "haiku".to_owned(),
                    ..base.llm
                },
                generation: GenerationSettings {
                    context_tokens: 100_000,
                    ..base.generation
                },
            }
        );
    }

    #[test]
    fn empty_settings_leave_the_app_settings_as_they_are() {
        let base = EngineSettings::default();

        assert_eq!(ProjectSettings::default().apply(base.clone()), base);
    }

    #[test]
    fn overridden_values_are_kept_within_safe_ranges() {
        let settings = ProjectSettings {
            chars_per_call: Some(5),
            temperature: Some(9.0),
            ..ProjectSettings::default()
        };

        let applied = settings.apply(EngineSettings::default());

        assert_eq!(applied.generation.chars_per_call, 200);
        assert!((applied.generation.temperature - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_project_without_settings_overrides_nothing() {
        let (_dir, project) = project_with_settings(None);

        assert_eq!(
            ProjectSettings::load(&project).unwrap(),
            ProjectSettings::default()
        );
    }

    #[test]
    fn saved_settings_are_loaded_back_and_the_manifest_keeps_its_other_items() {
        let (_dir, project) = project_with_settings(None);

        claude_code_haiku().save(&project, None).unwrap();

        assert_eq!(
            ProjectSettings::load(&project).unwrap(),
            claude_code_haiku()
        );
        let manifest = project.manifest().unwrap();
        assert_eq!(manifest.title, "灯台守の娘");
        assert_eq!(manifest.extra["memo"], serde_json::json!("手で足した項目"));
    }

    #[test]
    fn saving_keeps_items_this_version_does_not_know() {
        let (_dir, project) = project_with_settings(Some(serde_json::json!({
            "provider": "openai_compatible",
            "future_option": "新しい版の設定",
        })));

        claude_code_haiku().save(&project, None).unwrap();

        let saved = project.manifest().unwrap().settings.unwrap();
        assert_eq!(saved["provider"], "claude_code");
        assert_eq!(saved["future_option"], "新しい版の設定");
    }

    #[test]
    fn saving_empty_settings_removes_the_settings_item() {
        let (dir, project) = project_with_settings(Some(serde_json::json!({ "polish": true })));

        ProjectSettings::default().save(&project, None).unwrap();

        assert_eq!(project.manifest().unwrap().settings, None);
        let text = std::fs::read_to_string(dir.path().join(layout::MANIFEST)).unwrap();
        assert!(!text.contains("settings"), "{text}");
    }

    #[test]
    fn saving_refuses_when_the_file_changed_after_it_was_read() {
        let (_dir, project) = project_with_settings(None);
        let (_, read_hash) = ProjectSettings::load_with_hash(&project).unwrap();
        claude_code_haiku().save(&project, None).unwrap();

        let stale = ProjectSettings {
            polish: Some(true),
            ..ProjectSettings::default()
        };
        let error = stale.save(&project, Some(&read_hash)).unwrap_err();

        assert!(matches!(
            error,
            EngineError::Project(kataribe_project::ProjectError::Conflict { .. })
        ));
        assert_eq!(
            ProjectSettings::load(&project).unwrap(),
            claude_code_haiku()
        );
    }

    #[test]
    fn saving_with_the_hash_that_was_read_succeeds_and_returns_the_new_hash() {
        let (_dir, project) = project_with_settings(None);
        let (_, read_hash) = ProjectSettings::load_with_hash(&project).unwrap();

        let saved_hash = claude_code_haiku()
            .save(&project, Some(&read_hash))
            .unwrap();

        assert_ne!(saved_hash, read_hash);
        assert_eq!(
            ProjectSettings::load_with_hash(&project).unwrap().1,
            saved_hash
        );
    }

    #[test]
    fn numbers_are_kept_within_safe_ranges_when_saved() {
        let (_dir, project) = project_with_settings(None);
        let settings = ProjectSettings {
            chars_per_call: Some(50),
            quality_retries: Some(10),
            ..ProjectSettings::default()
        };

        settings.save(&project, None).unwrap();

        let saved = ProjectSettings::load(&project).unwrap();
        assert_eq!(saved.chars_per_call, Some(200));
        assert_eq!(saved.quality_retries, Some(5));
        assert_eq!(saved.temperature, None);
    }

    #[test]
    fn settings_sent_back_by_the_screen_keep_the_unknown_items_in_the_file() {
        let (_dir, project) = project_with_settings(Some(serde_json::json!({
            "polish": true,
            "future_option": "新しい版の設定",
        })));
        // 画面は受け取った JSON（知らない項目も入る）を編集して送り返す
        let sent_to_screen =
            serde_json::to_value(ProjectSettings::load(&project).unwrap()).unwrap();
        let mut edited = sent_to_screen.clone();
        edited["future_option"] = serde_json::json!("画面で変えたつもりの値");
        edited["polish"] = serde_json::json!(false);
        let received: ProjectSettings = serde_json::from_value(edited).unwrap();

        received.save(&project, None).unwrap();

        let saved = project.manifest().unwrap().settings.unwrap();
        assert_eq!(saved["future_option"], "新しい版の設定");
        assert_eq!(saved["polish"], false);
    }

    #[test]
    fn a_whole_number_temperature_written_by_hand_can_be_read() {
        let dir = TempDir::new().unwrap();
        let project = project_with_settings_in(&dir, None);
        let manifest_path = dir.path().join(layout::MANIFEST);
        let text = std::fs::read_to_string(&manifest_path).unwrap();
        std::fs::write(
            &manifest_path,
            format!("{text}settings:\n  temperature: 1\n"),
        )
        .unwrap();

        let settings = ProjectSettings::load(&project).unwrap();

        assert_eq!(settings.temperature, Some(1.0));
    }

    #[test]
    fn invalid_settings_are_reported_with_the_file() {
        let (_dir, project) =
            project_with_settings(Some(serde_json::json!({ "provider": "unknown_api" })));

        let error = ProjectSettings::load(&project).unwrap_err();

        let message = error.to_string();
        assert!(message.contains("kataribe.yaml の settings"), "{message}");
        assert!(message.contains("unknown_api"), "{message}");
        assert!(message.contains("作品情報"), "{message}");
    }
}
