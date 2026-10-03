//! `remove` サブコマンドの `TARGET` 引数のミニ言語。
//!
//! `character:<id>` のような `種類:引数` の形を解析し、[`kataribe_engine::StructureEdit`] に変換する。
//! 書式は `generate` の `TASK`（[`crate::task_spec`]）と同じ。

use std::str::FromStr;

use kataribe_engine::StructureEdit;
use kataribe_project::{ChapterId, CharacterId, RelPath, SceneId, layout};

use crate::task_spec::parse_chapter_and_scene;

/// コマンドラインの `TARGET` 引数を解析した結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveTarget {
    Character(CharacterId),
    /// `world/` 直下の資料のパス。
    World(RelPath),
    Scene {
        chapter: ChapterId,
        scene: SceneId,
    },
}

impl RemoveTarget {
    /// 構成の操作にする。
    #[must_use]
    pub fn into_edit(self) -> StructureEdit {
        match self {
            RemoveTarget::Character(id) => StructureEdit::RemoveCharacter { id },
            RemoveTarget::World(path) => StructureEdit::RemoveWorldDocument { path },
            RemoveTarget::Scene { chapter, scene } => StructureEdit::RemoveScene { chapter, scene },
        }
    }
}

impl FromStr for RemoveTarget {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Some(id) = input.strip_prefix("character:") {
            return CharacterId::new(id)
                .map(RemoveTarget::Character)
                .map_err(|error| error.to_string());
        }
        if let Some(spec) = input.strip_prefix("world:") {
            return parse_world(spec).map(RemoveTarget::World);
        }
        if let Some(rest) = input.strip_prefix("scene:") {
            return parse_chapter_and_scene(rest, "scene")
                .map(|(chapter, scene)| RemoveTarget::Scene { chapter, scene });
        }
        Err(format!(
            "不明な対象です: {input}\n\
             次のいずれかを指定してください: character:<id> | world:<name または path> | scene:<NN>/<sNN>"
        ))
    }
}

/// 世界観の資料の指定を、作品フォルダ内のパスにする。
/// `glossary` や `glossary.md` は `world/glossary.md`、`/` を含むものはパスとしてそのまま使う。
fn parse_world(spec: &str) -> Result<RelPath, String> {
    if spec.is_empty() {
        return Err("world: の後に、資料の名前かパスを指定してください。".to_owned());
    }
    let path = if spec.contains('/') {
        spec.to_owned()
    } else if std::path::Path::new(spec)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    {
        format!("{}/{spec}", layout::WORLD_DIR)
    } else {
        format!("{}/{spec}.md", layout::WORLD_DIR)
    };
    RelPath::new(&path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_character_with_id() {
        let target: RemoveTarget = "character:kirishima-rin".parse().unwrap();
        assert_eq!(
            target,
            RemoveTarget::Character(CharacterId::new("kirishima-rin").unwrap())
        );
    }

    #[test]
    fn parses_a_world_document_by_name_or_by_path() {
        let expected = RemoveTarget::World(RelPath::new("world/glossary.md").unwrap());
        assert_eq!("world:glossary".parse(), Ok(expected.clone()));
        assert_eq!("world:glossary.md".parse(), Ok(expected.clone()));
        assert_eq!("world:world/glossary.md".parse(), Ok(expected));
    }

    #[test]
    fn a_world_document_name_may_be_written_in_japanese() {
        let target: RemoveTarget = "world:用語集".parse().unwrap();
        assert_eq!(
            target,
            RemoveTarget::World(RelPath::new("world/用語集.md").unwrap())
        );
    }

    #[test]
    fn parses_a_scene_with_chapter_and_scene() {
        let target: RemoveTarget = "scene:01/s02".parse().unwrap();
        assert_eq!(
            target,
            RemoveTarget::Scene {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
            }
        );
    }

    #[test]
    fn rejects_unknown_kinds() {
        assert!("chapter:01".parse::<RemoveTarget>().is_err());
        assert!("rin".parse::<RemoveTarget>().is_err());
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!("character:Rin".parse::<RemoveTarget>().is_err());
        assert!("scene:01".parse::<RemoveTarget>().is_err());
        assert!("scene:1/s01".parse::<RemoveTarget>().is_err());
        assert!("scene:01/01".parse::<RemoveTarget>().is_err());
        assert!("world:../escape".parse::<RemoveTarget>().is_err());
        assert!("world:".parse::<RemoveTarget>().is_err());
    }

    #[test]
    fn the_scene_error_names_the_scene_prefix() {
        let message = "scene:01".parse::<RemoveTarget>().unwrap_err();
        assert!(message.contains("scene:01/s01"), "{message}");
    }

    #[test]
    fn into_edit_builds_the_matching_edit() {
        assert_eq!(
            RemoveTarget::Character(CharacterId::new("rin").unwrap()).into_edit(),
            StructureEdit::RemoveCharacter {
                id: CharacterId::new("rin").unwrap()
            }
        );
        assert_eq!(
            RemoveTarget::World(RelPath::new("world/glossary.md").unwrap()).into_edit(),
            StructureEdit::RemoveWorldDocument {
                path: RelPath::new("world/glossary.md").unwrap()
            }
        );
        assert_eq!(
            RemoveTarget::Scene {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
            }
            .into_edit(),
            StructureEdit::RemoveScene {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
            }
        );
    }
}
