use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kataribe_llm::ChatModel;
use kataribe_project::Project;
use tokio_util::sync::CancellationToken;

use crate::caller::Caller;
use crate::change_set::ChangeSet;
use crate::error::Result;
use crate::events::{EventSink, GenerationEvent};
use crate::genre::GenreCatalog;
use crate::prompt::PromptLibrary;
use crate::settings::GenerationSettings;
use crate::stages::{self, Stage};
use crate::task::Task;

/// 執筆エンジン。1 つの LLM と生成設定の組み合わせごとに作る。
#[derive(Debug)]
pub struct Engine {
    model: Arc<dyn ChatModel>,
    settings: GenerationSettings,
    prompts: PromptLibrary,
    genres: GenreCatalog,
    /// サーバーが JSON Schema の指定を受け付けるか。拒否されたら以後は指示文で JSON を求める。
    structured_output: AtomicBool,
}

impl Engine {
    pub fn new(model: Arc<dyn ChatModel>, settings: GenerationSettings) -> Result<Self> {
        Ok(Self {
            model,
            settings: settings.sanitized(),
            prompts: PromptLibrary::builtin()?,
            genres: GenreCatalog::builtin()?,
            structured_output: AtomicBool::new(true),
        })
    }

    pub fn settings(&self) -> &GenerationSettings {
        &self.settings
    }

    /// タスクを実行し、変更案を返す。作品フォルダの原稿は書き換えない
    /// （書き換えるのは `.kataribe/cache/` の中間データだけ）。
    pub async fn generate(
        &self,
        project: &Project,
        task: &Task,
        events: &dyn EventSink,
        cancel: &CancellationToken,
    ) -> Result<ChangeSet> {
        events.emit(GenerationEvent::Started {
            model: self.model.describe(),
        });
        let caller = Caller::new(
            self.model.as_ref(),
            events,
            cancel,
            &self.settings,
            &self.structured_output,
        );
        let stage = Stage::new(project, &self.genres, &self.prompts, caller, &self.settings)?;
        Ok(stages::run(&stage, task).await?.made_for(project))
    }
}
