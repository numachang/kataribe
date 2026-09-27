//! 現在の接続設定・API キーから LLM クライアントと執筆エンジンを作る。
//!
//! 設定や API キーが変わっても、呼び出しのたびに現在の値から作り直すだけなので、
//! 古い接続先やキーを使い続けることはない（キャッシュを持たない）。

use kataribe_engine::{
    ApiKeyStore, Engine, EngineSettings, GenerationSettings, LlmSettings, ProjectSettings,
};
use kataribe_llm::ModelInfo;
use kataribe_project::Project;

use crate::error::CommandError;
use crate::settings::AppSettings;

/// 生成に使う設定。作品を開いていれば、アプリ全体の設定に作品の設定（`kataribe.yaml`）を重ねる。
///
/// 作品の設定は毎回ファイルから読む。外で `kataribe.yaml` を直しても、次の生成から反映される。
pub fn effective_settings(
    app: &AppSettings,
    project: Option<&Project>,
) -> Result<EngineSettings, CommandError> {
    let base = EngineSettings {
        llm: app.llm.clone(),
        generation: app.generation.clone(),
    };
    let Some(project) = project else {
        return Ok(base);
    };
    Ok(ProjectSettings::load(project)?.apply(base))
}

/// 接続先が API キーを使うときだけ、保存済みの API キーを読む（使わないときは資格情報ストアに触らない）。
pub fn load_api_key(
    store: &ApiKeyStore,
    llm: &LlmSettings,
) -> Result<Option<String>, CommandError> {
    if llm.provider.uses_api_key() {
        store.load().map_err(CommandError::from)
    } else {
        Ok(None)
    }
}

/// 接続設定・生成設定・API キーから執筆エンジンを作る。
pub fn build_engine(
    llm: &LlmSettings,
    generation: &GenerationSettings,
    api_key: Option<String>,
) -> Result<Engine, CommandError> {
    let model = kataribe_engine::build_chat_model(llm, api_key)?;
    Engine::new(model, generation.clone()).map_err(CommandError::from)
}

/// 選べるモデルの一覧。
pub async fn list_models(
    llm: &LlmSettings,
    api_key: Option<String>,
) -> Result<Vec<ModelInfo>, CommandError> {
    kataribe_engine::list_models(llm, api_key)
        .await
        .map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use kataribe_engine::LlmProvider;
    use kataribe_project::{FORMAT_VERSION, Manifest, Rating};
    use tempfile::TempDir;

    fn project_with_settings(dir: &TempDir, settings: Option<serde_json::Value>) -> Project {
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
            extra: std::collections::BTreeMap::new(),
        };
        Project::create(dir.path(), &manifest).unwrap()
    }

    #[test]
    fn without_a_project_the_app_settings_are_used() {
        let app = AppSettings::default();

        let settings = effective_settings(&app, None).unwrap();

        assert_eq!(settings.llm, app.llm);
        assert_eq!(settings.generation, app.generation);
    }

    #[test]
    fn the_project_settings_override_the_app_settings() {
        let dir = TempDir::new().unwrap();
        let project = project_with_settings(
            &dir,
            Some(serde_json::json!({ "provider": "claude_code", "claude_model": "haiku" })),
        );
        let app = AppSettings::default();

        let settings = effective_settings(&app, Some(&project)).unwrap();

        assert_eq!(settings.llm.provider, LlmProvider::ClaudeCode);
        assert_eq!(settings.llm.claude_model, "haiku");
        assert_eq!(settings.llm.base_url, app.llm.base_url);
        assert_eq!(settings.generation, app.generation);
    }

    #[test]
    fn invalid_project_settings_are_an_error() {
        let dir = TempDir::new().unwrap();
        let project =
            project_with_settings(&dir, Some(serde_json::json!({ "draft_unit": "page" })));

        let error = effective_settings(&AppSettings::default(), Some(&project)).unwrap_err();

        assert!(error.message.contains("settings"), "{}", error.message);
    }

    #[test]
    fn build_engine_rejects_a_base_url_without_a_scheme() {
        let llm = LlmSettings {
            base_url: "localhost:1234/v1".to_owned(),
            ..LlmSettings::default()
        };

        let error = build_engine(&llm, &GenerationSettings::default(), None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Llm);
    }

    #[test]
    fn build_engine_succeeds_with_a_valid_connection_setting() {
        let llm = LlmSettings::default();
        let generation = GenerationSettings::default();

        assert!(build_engine(&llm, &generation, None).is_ok());
    }
}
