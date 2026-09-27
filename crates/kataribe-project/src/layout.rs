//! 作品フォルダの構成（§2）の唯一の定義。
//!
//! パスの文字列はここにしか書かない。他のモジュールはこの定数・関数を通して参照する。

use crate::model::{ChapterId, CharacterId, SceneId};
use crate::path::RelPath;

/// 作品情報。
pub const MANIFEST: &str = "kataribe.yaml";
/// 企画。
pub const CONCEPT: &str = "concept.md";
/// 文体ガイド。
pub const STYLE: &str = "style.md";
/// 世界観フォルダ。
pub const WORLD_DIR: &str = "world";
/// 世界観の概要。
pub const WORLD_OVERVIEW: &str = "world/overview.md";
/// 登場人物フォルダ。
pub const CHARACTERS_DIR: &str = "characters";
/// 全体あらすじ。
pub const SYNOPSIS: &str = "plot/synopsis.md";
/// 章のフォルダ。
pub const CHAPTERS_DIR: &str = "plot/chapters";
/// 本文フォルダ。
pub const MANUSCRIPT_DIR: &str = "manuscript";
/// アプリの内部データ（Git 管理外）。
pub const INTERNAL_DIR: &str = ".kataribe";
/// 生成の中間データ。
pub const CACHE_DIR: &str = ".kataribe/cache";
/// 上書き前のバックアップ。
pub const BACKUPS_DIR: &str = ".kataribe/backups";
/// 削除したファイルの置き場。
pub const TRASH_DIR: &str = ".kataribe/trash";

/// `.kataribe/` を Git 管理から除外するファイル。
pub const GITIGNORE: &str = ".gitignore";
/// [`GITIGNORE`] の内容。[`INTERNAL_DIR`] と一致させること。
pub const GITIGNORE_CONTENT: &str = ".kataribe/\n";
/// 改行を LF に固定するファイル。
pub const GITATTRIBUTES: &str = ".gitattributes";
/// [`GITATTRIBUTES`] の内容。
pub const GITATTRIBUTES_CONTENT: &str = "* text=auto eol=lf\n";

/// `characters/<id>.md` のパス。
#[must_use]
pub fn character_path(id: &CharacterId) -> RelPath {
    RelPath::trusted(format!("{CHARACTERS_DIR}/{}.md", id.as_str()))
}

/// `plot/chapters/<NN>.md` のパス。
#[must_use]
pub fn chapter_path(id: &ChapterId) -> RelPath {
    RelPath::trusted(format!("{CHAPTERS_DIR}/{id}.md"))
}

/// `manuscript/<NN>/<scene-id>.txt` のパス。
#[must_use]
pub fn scene_text_path(chapter: &ChapterId, scene: &SceneId) -> RelPath {
    RelPath::trusted(format!("{MANUSCRIPT_DIR}/{chapter}/{scene}.txt"))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn constants_are_valid_relative_paths() {
        for constant in [
            MANIFEST,
            CONCEPT,
            STYLE,
            WORLD_OVERVIEW,
            SYNOPSIS,
            CACHE_DIR,
        ] {
            assert!(
                RelPath::new(constant).is_ok(),
                "invalid constant: {constant}"
            );
        }
    }

    #[test]
    fn character_path_builds_expected_location() {
        let id = CharacterId::new("kirishima-rin").unwrap();
        assert_eq!(character_path(&id).as_str(), "characters/kirishima-rin.md");
    }

    #[test]
    fn chapter_path_builds_expected_location() {
        let id = ChapterId::from_number(1);
        assert_eq!(chapter_path(&id).as_str(), "plot/chapters/01.md");
    }

    #[test]
    fn scene_text_path_builds_expected_location() {
        let chapter = ChapterId::from_number(1);
        let scene = SceneId::from_number(1);
        assert_eq!(
            scene_text_path(&chapter, &scene).as_str(),
            "manuscript/01/s01.txt"
        );
    }
}
