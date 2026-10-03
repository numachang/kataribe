//! 画面から呼ぶ Tauri コマンド。engine・project・text を公開するだけの薄い層にする。
//!
//! ここに置くのは Tauri の呼び出し規約（`State`・`Channel`・戻り値の形）に合わせるための
//! 薄いラッパーだけ。実際の判断は `crate::files`・`crate::projects`・`crate::generation`・
//! `crate::engine_client`・`crate::settings` の、Tauri に依存しない関数が行う。

use tauri::State;
use tauri::ipc::Channel;

use kataribe_engine::{
    ChangeSet, GenerationEvent, GenrePreset, LlmSettings, NewProject, PipelineStep,
    ProjectOverview, ProjectSettings, StructureEdit, StructurePlan, Task,
};
use kataribe_llm::ModelInfo;
use kataribe_project::{EditableDocument, ParsedDocument};
use kataribe_text::count::TextStats;
use kataribe_text::quality::{QualityOptions, QualityReport};
use kataribe_text::ruby::Segment;

use crate::error::CommandError;
use crate::hashed_file::{DocumentFile, ProjectSettingsFile};
use crate::settings::AppSettings;
use crate::state::AppState;
use crate::{engine_client, files, generation, projects, structure};

// ---- 設定 ----

#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError> {
    state.reload_settings()
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), CommandError> {
    state.save_settings(settings)
}

/// 開いている作品の設定（`kataribe.yaml` の `settings`）と、読んだ時点の `kataribe.yaml` のハッシュ。
#[tauri::command]
pub async fn load_project_settings(
    state: State<'_, AppState>,
) -> Result<ProjectSettingsFile, CommandError> {
    let project = state.require_project()?;
    let (settings, hash) = ProjectSettings::load_with_hash(&project)?;
    Ok(ProjectSettingsFile {
        settings,
        hash: hash.to_string(),
    })
}

/// 開いている作品の設定を保存し、保存した後の `kataribe.yaml` のハッシュを返す。
/// `expected_hash`（読んだときのハッシュ）から `kataribe.yaml` が変わっていれば `conflict` で失敗する。
#[tauri::command]
pub async fn save_project_settings(
    state: State<'_, AppState>,
    settings: ProjectSettings,
    expected_hash: String,
) -> Result<String, CommandError> {
    let project = state.require_project()?;
    let expected = files::parse_content_hash(&expected_hash)?;
    let hash = settings.save(&project, Some(&expected))?;
    Ok(hash.to_string())
}

#[tauri::command]
pub async fn set_api_key(
    state: State<'_, AppState>,
    api_key: Option<String>,
) -> Result<(), CommandError> {
    match api_key {
        Some(key) => state.api_key_store().save(&key).map_err(CommandError::from),
        None => state.api_key_store().delete().map_err(CommandError::from),
    }
}

#[tauri::command]
pub async fn has_api_key(state: State<'_, AppState>) -> Result<bool, CommandError> {
    let key = state.api_key_store().load().map_err(CommandError::from)?;
    Ok(key.is_some())
}

/// `llm` を渡すと、保存前の入力中の接続先で試す（接続テスト用）。渡さなければ、開いている作品の設定を
/// 重ねた接続先を使う。API キーは保存済みのものを使う。
#[tauri::command]
pub async fn list_models(
    state: State<'_, AppState>,
    llm: Option<LlmSettings>,
) -> Result<Vec<ModelInfo>, CommandError> {
    let effective_llm = match llm {
        Some(llm) => llm,
        None => {
            engine_client::effective_settings(&state.settings(), state.project().as_deref())?.llm
        }
    };
    let api_key = engine_client::load_api_key(state.api_key_store(), &effective_llm)?;
    engine_client::list_models(&effective_llm, api_key).await
}

#[tauri::command]
pub async fn list_genres() -> Result<Vec<GenrePreset>, CommandError> {
    let catalog = kataribe_engine::GenreCatalog::builtin().map_err(CommandError::from)?;
    Ok(catalog.genres().to_vec())
}

// ---- 作品 ----

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    folder: String,
    project: NewProject,
) -> Result<ProjectOverview, CommandError> {
    let (opened, overview) = projects::create_project(std::path::Path::new(&folder), project)?;
    state.set_project(opened);
    remember_recent_project(&state, &folder);
    Ok(overview)
}

#[tauri::command]
pub async fn open_project(
    state: State<'_, AppState>,
    folder: String,
) -> Result<ProjectOverview, CommandError> {
    let (opened, overview) = projects::open_project(std::path::Path::new(&folder))?;
    state.set_project(opened);
    remember_recent_project(&state, &folder);
    Ok(overview)
}

/// 最近の作品に記録する。記録できなくても作品は開けているので、失敗はログに残すだけにする
/// （ここでエラーを返すと、画面は「開けなかった」と表示するのに作品は開いたままになり、食い違う）。
fn remember_recent_project(state: &AppState, folder: &str) {
    if let Err(error) = state.record_recent_project(folder) {
        tracing::warn!(%error, folder, "最近の作品を記録できませんでした");
    }
}

