//! 設定資料と本文を段階的に生成する執筆エンジン。
//!
//! GUI（src-tauri）と CLI（kataribe-cli）は、どちらもこの crate だけを通して生成を行う。
//! どの生成も作品フォルダを直接書き換えず、変更案（[`ChangeSet`]）を返す。

mod caller;
mod change_set;
mod cleanup;
mod engine;
mod error;
mod events;
mod excerpt;
mod genre;
mod new_project;
mod overview;
mod pipeline;
mod prompt;
mod secrets;
mod settings;
mod stages;
mod task;

pub use change_set::{ChangeSet, FileChange};
pub use engine::Engine;
pub use error::{EngineError, Result};
pub use events::{EventSink, GenerationEvent, IgnoreEvents, NoticeLevel};
pub use genre::{GenreCatalog, GenrePreset};
pub use new_project::{NewProject, create_project};
pub use overview::{
    EntryKind, OverviewEntry, OverviewSection, ProjectOverview, SectionKind, overview,
};
pub use pipeline::{PipelineStep, StepState, pipeline};
pub use secrets::ApiKeyStore;
pub use settings::{
    DraftUnit, EngineSettings, GenerationSettings, LlmSettings, default_settings_path,
    load_settings, save_settings,
};
pub use task::Task;

/// アプリの識別子。設定ファイルの置き場所（Tauri のアプリ設定フォルダ）に使う。
pub const APP_IDENTIFIER: &str = "io.github.numachang.kataribe";
