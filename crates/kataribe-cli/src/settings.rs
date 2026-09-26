//! 設定ファイルの読み込みと、グローバルオプションによる上書き・API キーの解決。

use std::path::PathBuf;

use anyhow::Context;
use kataribe_engine::{ApiKeyStore, EngineSettings};

use crate::args::GlobalOptions;

/// 設定ファイルの場所。`--settings` が無ければ GUI と同じ既定の場所を使う。
#[must_use]
pub fn settings_path(global: &GlobalOptions) -> Option<PathBuf> {
    global
        .settings
        .clone()
        .or_else(kataribe_engine::default_settings_path)
}

/// 設定ファイルを読み、グローバルオプションで上書きし、値を安全な範囲に収める。
pub fn load_effective_settings(global: &GlobalOptions) -> anyhow::Result<EngineSettings> {
    let mut settings = match settings_path(global) {
        Some(path) => {
            kataribe_engine::load_settings(&path).with_context(|| "設定ファイルを読み込めません")?
        }
        None => EngineSettings::default(),
    };
    apply_overrides(&mut settings, global);
    settings.generation = settings.generation.sanitized();
    Ok(settings)
}

/// グローバルオプションで指定された値だけ、設定ファイルの値を上書きする。
fn apply_overrides(settings: &mut EngineSettings, global: &GlobalOptions) {
    if let Some(base_url) = &global.base_url {
        settings.llm.base_url.clone_from(base_url);
    }
    if let Some(model) = &global.model {
        settings.llm.model.clone_from(model);
    }
    if let Some(unit) = global.unit {
        settings.generation.draft_unit = unit.into();
    }
    if let Some(chars_per_call) = global.chars_per_call {
        settings.generation.chars_per_call = chars_per_call;
    }
    if let Some(context_tokens) = global.context_tokens {
        settings.generation.context_tokens = context_tokens;
    }
    if let Some(temperature) = global.temperature {
        settings.generation.temperature = temperature;
    }
    if global.polish {
        settings.generation.polish = true;
    }
    if let Some(quality_retries) = global.quality_retries {
        settings.generation.quality_retries = quality_retries;
    }
}

/// API キーを、`--api-key-env` の環境変数 → 資格情報マネージャーの順で探す。
pub fn resolve_api_key(api_key_env: &str) -> anyhow::Result<Option<String>> {
    let from_env = std::env::var(api_key_env).ok();
    resolve_api_key_from(from_env.as_deref(), || ApiKeyStore::default().load())
}

/// [`resolve_api_key`] の中身。環境変数の取得元と資格情報ストアを差し替えられるようにして、
/// 本物の環境変数や資格情報マネージャーに触らずにテストできるようにする。
fn resolve_api_key_from(
    env_value: Option<&str>,
    load_from_store: impl FnOnce() -> kataribe_engine::Result<Option<String>>,
) -> anyhow::Result<Option<String>> {
    if let Some(value) = env_value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_owned()));
        }
    }
    load_from_store().with_context(|| "API キーを資格情報マネージャーから読み込めません")
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_engine::DraftUnit;

    fn overridden(global: &GlobalOptions) -> EngineSettings {
        let mut settings = EngineSettings::default();
        apply_overrides(&mut settings, global);
        settings
    }

    #[test]
    fn overrides_only_apply_when_present() {
        let settings = overridden(&GlobalOptions::default());
        assert_eq!(settings, EngineSettings::default());
    }

    #[test]
    fn overrides_replace_llm_and_generation_settings() {
        let global = GlobalOptions {
            base_url: Some("http://localhost:9999/v1".to_owned()),
            model: Some("gemma".to_owned()),
            unit: Some(crate::args::DraftUnitArg::Beat),
            chars_per_call: Some(500),
            context_tokens: Some(4096),
            temperature: Some(0.3),
            polish: true,
            quality_retries: Some(3),
            ..GlobalOptions::default()
        };
        let settings = overridden(&global);

        assert_eq!(settings.llm.base_url, "http://localhost:9999/v1");
        assert_eq!(settings.llm.model, "gemma");
        assert_eq!(settings.generation.draft_unit, DraftUnit::Beat);
        assert_eq!(settings.generation.chars_per_call, 500);
        assert_eq!(settings.generation.context_tokens, 4096);
        assert!((settings.generation.temperature - 0.3).abs() < f64::EPSILON);
        assert!(settings.generation.polish);
        assert_eq!(settings.generation.quality_retries, 3);
    }

    #[test]
    fn resolve_api_key_prefers_a_non_empty_environment_value_and_trims_it() {
        let resolved =
            resolve_api_key_from(Some("  from-env  "), || panic!("ストアに触れてはいけない"))
                .unwrap();
        assert_eq!(resolved.as_deref(), Some("from-env"));
    }

    #[test]
    fn resolve_api_key_falls_back_to_the_store_when_the_environment_value_is_blank() {
        let resolved =
            resolve_api_key_from(Some("   "), || Ok(Some("from-store".to_owned()))).unwrap();
        assert_eq!(resolved.as_deref(), Some("from-store"));
    }

    #[test]
    fn resolve_api_key_falls_back_to_the_store_when_the_environment_variable_is_unset() {
        let resolved = resolve_api_key_from(None, || Ok(None)).unwrap();
        assert_eq!(resolved, None);
    }
}
