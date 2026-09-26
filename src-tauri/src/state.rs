//! アプリ全体で共有する状態（`tauri::State`）。
//!
//! ロックは値の読み書きの間だけ持ち、`.await` をまたいでは持たない
//! （生成ジョブの実行など、時間のかかる `.await` の前には値を複製してロックを手放す）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use kataribe_engine::ApiKeyStore;
use kataribe_project::Project;

use crate::error::CommandError;
use crate::generation::JobRegistry;
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
    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    #[must_use]
    pub fn settings(&self) -> AppSettings {
        self.settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_settings(&self, settings: AppSettings) {
        *self.settings.lock().unwrap_or_else(PoisonError::into_inner) = settings;
    }

    /// `folder` を最近使った作品の先頭に置き、設定ファイルに保存する。
    pub fn record_recent_project(&self, folder: &str) -> Result<(), CommandError> {
        let mut settings = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
        settings::record_recent_project(&mut settings.recent_projects, folder);
        settings::save(&self.settings_path, &settings)
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

    pub fn set_project(&self, project: Project) {
        *self.project.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(project));
    }

    pub fn clear_project(&self) {
        *self.project.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    #[must_use]
    pub fn jobs(&self) -> &JobRegistry {
        &self.jobs
    }
}
