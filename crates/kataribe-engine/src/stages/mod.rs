//! 生成タスクごとの処理。

mod addition;
mod cast;
mod documents;
mod draft;
pub(crate) mod materials;
mod outline;
mod revise;
mod scene_plan;

use kataribe_project::{Manifest, Project, RelPath, TextFile};

use crate::caller::Caller;
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::genre::GenreCatalog;
use crate::prompt::{Prompt, PromptLibrary};
use crate::settings::GenerationSettings;
use crate::task::Task;
use materials::ProjectInfo;

/// 生成の出力に回すトークンの下限。これを確保できないほど文脈が長いなら、設定の見直しを促す。
const MIN_OUTPUT_TOKENS: usize = 512;

/// 1 つのタスクの実行に必要なものをまとめたもの。
pub(crate) struct Stage<'a> {
    pub project: &'a Project,
    pub manifest: Manifest,
    pub info: ProjectInfo,
    pub prompts: &'a PromptLibrary,
    pub caller: Caller<'a>,
    pub settings: &'a GenerationSettings,
}

impl<'a> Stage<'a> {
    pub fn new(
        project: &'a Project,
        genres: &GenreCatalog,
        prompts: &'a PromptLibrary,
        caller: Caller<'a>,
        settings: &'a GenerationSettings,
    ) -> Result<Self> {
        let manifest = project.manifest()?;
        let info = ProjectInfo::new(&manifest, genres);
        Ok(Self {
            project,
            manifest,
            info,
            prompts,
            caller,
            settings,
        })
    }

    /// 生成を始める前のファイルの内容。変更案の基準にする（適用時に、その後の編集との競合を確かめる）。
    pub fn snapshot(&self, path: &RelPath) -> Result<Option<TextFile>> {
        Ok(self.project.store().read_text_opt(path)?)
    }

    /// `chars` 文字の文章を書かせるのに確保する出力トークン数。
    /// 日本語は 1 文字 1 トークン前後なので、書きすぎの分を上乗せする。思考を止めないなら思考の分も足す。
    pub fn output_tokens_for_chars(&self, chars: u32) -> u32 {
        const HEADROOM: u32 = 256;
        const THINKING_ALLOWANCE: u32 = 4096;
        let prose = chars.saturating_mul(3) / 2 + HEADROOM;
        if self.settings.disable_thinking {
            prose
        } else {
            prose.saturating_add(THINKING_ALLOWANCE)
        }
    }

    /// プロンプトに使えるトークン数（出力に `output_tokens` を確保したうえで）。
    pub fn prompt_budget(&self, output_tokens: u32) -> usize {
        self.usable_context().saturating_sub(output_tokens as usize)
    }

    /// 見積もりの誤差とチャットテンプレートの分として、文脈の 5% を余白に残した長さ。
    fn usable_context(&self) -> usize {
        let context = self.settings.context_tokens as usize;
        context - context / 20
    }

    /// 出力に確保するトークン数。望ましい値 `desired` を、文脈の残りに収まるよう切り詰める。
    pub fn output_tokens(&self, prompt: &Prompt, desired: u32) -> Result<u32> {
        let available = self
            .usable_context()
            .saturating_sub(prompt.estimated_tokens());
        if available < MIN_OUTPUT_TOKENS {
            return Err(EngineError::ContextTooSmall(format!(
                "資料が長く、モデルの文脈（{} トークン）に収まりません。設定の「文脈の長さ」を増やすか、資料を短くしてください。",
                self.settings.context_tokens
            )));
        }
        Ok(desired.min(u32::try_from(available).unwrap_or(u32::MAX)))
    }
}

pub(crate) async fn run(stage: &Stage<'_>, task: &Task) -> Result<ChangeSet> {
    match task {
        Task::Concept => documents::concept(stage).await,
        Task::Style => documents::style(stage).await,
        Task::World => documents::world(stage).await,
        Task::Cast => cast::roster(stage).await,
        Task::Character { id } => cast::profile(stage, id).await,
        Task::Synopsis => documents::synopsis(stage).await,
        Task::Outline => outline::chapters(stage).await,
        Task::ScenePlan { chapter } => scene_plan::scenes(stage, *chapter).await,
        Task::Draft { chapter, scene } => draft::write(stage, *chapter, *scene).await,
        Task::Revise { path, instruction } => revise::document(stage, path, instruction).await,
        Task::AddCharacter { instruction } => addition::add_character(stage, instruction).await,
        Task::AddWorldDocument { name, instruction } => {
            addition::add_world_document(stage, name.as_deref(), instruction).await
        }
    }
}
