//! 生成ジョブの登録・実行・キャンセル。`generate` / `cancel_generation` コマンドの中身。

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use kataribe_engine::{ChangeSet, Engine, EventSink, Task};
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::error::{CommandError, CommandErrorKind};

/// 実行中のジョブの登録簿。ジョブ ID → そのジョブのキャンセルトークン。
pub type JobRegistry = Mutex<HashMap<String, CancellationToken>>;

/// ジョブ ID の重複を確かめてから登録し、キャンセルトークンを返す。
fn register(jobs: &JobRegistry, job_id: &str) -> Result<CancellationToken, CommandError> {
    let mut jobs = jobs.lock().unwrap_or_else(PoisonError::into_inner);
    if jobs.contains_key(job_id) {
        return Err(CommandError::new(
            CommandErrorKind::InvalidInput,
            "同じ ID の生成が既に実行中です。",
        ));
    }
    let token = CancellationToken::new();
    jobs.insert(job_id.to_owned(), token.clone());
    Ok(token)
}

fn unregister(jobs: &JobRegistry, job_id: &str) {
    jobs.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(job_id);
}

/// タスクを実行し、変更案を返す。途中経過は `events` に流す。
///
/// 成功・失敗・中止のいずれで終わっても、ジョブの登録を外す。
pub async fn generate(
    jobs: &JobRegistry,
    engine: &Engine,
    project: &Project,
    job_id: &str,
    task: &Task,
    events: &dyn EventSink,
) -> Result<ChangeSet, CommandError> {
    let cancel = register(jobs, job_id)?;
    let result = engine.generate(project, task, events, &cancel).await;
    unregister(jobs, job_id);
    result.map_err(CommandError::from)
}

/// 実行中のジョブを中止する。そのジョブが（既に終わっているなどで）見つからなければ何もしない。
pub fn cancel(jobs: &JobRegistry, job_id: &str) {
    if let Some(token) = jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(job_id)
    {
        token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_engine::{GenerationSettings, IgnoreEvents};
    use kataribe_llm::testing::{Script, ScriptedChatModel};
    use kataribe_project::{FORMAT_VERSION, Manifest, Rating};
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tempfile::TempDir;

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

    fn engine_with(model: ScriptedChatModel) -> Engine {
        Engine::new(Arc::new(model), GenerationSettings::default()).unwrap()
    }

    #[tokio::test]
    async fn generate_runs_the_task_and_clears_the_job_on_success() {
        let dir = TempDir::new().unwrap();
        let project = sample_project(&dir);
        let engine = engine_with(ScriptedChatModel::new([Script::reply([
            "夜の底で、遠くの灯りが揺れていた。",
        ])]));
        let jobs: JobRegistry = Mutex::new(HashMap::new());

        let result = generate(
            &jobs,
            &engine,
            &project,
            "job-1",
            &Task::Concept,
            &IgnoreEvents,
        )
        .await;

        assert!(result.is_ok(), "{result:?}");
        assert!(jobs.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn generate_maps_a_failed_task_to_a_command_error_and_still_clears_the_job() {
        let dir = TempDir::new().unwrap();
        let project = sample_project(&dir);
        // 台本を用意しないため、モデルはエラーで応答する。
        let engine = engine_with(ScriptedChatModel::new([]));
        let jobs: JobRegistry = Mutex::new(HashMap::new());

        let error = generate(
            &jobs,
            &engine,
            &project,
            "job-1",
            &Task::Concept,
            &IgnoreEvents,
        )
        .await
        .unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Llm);
        assert!(jobs.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn generate_rejects_a_duplicate_job_id_without_touching_the_existing_entry() {
        let dir = TempDir::new().unwrap();
        let project = sample_project(&dir);
        let engine = engine_with(ScriptedChatModel::new([]));
        let jobs: JobRegistry = Mutex::new(HashMap::new());
        let existing_token = register(&jobs, "job-1").unwrap();

        let error = generate(
            &jobs,
            &engine,
            &project,
            "job-1",
            &Task::Concept,
            &IgnoreEvents,
        )
        .await
        .unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
        assert!(jobs.lock().unwrap().contains_key("job-1"));
        assert!(!existing_token.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_stops_the_matching_job_and_generate_reports_it_as_cancelled() {
        let dir = TempDir::new().unwrap();
        let project = sample_project(&dir);
        let engine = engine_with(ScriptedChatModel::new([Script::reply(["本文"])]));
        let jobs: JobRegistry = Mutex::new(HashMap::new());

        let token = register(&jobs, "job-1").unwrap();
        cancel(&jobs, "job-1");
        assert!(token.is_cancelled());

        let result = engine
            .generate(&project, &Task::Concept, &IgnoreEvents, &token)
            .await;
        unregister(&jobs, "job-1");

        assert!(matches!(
            result,
            Err(kataribe_engine::EngineError::Cancelled)
        ));
        assert!(jobs.lock().unwrap().is_empty());
    }

    #[test]
    fn cancel_on_an_unknown_job_id_does_nothing() {
        let jobs: JobRegistry = Mutex::new(HashMap::new());
        cancel(&jobs, "no-such-job");
        assert!(jobs.lock().unwrap().is_empty());
    }
}
