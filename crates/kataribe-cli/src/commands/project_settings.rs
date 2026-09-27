//! `project-settings` サブコマンド: 作品ごとの設定（`kataribe.yaml` の `settings`）を表示・保存する。

use anyhow::Context;
use kataribe_engine::{EngineSettings, LlmProvider, ProjectSettings};
use kataribe_project::Project;

use crate::args::{GlobalOptions, ProjectSettingsArgs};
use crate::output::Console;
use crate::settings;

use super::Outcome;

pub fn run(
    args: &ProjectSettingsArgs,
    global: &GlobalOptions,
    console: &dyn Console,
) -> anyhow::Result<Outcome> {
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    if args.save {
        save(&project, global, console)?;
    }
    let stored = ProjectSettings::load(&project).context("作品の設定を読み込めません")?;
    let effective = settings::load_effective_settings(global, Some(&project))?;
    console
        .print(&render(&stored, &effective))
        .context("標準出力への書き込みに失敗しました")?;
    Ok(Outcome::Success)
}

/// コマンドラインで指定した値のうち、作品に保存できるものを保存する。
fn save(project: &Project, global: &GlobalOptions, console: &dyn Console) -> anyhow::Result<()> {
    if global.base_url.is_some() || global.claude_command.is_some() {
        // 注意を出せなくても保存はできるので、書き込みの失敗は無視する
        let _ = console.eprint(
            "--base-url と --claude-command は PC ごとの設定なので、作品には保存しません。\n",
        );
    }
    // --model は、保存したあとに使われる接続先のモデルとして保存する
    let provider = settings::load_effective_settings(global, Some(project))?
        .llm
        .provider;
    if with_overrides(ProjectSettings::default(), global, provider) == ProjectSettings::default() {
        let _ = console.eprint(
            "作品に保存できるオプション（--provider・--model・--unit など）が指定されていないため、何も変えません。\n",
        );
        return Ok(());
    }
    // 読んでから保存するまでの間に、画面や別の CLI が作品の設定を変えていたら、上書きせずに競合にする
    let (stored, read_hash) =
        ProjectSettings::load_with_hash(project).context("作品の設定を読み込めません")?;
    let updated = with_overrides(stored.clone(), global, provider);
    if updated == stored {
        let _ = console.eprint("作品の設定は、すでに指定した値のとおりです。\n");
        return Ok(());
    }
    updated
        .save(project, Some(&read_hash))
        .context("作品の設定を保存できません")?;
    Ok(())
}

/// `stored` に、コマンドラインで指定した値を重ねる。`--model` は `provider` のモデルとして扱う。
fn with_overrides(
    mut stored: ProjectSettings,
    global: &GlobalOptions,
    provider: LlmProvider,
) -> ProjectSettings {
    if let Some(provider) = global.provider {
        stored.provider = Some(provider.into());
    }
    if let Some(model) = &global.model {
        let target = match provider {
            LlmProvider::OpenaiCompatible => &mut stored.model,
            LlmProvider::ClaudeCode => &mut stored.claude_model,
        };
        *target = Some(model.clone());
    }
    if let Some(unit) = global.unit {
        stored.draft_unit = Some(unit.into());
    }
    if let Some(chars_per_call) = global.chars_per_call {
        stored.chars_per_call = Some(chars_per_call);
    }
    if let Some(context_tokens) = global.context_tokens {
        stored.context_tokens = Some(context_tokens);
    }
    if let Some(temperature) = global.temperature {
        stored.temperature = Some(temperature);
    }
    if global.polish {
        stored.polish = Some(true);
    }
    if global.no_polish {
        stored.polish = Some(false);
    }
    if let Some(quality_retries) = global.quality_retries {
        stored.quality_retries = Some(quality_retries);
    }
    stored
}

/// 作品の設定と、実際に使う設定を、`kataribe.yaml` と同じ項目名で並べる。
fn render(stored: &ProjectSettings, effective: &EngineSettings) -> String {
    let stored_lines = setting_lines(stored);
    let stored_section = if stored_lines.is_empty() {
        "  （なし。すべてアプリ全体の設定を使います）\n".to_owned()
    } else {
        stored_lines
    };
    format!(
        "作品の設定（kataribe.yaml の settings）:\n{stored_section}\n\
         実際に使う設定（アプリ全体の設定 ← 作品の設定 ← コマンドラインの指定）:\n{}",
        setting_lines(&as_project_settings(effective))
    )
}

/// 実際に使う設定を、作品の設定と同じ形（使う接続先のモデルだけを入れる）にする。
fn as_project_settings(effective: &EngineSettings) -> ProjectSettings {
    let (llm, generation) = (&effective.llm, &effective.generation);
    let mut settings = ProjectSettings::default();
    settings.provider = Some(llm.provider);
    match llm.provider {
        LlmProvider::OpenaiCompatible => settings.model = Some(llm.model.clone()),
        LlmProvider::ClaudeCode => settings.claude_model = Some(llm.claude_model.clone()),
    }
    settings.draft_unit = Some(generation.draft_unit);
    settings.chars_per_call = Some(generation.chars_per_call);
    settings.context_tokens = Some(generation.context_tokens);
    settings.temperature = Some(generation.temperature);
    settings.polish = Some(generation.polish);
    settings.quality_retries = Some(generation.quality_retries);
    settings.disable_thinking = Some(generation.disable_thinking);
    settings
}