#[tauri::command]
pub async fn close_project(state: State<'_, AppState>) -> Result<(), CommandError> {
    state.clear_project();
    Ok(())
}

#[tauri::command]
pub async fn overview(state: State<'_, AppState>) -> Result<ProjectOverview, CommandError> {
    let project = state.require_project()?;
    kataribe_engine::overview(&project).map_err(CommandError::from)
}

#[tauri::command]
pub async fn pipeline(state: State<'_, AppState>) -> Result<Vec<PipelineStep>, CommandError> {
    let project = state.require_project()?;
    let unit = engine_client::effective_settings(&state.settings(), Some(&project))?
        .generation
        .draft_unit;
    kataribe_engine::pipeline(&project, unit).map_err(CommandError::from)
}

// ---- 文書 ----

/// 画面で編集する形（人物資料・章立ては front matter を項目に分けた形）でファイルを読む。
#[tauri::command]
pub async fn read_document(
    state: State<'_, AppState>,
    path: String,
) -> Result<DocumentFile, CommandError> {
    let project = state.require_project()?;
    files::read_document(&project, &path)
}

#[tauri::command]
pub async fn write_document(
    state: State<'_, AppState>,
    path: String,
    document: EditableDocument,
    expected_hash: Option<String>,
) -> Result<String, CommandError> {
    let project = state.require_project()?;
    files::write_document(&project, &path, &document, expected_hash.as_deref())
}

/// 文字列を、パスの種類に応じて画面で編集する形に分ける。作品を開いていなくても使える。
#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn parse_document(path: String, content: String) -> Result<ParsedDocument, CommandError> {
    files::parse_document(&path, &content)
}

// ---- テキスト ----
//
// Tauri コマンドは IPC から受け取った値の持ち主になるので、ここでは借用ではなく
// 所有した `String` を受ける（`#[tauri::command]` の呼び出し規約）。中身は借用するだけなので
// clippy::needless_pass_by_value を意図的に無効にする。

// 長い本文の解析で画面の操作が止まらないよう、どれもメインスレッドの外（async）で動かす。

#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn text_stats(text: String) -> TextStats {
    kataribe_text::count::stats(&text)
}

#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn parse_ruby(text: String) -> Vec<Segment> {
    kataribe_text::ruby::parse(&text)
}

#[tauri::command(async)]
#[allow(clippy::needless_pass_by_value)]
pub fn analyze_quality(text: String, target_chars: Option<usize>) -> QualityReport {
    kataribe_text::quality::analyze(&text, &QualityOptions { target_chars })
}

// ---- 生成 ----

#[tauri::command]
pub async fn generate(
    state: State<'_, AppState>,
    job_id: String,
    task: Task,
    on_event: Channel<GenerationEvent>,
) -> Result<ChangeSet, CommandError> {
    // 準備（API キーの読み込みなど）の間に届いた中止も受け付けるよう、最初に登録する
    let job = generation::Job::register(state.jobs(), &job_id)?;
    let project = state.require_project()?;
    let settings = engine_client::effective_settings(&state.settings(), Some(&project))?;
    let api_key = engine_client::load_api_key(state.api_key_store(), &settings.llm)?;
    let engine = engine_client::build_engine(&settings.llm, &settings.generation, api_key)?;
    let cancel = job.cancel_token().clone();
    let sink = move |event: GenerationEvent| {
        // 画面が再読み込みなどで無くなり、送れなくなった。結果を受け取る先が無いので生成を止める
        if on_event.send(event).is_err() {
            cancel.cancel();
        }
    };
    job.run(&engine, &project, &task, &sink).await
}

#[tauri::command]
pub async fn cancel_generation(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), CommandError> {
    generation::cancel(state.jobs(), &job_id);
    Ok(())
}

#[tauri::command]
pub async fn apply_change_set(
    state: State<'_, AppState>,
    change_set: ChangeSet,
) -> Result<ProjectOverview, CommandError> {
    let project = state.require_project()?;
    change_set.apply(&project).map_err(CommandError::from)?;
    kataribe_engine::overview(&project).map_err(CommandError::from)
}

// ---- 構成（人物・世界観の資料・シーンの追加と削除） ----

/// 構成の操作の変更案と、確認の材料を作る。作品フォルダは書き換えない。適用は `apply_change_set`。
#[tauri::command]
pub async fn plan_structure_edit(
    state: State<'_, AppState>,
    edit: StructureEdit,
) -> Result<StructurePlan, CommandError> {
    let project = state.require_project()?;
    structure::plan_structure_edit(&project, &edit)
}

/// 人物の ID の案を、読み（かな）からローマ字で作る。
#[tauri::command]
pub async fn suggest_character_id(
    state: State<'_, AppState>,
    reading: String,
    name: String,
) -> Result<String, CommandError> {
    let project = state.require_project()?;
    structure::suggest_character_id(&project, &reading, &name)
}
