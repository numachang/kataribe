//! `models` サブコマンド: 選べるモデルの一覧を表示する。

use anyhow::Context;

use crate::args::GlobalOptions;
use crate::output::Console;
use crate::settings;

use super::Outcome;

pub async fn run(global: &GlobalOptions, console: &dyn Console) -> anyhow::Result<Outcome> {
    let settings = settings::load_effective_settings(global, None)?;
    let api_key = super::api_key_for(&settings.llm, &global.api_key_env)?;
    let models = kataribe_engine::list_models(&settings.llm, api_key)
        .await
        .context("モデル一覧を取得できません")?;

    if models.is_empty() {
        console
            .print("利用できるモデルがありません。\n")
            .context("標準出力への書き込みに失敗しました")?;
    }
    for model in models {
        let line = match model.context_length {
            Some(context_length) => format!("{}（文脈長 {context_length}）\n", model.id),
            None => format!("{}\n", model.id),
        };
        console
            .print(&line)
            .context("標準出力への書き込みに失敗しました")?;
    }
    Ok(Outcome::Success)
}
