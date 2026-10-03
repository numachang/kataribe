//! `remove` / `move` サブコマンドの `TARGET` 引数のミニ言語。
//!
//! `character:<id>` のような `種類:引数` の形を解析し、[`kataribe_engine::StructureEdit`] に変換する。
//! 書式は `generate` の `TASK`（[`crate::task_spec`]）と同じ。

use std::str::FromStr;

use kataribe_engine::StructureEdit;
use kataribe_project::{ChapterId, RelPath, SceneId, layout};

use crate::task_spec::parse_chapter_and_scene;

/// コマンドラインの `TARGET` 引数を解析した結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveTarget {
    /// `characters/` 直下の人物資料のパス。
    Character(RelPath),
    /// `world/` 直下の資料のパス。
    World(RelPath),
    /// 章。章立てと本文のフォルダがゴミ箱へ移り、後ろの章の番号が詰まる。
    Chapter(ChapterId),
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
            RemoveTarget::Character(path) => StructureEdit::RemoveCharacter { path },
            RemoveTarget::World(path) => StructureEdit::RemoveWorldDocument { path },
            RemoveTarget::Chapter(chapter) => StructureEdit::RemoveChapter { chapter },
            RemoveTarget::Scene { chapter, scene } => StructureEdit::RemoveScene { chapter, scene },
        }
    }
}

/// `move` の `TARGET` 引数を解析した結果。並べ替えられるのは、順番のあるものだけ
/// （世界観の資料には順番が無い）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveTarget {
    /// `characters/` 直下の人物資料のパス。
    Character(RelPath),
    /// 章。動く範囲の章の番号が振り直される。
    Chapter(ChapterId),
    Scene {
        chapter: ChapterId,
        scene: SceneId,
    },
}

impl MoveTarget {
    /// 構成の操作にする。`position` は、並べ替えたあとの位置（0 始まり）。
    #[must_use]
    pub fn into_edit(self, position: usize) -> StructureEdit {
        match self {
            MoveTarget::Character(path) => StructureEdit::MoveCharacter { path, position },
            MoveTarget::Chapter(chapter) => StructureEdit::MoveChapter { chapter, position },
            MoveTarget::Scene { chapter, scene } => StructureEdit::MoveScene {
                chapter,
                scene,
                position,
            },
        }
    }
}

impl FromStr for MoveTarget {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Some(spec) = input.strip_prefix("character:") {
            return parse_character(spec).map(MoveTarget::Character);
        }
        if let Some(spec) = input.strip_prefix("chapter:") {
            return parse_chapter(spec).map(MoveTarget::Chapter);
        }
        if let Some(rest) = input.strip_prefix("scene:") {
            return parse_chapter_and_scene(rest, "scene")
                .map(|(chapter, scene)| MoveTarget::Scene { chapter, scene });
        }
        if input.starts_with("world:") {
            return Err("世界観の資料には順番が無いので、並べ替えられません。".to_owned());
        }
        Err(format!(
            "不明な対象です: {input}\n\
             次のいずれかを指定してください: character:<id> | chapter:<NN> | scene:<NN>/<sNN>"
        ))
    }
}

impl FromStr for RemoveTarget {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Some(spec) = input.strip_prefix("character:") {
            return parse_character(spec).map(RemoveTarget::Character);
        }
        if let Some(spec) = input.strip_prefix("world:") {
            return parse_world(spec).map(RemoveTarget::World);
        }
        if let Some(spec) = input.strip_prefix("chapter:") {
            return parse_chapter(spec).map(RemoveTarget::Chapter);
        }
        if let Some(rest) = input.strip_prefix("scene:") {
            return parse_chapter_and_scene(rest, "scene")
                .map(|(chapter, scene)| RemoveTarget::Scene { chapter, scene });
        }
        Err(format!(
            "不明な対象です: {input}\n\
             次のいずれかを指定してください: character:<id> | world:<name または path> | chapter:<NN> | scene:<NN>/<sNN>"
        ))
    }
}

/// 章の指定（`01` など）を章の id にする。
fn parse_chapter(spec: &str) -> Result<ChapterId, String> {
    spec.parse().map_err(|_| {
        format!("chapter: の後に、章の番号（01 など、2〜3 桁）を指定してください: {spec}")
    })
}

/// 人物の指定を、`characters/` 直下のパスにする。`rin` や `rin.md` は `characters/rin.md`。
///
/// 人物 ID の規則には照らさない。手で足した `Rin.md` や `凛.md` のように、規則に合わない名前の
/// 人物資料も（目次に出るので）指定して消せるようにするため。
fn parse_character(spec: &str) -> Result<RelPath, String> {
    if spec.is_empty() || spec.contains('/') {
        return Err(
            "character: の後に、人物の ID（characters/ 直下のファイル名）を指定してください。"
                .to_owned(),
        );
    }
    markdown_path_in(layout::CHARACTERS_DIR, spec)
}

