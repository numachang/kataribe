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
    console
        .print(&format!(
            "作品を作成しました: {}\n",
            project.root().display()
        ))
        .context("標準出力への書き込みに失敗しました")?;
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
            .map(|text| kataribe_project::normalize_text(&text))
            .with_context(|| format!("企画の種のファイルを読み込めません: {}", path.display())),
        (None, None) => anyhow::bail!("--idea か --idea-file のどちらかを指定してください"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::IdeaSource;

    fn args_with_idea_file(path: std::path::PathBuf) -> NewArgs {
        NewArgs {
            folder: std::path::PathBuf::from("folder"),
            title: "題名".to_owned(),
            author: None,
            genre: None,
            genre_note: None,
            rating: crate::args::RatingArg::General,
            length: 30_000,
            idea_source: IdeaSource {
                idea: None,
                idea_file: Some(path),
            },
        }
    }

    #[test]
    fn resolve_idea_normalizes_bom_and_crlf_from_the_idea_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("idea.txt");
        std::fs::write(&path, "\u{feff}嵐の洋館。\r\n二行目。\r\n").unwrap();

        let idea = resolve_idea(&args_with_idea_file(path)).unwrap();

        assert_eq!(idea, "嵐の洋館。\n二行目。\n");
    }

    #[test]
    fn resolve_idea_uses_the_inline_text_as_is() {
        let args = NewArgs {
            idea_source: IdeaSource {
                idea: Some("嵐の洋館。".to_owned()),
                idea_file: None,
            },
            ..args_with_idea_file(std::path::PathBuf::from("unused"))
        };

        let idea = resolve_idea(&args).unwrap();

        assert_eq!(idea, "嵐の洋館。");
    }
}
