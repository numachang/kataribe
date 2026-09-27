//! アプリ全体で共有する状態（`tauri::State`）。
//!
//! ロックは値の読み書きの間だけ持ち、`.await` をまたいでは持たない
//! （生成ジョブの実行など、時間のかかる `.await` の前には値を複製してロックを手放す）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use kataribe_engine::ApiKeyStore;
use kataribe_project::Project;

use crate::error::CommandError;
use crate::generation::{self, JobRegistry};
use crate::settings::{self, AppSettings};

/// アプリ全体で共有する状態。
pub struct AppState {
    /// 開いている作品。`None` なら何も開いていない。
    project: Mutex<Option<Arc<Project>>>,
    /// 実行中の生成ジョブ。
    jobs: JobRegistry,
    /// 現在の設定（設定ファイルの内容と一致させておく）。
    settings: Mutex<AppSettings>,
    /// 設定ファイルの場所。
    settings_path: PathBuf,
    /// API キーの保存場所（OS の資格情報ストア）。
    api_key_store: ApiKeyStore,
}

impl AppState {
    #[must_use]
    pub fn new(settings_path: PathBuf, settings: AppSettings) -> Self {
        Self {
            project: Mutex::new(None),
            jobs: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings),
            settings_path,
            api_key_store: ApiKeyStore::default(),
        }
    }

    #[must_use]
    pub fn settings(&self) -> AppSettings {
        self.lock_settings().clone()
    }

    /// 設定ファイルを読み直し、現在の設定にする。
    pub fn reload_settings(&self) -> Result<AppSettings, CommandError> {
        let mut current = self.lock_settings();
        let loaded = settings::load(&self.settings_path)?;
        current.clone_from(&loaded);
        Ok(loaded)
    }

    /// 画面で編集した設定を保存する。最近の作品の一覧は Rust 側が管理するので、画面の値では
    /// 上書きしない（画面が起動時に読んだ古い一覧で、その後に開いた作品を消さないため）。
    pub fn save_settings(&self, edited: AppSettings) -> Result<(), CommandError> {
        let mut current = self.lock_settings();
        let settings = AppSettings {
            recent_projects: current.recent_projects.clone(),
            ..edited
        };
        settings::save(&self.settings_path, &settings)?;
        *current = settings;
        Ok(())
    }

    /// `folder` を最近使った作品の先頭に置き、設定ファイルに保存する。
    ///
    /// 保存の前に設定ファイルを読み直し、読めなければ（壊れているなど）書かずにエラーを返す。
    /// 手で直している途中の設定や CLI が変えた設定を、メモリ上の値や既定値で上書きしないため。
    pub fn record_recent_project(&self, folder: &str) -> Result<(), CommandError> {
        let mut current = self.lock_settings();
        settings::record_recent_project(&mut current.recent_projects, folder);
        let mut on_disk = settings::load(&self.settings_path)?;
        on_disk.recent_projects.clone_from(&current.recent_projects);
        settings::save(&self.settings_path, &on_disk)?;
        *current = on_disk;
        Ok(())
    }

    fn lock_settings(&self) -> std::sync::MutexGuard<'_, AppSettings> {
        self.settings.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn api_key_store(&self) -> &ApiKeyStore {
        &self.api_key_store
    }

    #[must_use]
    pub fn project(&self) -> Option<Arc<Project>> {
        self.project
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 開いている作品。開いていなければ `not_found` エラーにする。
    pub fn require_project(&self) -> Result<Arc<Project>, CommandError> {
        self.project().ok_or_else(CommandError::project_not_open)
    }

    /// 作品を開く。前の作品の生成ジョブは中止する。
    pub fn set_project(&self, project: Project) {
        self.replace_project(Some(Arc::new(project)));
    }

    /// 作品を閉じる。実行中の生成ジョブは中止する。
    pub fn clear_project(&self) {
        self.replace_project(None);
    }

    /// 作品を入れ替える。前の作品のための生成を続けても、その変更案を使う先が無いので止める。
    fn replace_project(&self, project: Option<Arc<Project>>) {
        generation::cancel_all(&self.jobs);
        *self.project.lock().unwrap_or_else(PoisonError::into_inner) = project;
    }

    #[must_use]
    pub fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::Job;
    use kataribe_project::{FORMAT_VERSION, Manifest, Rating};
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn state_with_settings_file(dir: &TempDir, content: Option<&str>) -> AppState {
        let path = dir.path().join("settings.json");
        if let Some(content) = content {
            std::fs::write(&path, content).unwrap();
        }
        AppState::new(path, AppSettings::default())
    }

    fn saved_settings(dir: &TempDir) -> AppSettings {
        settings::load(&dir.path().join("settings.json")).unwrap()
    }

    fn sample_project(dir: &TempDir) -> Project {
        let manifest = Manifest {
            format: FORMAT_VERSION,
            title: "テスト作品".to_owned(),
            author: None,
            genre: "general".to_owned(),
            genre_note: None,
            rating: Rating::General,
            target_length: 10_000,
            idea: "静かな夜の物語".to_owned(),
            extra: std::collections::BTreeMap::new(),
        };
        Project::create(dir.path(), &manifest).unwrap();
        Project::open(dir.path()).unwrap()
    }

    #[test]
    fn saving_settings_keeps_the_recent_projects_recorded_since_startup() {
        let dir = TempDir::new().unwrap();
        let state = state_with_settings_file(&dir, None);
        state
            .record_recent_project("C:/novels/opened-later")
            .unwrap();

        // 画面は起動時に読んだ設定（最近の作品が空）を編集して送ってくる
        let mut edited = AppSettings::default();
        edited.llm.model = "gemma".to_owned();
        state.save_settings(edited).unwrap();

        let saved = saved_settings(&dir);
        assert_eq!(saved.llm.model, "gemma");
        assert_eq!(saved.recent_projects, vec!["C:/novels/opened-later"]);
        assert_eq!(state.settings(), saved);
    }

    #[test]
    fn recording_a_recent_project_keeps_settings_changed_by_others() {
        let dir = TempDir::new().unwrap();
        let state = state_with_settings_file(&dir, None);
        // CLI などが、アプリの起動後に設定ファイルを書き換えた
        let mut changed = AppSettings::default();
        changed.llm.model = "changed-by-cli".to_owned();
        settings::save(&dir.path().join("settings.json"), &changed).unwrap();

        state.record_recent_project("C:/novels/a").unwrap();

        let saved = saved_settings(&dir);
        assert_eq!(saved.llm.model, "changed-by-cli");
        assert_eq!(saved.recent_projects, vec!["C:/novels/a"]);
    }

    #[test]
    fn a_broken_settings_file_is_not_overwritten_when_recording_a_recent_project() {
        let dir = TempDir::new().unwrap();
        let broken = r#"{"llm": {"model": "gemma"},"#;
        let state = state_with_settings_file(&dir, Some(broken));

        assert!(state.record_recent_project("C:/novels/a").is_err());

        let on_disk = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert_eq!(on_disk, broken);
        assert_eq!(state.settings().recent_projects, vec!["C:/novels/a"]);
    }

    #[test]
    fn switching_or_closing_the_project_cancels_running_generation() {
        let (settings_dir, first_dir, second_dir) = (
            TempDir::new().unwrap(),
            TempDir::new().unwrap(),
            TempDir::new().unwrap(),
        );
        let state = state_with_settings_file(&settings_dir, None);
        state.set_project(sample_project(&first_dir));

        let job_for_first = Job::register(state.jobs(), "job-1").unwrap();
        state.set_project(sample_project(&second_dir));
        assert!(job_for_first.cancel_token().is_cancelled());

        let job_for_second = Job::register(state.jobs(), "job-2").unwrap();
        state.clear_project();
        assert!(job_for_second.cancel_token().is_cancelled());
        assert!(state.project().is_none());
    }
}