/// 書いてある項目を 1 行ずつ `  項目: 値` の形にする。
fn setting_lines(settings: &ProjectSettings) -> String {
    let Ok(serde_json::Value::Object(items)) = serde_json::to_value(settings) else {
        return String::new();
    };
    items
        .iter()
        .map(|(key, value)| match value {
            serde_json::Value::String(text) => format!("  {key}: {text}\n"),
            other => format!("  {key}: {other}\n"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{DraftUnitArg, ProviderArg};
    use crate::output::BufferConsole;
    use crate::test_support::new_test_project;
    use kataribe_engine::DraftUnit;

    fn app_settings_file(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        path
    }

    #[test]
    fn saving_stores_only_the_given_options_and_sends_the_model_to_the_chosen_provider() {
        let global = GlobalOptions {
            provider: Some(ProviderArg::ClaudeCode),
            model: Some("haiku".to_owned()),
            unit: Some(DraftUnitArg::Scene),
            ..GlobalOptions::default()
        };

        let saved = with_overrides(ProjectSettings::default(), &global, LlmProvider::ClaudeCode);

        let mut expected = ProjectSettings::default();
        expected.provider = Some(LlmProvider::ClaudeCode);
        expected.claude_model = Some("haiku".to_owned());
        expected.draft_unit = Some(DraftUnit::Scene);
        assert_eq!(saved, expected);
    }

    #[test]
    fn no_polish_is_saved_as_false() {
        let global = GlobalOptions {
            no_polish: true,
            ..GlobalOptions::default()
        };

        let saved = with_overrides(
            ProjectSettings::default(),
            &global,
            LlmProvider::OpenaiCompatible,
        );

        assert_eq!(saved.polish, Some(false));
    }

    #[test]
    fn saving_without_any_storable_option_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("novel");
        new_test_project(&folder);
        let manifest_path = folder.join("kataribe.yaml");
        let before = std::fs::read_to_string(&manifest_path).unwrap();
        let global = GlobalOptions {
            settings: Some(app_settings_file(&dir)),
            ..GlobalOptions::default()
        };
        let console = BufferConsole::new();
        let args = ProjectSettingsArgs { folder, save: true };

        run(&args, &global, &console).unwrap();

        assert!(console.stderr().contains("何も変えません"));
        assert_eq!(std::fs::read_to_string(&manifest_path).unwrap(), before);
    }

    #[test]
    fn saving_a_value_the_project_already_has_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("novel");
        let project = new_test_project(&folder);
        let mut stored = ProjectSettings::default();
        stored.draft_unit = Some(DraftUnit::Beat);
        stored.save(&project, None).unwrap();
        let global = GlobalOptions {
            settings: Some(app_settings_file(&dir)),
            unit: Some(DraftUnitArg::Beat),
            ..GlobalOptions::default()
        };
        let console = BufferConsole::new();
        let args = ProjectSettingsArgs { folder, save: true };

        run(&args, &global, &console).unwrap();

        assert!(
            console.stderr().contains("すでに指定した値のとおり"),
            "{}",
            console.stderr()
        );
    }

    #[test]
    fn saving_keeps_what_the_project_already_has() {
        let mut stored = ProjectSettings::default();
        stored.context_tokens = Some(100_000);
        let global = GlobalOptions {
            temperature: Some(0.5),
            ..GlobalOptions::default()
        };

        let saved = with_overrides(stored, &global, LlmProvider::OpenaiCompatible);

        assert_eq!(saved.context_tokens, Some(100_000));
        assert_eq!(saved.temperature, Some(0.5));
    }

    #[test]
    fn save_then_show_lists_the_project_settings_and_the_effective_ones() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("novel");
        new_test_project(&folder);
        let global = GlobalOptions {
            settings: Some(app_settings_file(&dir)),
            provider: Some(ProviderArg::ClaudeCode),
            model: Some("haiku".to_owned()),
            base_url: Some("http://localhost:9999/v1".to_owned()),
            ..GlobalOptions::default()
        };
        let console = BufferConsole::new();
        let args = ProjectSettingsArgs {
            folder: folder.clone(),
            save: true,
        };

        run(&args, &global, &console).unwrap();

        let output = console.stdout();
        assert!(output.contains("作品の設定（kataribe.yaml の settings）:\n  claude_model: haiku\n  provider: claude_code\n"), "{output}");
        assert!(output.contains("実際に使う設定"), "{output}");
        assert!(console.stderr().contains("--base-url"));
        let saved = ProjectSettings::load(&Project::open(&folder).unwrap()).unwrap();
        assert_eq!(saved.provider, Some(LlmProvider::ClaudeCode));
        assert_eq!(saved.claude_model.as_deref(), Some("haiku"));
    }

    #[test]
    fn showing_a_project_without_settings_says_the_app_settings_are_used() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("novel");
        new_test_project(&folder);
        let global = GlobalOptions {
            settings: Some(app_settings_file(&dir)),
            ..GlobalOptions::default()
        };
        let console = BufferConsole::new();
        let args = ProjectSettingsArgs {
            folder: folder.clone(),
            save: false,
        };

        run(&args, &global, &console).unwrap();

        assert!(
            console
                .stdout()
                .contains("（なし。すべてアプリ全体の設定を使います）")
        );
        assert_eq!(
            ProjectSettings::load(&Project::open(&folder).unwrap()).unwrap(),
            ProjectSettings::default()
        );
    }
}