/// 世界観の資料の指定を、作品フォルダ内のパスにする。
/// `glossary` や `glossary.md` は `world/glossary.md`、`/` を含むものはパスとしてそのまま使う。
fn parse_world(spec: &str) -> Result<RelPath, String> {
    if spec.is_empty() {
        return Err("world: の後に、資料の名前かパスを指定してください。".to_owned());
    }
    if spec.contains('/') {
        return RelPath::new(spec).map_err(|error| error.to_string());
    }
    markdown_path_in(layout::WORLD_DIR, spec)
}

/// `dir` 直下の Markdown のパス。`name` に拡張子 `.md` が無ければ付ける。
fn markdown_path_in(dir: &str, name: &str) -> Result<RelPath, String> {
    let has_extension = std::path::Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
    let file_name = if has_extension {
        name.to_owned()
    } else {
        format!("{name}.md")
    };
    RelPath::new(&format!("{dir}/{file_name}")).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(path: &str) -> RemoveTarget {
        RemoveTarget::Character(RelPath::new(path).unwrap())
    }

    #[test]
    fn parses_a_character_by_id_or_by_file_name() {
        let expected = character("characters/kirishima-rin.md");
        assert_eq!("character:kirishima-rin".parse(), Ok(expected.clone()));
        assert_eq!("character:kirishima-rin.md".parse(), Ok(expected));
    }

    #[test]
    fn a_character_name_that_breaks_the_id_rules_is_still_a_target() {
        assert_eq!("character:Rin".parse(), Ok(character("characters/Rin.md")));
        assert_eq!("character:凛".parse(), Ok(character("characters/凛.md")));
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
    fn parses_a_chapter_by_its_number() {
        assert_eq!(
            "chapter:03".parse(),
            Ok(RemoveTarget::Chapter(ChapterId::from_number(3)))
        );
        assert_eq!(
            "chapter:003".parse(),
            Ok(RemoveTarget::Chapter(ChapterId::new("003").unwrap()))
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
        assert!("chapters:01".parse::<RemoveTarget>().is_err());
        assert!("rin".parse::<RemoveTarget>().is_err());
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!("character:".parse::<RemoveTarget>().is_err());
        assert!("character:old/rin".parse::<RemoveTarget>().is_err());
        assert!("chapter:".parse::<RemoveTarget>().is_err());
        assert!("chapter:1".parse::<RemoveTarget>().is_err());
        assert!("chapter:01/s01".parse::<RemoveTarget>().is_err());
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
    fn a_move_target_parses_a_character_a_chapter_and_a_scene_like_a_remove_target() {
        assert_eq!(
            "character:Rin".parse(),
            Ok(MoveTarget::Character(
                RelPath::new("characters/Rin.md").unwrap()
            ))
        );
        assert_eq!(
            "chapter:03".parse(),
            Ok(MoveTarget::Chapter(ChapterId::from_number(3)))
        );
        assert_eq!(
            "scene:01/s02".parse(),
            Ok(MoveTarget::Scene {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
            })
        );
    }

    #[test]
    fn a_world_document_cannot_be_moved_because_it_has_no_order() {
        let message = "world:glossary".parse::<MoveTarget>().unwrap_err();

        assert!(message.contains("順番が無い"), "{message}");
    }

    #[test]
    fn a_move_target_rejects_unknown_kinds_and_bad_arguments() {
        let unknown = "rin".parse::<MoveTarget>().unwrap_err();
        assert_eq!(
            unknown,
            "不明な対象です: rin\n\
             次のいずれかを指定してください: character:<id> | chapter:<NN> | scene:<NN>/<sNN>"
        );
        assert!("character:".parse::<MoveTarget>().is_err());
        assert!("chapter:1".parse::<MoveTarget>().is_err());
        assert!("scene:01".parse::<MoveTarget>().is_err());
    }

    #[test]
    fn a_move_target_builds_the_matching_edit_with_the_position() {
        assert_eq!(
            MoveTarget::Character(RelPath::new("characters/rin.md").unwrap()).into_edit(2),
            StructureEdit::MoveCharacter {
                path: RelPath::new("characters/rin.md").unwrap(),
                position: 2,
            }
        );
        assert_eq!(
            MoveTarget::Chapter(ChapterId::from_number(3)).into_edit(0),
            StructureEdit::MoveChapter {
                chapter: ChapterId::from_number(3),
                position: 0,
            }
        );
        assert_eq!(
            MoveTarget::Scene {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
            }
            .into_edit(4),
            StructureEdit::MoveScene {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
                position: 4,
            }
        );
    }

    #[test]
    fn into_edit_builds_the_matching_edit() {
        assert_eq!(
            character("characters/rin.md").into_edit(),
            StructureEdit::RemoveCharacter {
                path: RelPath::new("characters/rin.md").unwrap()
            }
        );
        assert_eq!(
            RemoveTarget::World(RelPath::new("world/glossary.md").unwrap()).into_edit(),
            StructureEdit::RemoveWorldDocument {
                path: RelPath::new("world/glossary.md").unwrap()
            }
        );
        assert_eq!(
            RemoveTarget::Chapter(ChapterId::from_number(3)).into_edit(),
            StructureEdit::RemoveChapter {
                chapter: ChapterId::from_number(3)
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
