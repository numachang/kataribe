//! 生成ジョブの登録・実行・キャンセル。`generate` / `cancel_generation` コマンドの中身。

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use kataribe_engine::{ChangeSet, Engine, EventSink, Task};
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::error::{CommandError, CommandErrorKind};

/// 実行中のジョブの登録簿。ジョブ ID → そのジョブのキャンセルトークン。
pub type JobRegistry = Mutex<HashMap<String, CancellationToken>>;

/// 登録簿に載った生成ジョブ。drop すると登録を外す（成功・失敗・中止・途中のエラーのどれで終わっても）。
pub struct Job<'a> {
    jobs: &'a JobRegistry,
    id: String,
    cancel: CancellationToken,
}

impl<'a> Job<'a> {
    /// ジョブ ID の重複を確かめてから登録する。
    pub fn register(jobs: &'a JobRegistry, id: &str) -> Result<Self, CommandError> {
        let mut registered = jobs.lock().unwrap_or_else(PoisonError::into_inner);
        if registered.contains_key(id) {
            return Err(CommandError::new(
                CommandErrorKind::InvalidInput,
                "同じ ID の生成が既に実行中です。",
            ));
        }
        let cancel = CancellationToken::new();
        registered.insert(id.to_owned(), cancel.clone());
        Ok(Self {
            jobs,
            id: id.to_owned(),
            cancel,
        })
    }

    /// このジョブのキャンセルトークン。
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel
    }

    /// タスクを実行し、変更案を返す。途中経過は `events` に流す。
    pub async fn run(
        &self,
        engine: &Engine,
        project: &Project,
        task: &Task,
        events: &dyn EventSink,
    ) -> Result<ChangeSet, CommandError> {
        engine
            .generate(project, task, events, &self.cancel)
            .await
            .map_err(CommandError::from)
    }
}

impl Drop for Job<'_> {
    fn drop(&mut self) {
        self.jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.id);
    }
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

/// 実行中のジョブをすべて中止する（作品を閉じる・開き直すとき）。
pub fn cancel_all(jobs: &JobRegistry) {
    for token in jobs.lock().unwrap_or_else(PoisonError::into_inner).values() {
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
            settings: None,
            extra: std::collections::BTreeMap::new(),
        };
        Project::create(dir.path(), &manifest).unwrap();
        Project::open(dir.path()).unwrap()
    }

    fn engine_with(model: ScriptedChatModel) -> Engine {
        Engine::new(Arc::new(model), GenerationSettings::default()).unwrap()
    }

    fn new_registry() -> JobRegistry {
        Mutex::new(HashMap::new())
    }

    #[tokio::test]
    async fn job_runs_the_task_and_leaves_the_registry_when_dropped() {
        let dir = TempDir::new().unwrap();
        let project = sample_project(&dir);
        let engine = engine_with(ScriptedChatModel::new([Script::reply([
            "夜の底で、遠くの灯りが揺れていた。",
        ])]));
        let jobs = new_registry();

        let job = Job::register(&jobs, "job-1").unwrap();
        let result = job
            .run(&engine, &project, &Task::Concept, &IgnoreEvents)
            .await;
        assert!(jobs.lock().unwrap().contains_key("job-1"));
        drop(job);

        assert!(result.is_ok(), "{result:?}");
        assert!(jobs.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_task_becomes_a_command_error() {
        let dir = TempDir::new().unwrap();
        let project = sample_project(&dir);
        // 台本を用意しないため、モデルはエラーで応答する。
        let engine = engine_with(ScriptedChatModel::new([]));
        let jobs = new_registry();

        let job = Job::register(&jobs, "job-1").unwrap();
        let error = job
            .run(&engine, &project, &Task::Concept, &IgnoreEvents)
            .await
            .unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Llm);
    }

    #[test]
    fn duplicate_job_id_is_rejected_without_touching_the_existing_job() {
        let jobs = new_registry();
        let existing = Job::register(&jobs, "job-1").unwrap();

        let error = Job::register(&jobs, "job-1").err().unwrap();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
        assert!(jobs.lock().unwrap().contains_key("job-1"));
        assert!(!existing.cancel_token().is_cancelled());
    }

    #[tokio::test]
    async fn job_cancelled_before_it_runs_reports_cancelled() {
        let dir = TempDir::new().unwrap();
        let project = sample_project(&dir);
        let engine = engine_with(ScriptedChatModel::new([Script::reply(["本文"])]));
        let jobs = new_registry();

        // 準備中（API キーの読み込みなど）に中止が届いた場合
        let job = Job::register(&jobs, "job-1").unwrap();
        cancel(&jobs, "job-1");
        let error = job
            .run(&engine, &project, &Task::Concept, &IgnoreEvents)
            .await
            .unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Cancelled);
    }

    #[test]
    fn cancel_on_an_unknown_job_id_does_nothing() {
        let jobs = new_registry();
        cancel(&jobs, "no-such-job");
        assert!(jobs.lock().unwrap().is_empty());
    }

    #[test]
    fn cancel_all_stops_every_running_job() {
        let jobs = new_registry();
        let first = Job::register(&jobs, "job-1").unwrap();
        let second = Job::register(&jobs, "job-2").unwrap();

        cancel_all(&jobs);

        assert!(first.cancel_token().is_cancelled());
        assert!(second.cancel_token().is_cancelled());
    }
}
