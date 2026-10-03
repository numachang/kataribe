//! `add` サブコマンド: 人物・世界観の資料・章・シーンを自分で書いて足す。

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use kataribe_engine::{NewScenePlan, StructureEdit};
use kataribe_project::CharacterMeta;

use crate::ApplyGuard;
use crate::output::Console;
use crate::structure_args::{
    AddArgs, AddChapterArgs, AddCharacterArgs, AddSceneArgs, AddTarget, AddWorldArgs, BodySource,
    StorylineSource,
};

use super::{Outcome, structure_edit};

pub async fn run(
    args: &AddArgs,
    console: &dyn Console,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<Outcome> {
    let (folder, edit, dry_run) = match &args.target {
        AddTarget::Character(args) => (&args.folder, character_edit(args)?, args.dry_run),
        AddTarget::World(args) => (&args.folder, world_edit(args)?, args.dry_run),
        AddTarget::Chapter(args) => (&args.folder, chapter_edit(args)?, args.dry_run),
        AddTarget::Scene(args) => (&args.folder, scene_edit(args), args.dry_run),
    };
    structure_edit::run(folder, &edit, dry_run, console, apply_guard).await
}

fn character_edit(args: &AddCharacterArgs) -> anyhow::Result<StructureEdit> {
    Ok(StructureEdit::AddCharacter {
        id: args.id.clone(),
        meta: CharacterMeta {
            name: args.name.clone(),
            reading: args.reading.clone(),
            role: args.role.clone().unwrap_or_default(),
            summary: args.summary.clone().unwrap_or_default(),
            order: args.order,
            extra: BTreeMap::new(),
        },
        body: read_body(&args.body)?,
    })
}

fn world_edit(args: &AddWorldArgs) -> anyhow::Result<StructureEdit> {
    Ok(StructureEdit::AddWorldDocument {
        name: args.name.clone(),
        title: args.title.clone(),
        body: read_body(&args.body)?,
    })
}

fn chapter_edit(args: &AddChapterArgs) -> anyhow::Result<StructureEdit> {
    Ok(StructureEdit::AddChapter {
        before: args.before,
        title: args.title.clone(),
        storyline: read_storyline(&args.storyline)?,
    })
}

fn scene_edit(args: &AddSceneArgs) -> StructureEdit {
    StructureEdit::AddScene {
        chapter: args.chapter,
        before: args.before,
        scene: NewScenePlan {
            title: args.title.clone(),
            summary: args.summary.clone().unwrap_or_default(),
            pov: args.pov.clone(),
            characters: args.characters.clone(),
            place: args.place.clone(),
            time: args.time.clone(),
            target_chars: args.target_chars,
        },
    }
}

/// `--body` か `--body-file` から本文を取り出す。どちらも無ければ空。
fn read_body(source: &BodySource) -> anyhow::Result<String> {
    read_text_or_file(source.body.as_deref(), source.body_file.as_deref(), "本文")
}

/// `--storyline` か `--storyline-file` からストーリーラインを取り出す。どちらも無ければ空。
fn read_storyline(source: &StorylineSource) -> anyhow::Result<String> {
    read_text_or_file(
        source.storyline.as_deref(),
        source.storyline_file.as_deref(),
        "ストーリーライン",
    )
}

/// 直接の文章か、文章を書いたファイルから取り出す。どちらも無ければ空。`what` はエラーに出す呼び名。
///
/// ファイルは BOM と CRLF を正規化して読む（作品の中のファイルと同じ規則）。
fn read_text_or_file(
    text: Option<&str>,
    file: Option<&Path>,
    what: &str,
) -> anyhow::Result<String> {
    match (text, file) {
        (Some(text), _) => Ok(text.to_owned()),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map(|text| kataribe_project::normalize_text(&text))
            .with_context(|| format!("{what}のファイルを読み込めません: {}", path.display())),
        (None, None) => Ok(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_comes_from_the_text_the_file_or_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("body.txt");
        std::fs::write(&path, "\u{feff}一行目\r\n二行目\r\n").unwrap();

        let inline = BodySource {
            body: Some("直接の本文".to_owned()),
            body_file: None,
        };
        let from_file = BodySource {
            body: None,
            body_file: Some(path),
        };
        let nothing = BodySource {
            body: None,
            body_file: None,
        };

        assert_eq!(read_body(&inline).unwrap(), "直接の本文");
        assert_eq!(read_body(&from_file).unwrap(), "一行目\n二行目\n");
        assert_eq!(read_body(&nothing).unwrap(), "");
    }

    #[test]
    fn the_storyline_comes_from_the_text_the_file_or_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("storyline.txt");
        std::fs::write(&path, "\u{feff}あらすじ\r\n").unwrap();

        let inline = StorylineSource {
            storyline: Some("直接のあらすじ".to_owned()),
            storyline_file: None,
        };
        let from_file = StorylineSource {
            storyline: None,
            storyline_file: Some(path),
        };
        let nothing = StorylineSource {
            storyline: None,
            storyline_file: None,
        };

        assert_eq!(read_storyline(&inline).unwrap(), "直接のあらすじ");
        assert_eq!(read_storyline(&from_file).unwrap(), "あらすじ\n");
        assert_eq!(read_storyline(&nothing).unwrap(), "");
    }

    #[test]
    fn a_missing_storyline_file_is_an_error_that_names_the_kind_of_text_and_the_file() {
        let source = StorylineSource {
            storyline: None,
            storyline_file: Some(std::path::PathBuf::from("no-such-storyline.txt")),
        };

        let error = format!("{:#}", read_storyline(&source).unwrap_err());

        assert!(error.contains("ストーリーラインのファイル"), "{error}");
        assert!(error.contains("no-such-storyline.txt"), "{error}");
    }

    #[test]
    fn a_missing_body_file_is_an_error_that_names_the_file() {
        let source = BodySource {
            body: None,
            body_file: Some(std::path::PathBuf::from("no-such-file.txt")),
        };

        let error = read_body(&source).unwrap_err();

        assert!(
            format!("{error:#}").contains("no-such-file.txt"),
            "{error:#}"
        );
    }
}
