//! `models` サブコマンド: LLM サーバーのモデル一覧を表示する。

use anyhow::Context;

use crate::args::GlobalOptions;
use crate::output::Console;
use crate::settings;

use super::Outcome;

pub async fn run(global: &GlobalOptions, console: &dyn Console) -> anyhow::Result<Outcome> {
    let settings = settings::load_effective_settings(global)?;
    let api_key = settings::resolve_api_key(&global.api_key_env)?;
    let client = super::build_llm_client(&settings.llm, api_key)?;
    let models = client
        .list_models()
        .await
        .context("モデル一覧を取得できません")?;

    if models.is_empty() {
        console.print("利用できるモデルがありません。\n");
    }
    for model in models {
        match model.context_length {
            Some(context_length) => {
                console.print(&format!("{}（文脈長 {context_length}）\n", model.id));
            }
            None => console.print(&format!("{}\n", model.id)),
        }
    }
    Ok(Outcome::Success)
}
