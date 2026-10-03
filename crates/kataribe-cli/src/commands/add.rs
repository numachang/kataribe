//! `add` サブコマンド: 人物・世界観の資料・シーンを自分で書いて足す。

use std::collections::BTreeMap;

use anyhow::Context;
use kataribe_engine::{NewScenePlan, StructureEdit};
use kataribe_project::CharacterMeta;

use crate::ApplyGuard;
use crate::output::Console;
use crate::structure_args::{
    AddArgs, AddCharacterArgs, AddSceneArgs, AddTarget, AddWorldArgs, BodySource,
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
///
/// ファイルは BOM と CRLF を正規化して読む（作品の中のファイルと同じ規則）。
fn read_body(source: &BodySource) -> anyhow::Result<String> {
    match (&source.body, &source.body_file) {
        (Some(body), _) => Ok(body.clone()),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map(|text| kataribe_project::normalize_text(&text))
            .with_context(|| format!("本文のファイルを読み込めません: {}", path.display())),
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
