//! `new` サブコマンド: 新しい作品を作る。

use anyhow::Context;
use kataribe_engine::{NewProject, create_project};

use crate::args::NewArgs;
use crate::output::Console;

use super::Outcome;

pub fn run(args: &NewArgs, console: &dyn Console) -> anyhow::Result<Outcome> {
    let idea = resolve_idea(args)?;
    let new_project = NewProject {
        title: args.title.clone(),
        author: args.author.clone(),
        genre: args.genre.clone().unwrap_or_else(|| "general".to_owned()),
        genre_note: args.genre_note.clone(),
        rating: args.rating.into(),
        target_length: args.length,
        idea,
    };
    let project =
        create_project(&args.folder, new_project).context("作品を作成できませんでした")?;
    console.print(&format!(
        "作品を作成しました: {}\n",
        project.root().display()
    ));
    Ok(Outcome::Success)
}

/// `--idea` か `--idea-file` のどちらかから企画の種を取り出す。
///
/// clap の `ArgGroup` によりどちらか一方は必ず指定されているが、その保証は
/// 型には表れないため、万一の場合でも panic せずエラーとして扱う。
fn resolve_idea(args: &NewArgs) -> anyhow::Result<String> {
    match (&args.idea_source.idea, &args.idea_source.idea_file) {
        (Some(idea), _) => Ok(idea.clone()),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("企画の種のファイルを読み込めません: {}", path.display())),
        (None, None) => anyhow::bail!("--idea か --idea-file のどちらかを指定してください"),
    }
}
