//! 作品フォルダの構成（§2）の唯一の定義。
//!
//! パスの文字列はここにしか書かない。他のモジュールはこの定数・関数を通して参照する。

use crate::model::{ChapterId, CharacterId, SceneId, WorldDocumentName};
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

/// `world/<name>.md`（世界観の資料）のパス。
#[must_use]
pub fn world_document_path(name: &WorldDocumentName) -> RelPath {
    RelPath::trusted(format!("{WORLD_DIR}/{}.md", name.as_str()))
}

/// `characters/` 直下の Markdown か。人物資料の判定。
///
/// 手で足したファイルも目次に出るので、ファイル名は [`CharacterId`] の規則に合っていなくてよい
/// （`Rin.md` や `凛.md` も人物資料として消せるように）。
#[must_use]
pub fn is_character_document(path: &RelPath) -> bool {
    is_markdown_directly_in(path, CHARACTERS_DIR)
}

/// `world/` 直下の、世界観の概要（`overview.md`）以外の Markdown か。利用者が足した世界観の資料の判定。
///
/// 手で足したファイルも対象にするので、ファイル名は [`WorldDocumentName`] の規則に合っていなくてよい。
/// Windows は大文字小文字を区別せず、`world/Overview.md` も概要として読めてしまうので、
/// 概要かどうかは大文字小文字を無視して調べる（概要を資料として消せないように）。
#[must_use]
pub fn is_additional_world_document(path: &RelPath) -> bool {
    is_markdown_directly_in(path, WORLD_DIR) && !path.as_str().eq_ignore_ascii_case(WORLD_OVERVIEW)
}

/// `dir` の直下の Markdown か。拡張子の大文字小文字は区別しない（Windows では `a.MD` も `a.md` と同じファイル）。
fn is_markdown_directly_in(path: &RelPath, dir: &str) -> bool {
    path.parent().is_some_and(|parent| parent.as_str() == dir)
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

/// `plot/chapters/<NN>.md` のパス。
#[must_use]
pub fn chapter_path(id: &ChapterId) -> RelPath {
    RelPath::trusted(format!("{CHAPTERS_DIR}/{id}.md"))
}

/// パスが指すファイルの種類。画面が front matter を項目に分けて扱うかどうかを決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentKind {
    /// `characters/<有効な id>.md`。
    Character(CharacterId),
    /// `plot/chapters/<有効な NN>.md`。
    Chapter(ChapterId),
    /// 上のどれでもないもの（サブフォルダの下・id として無効な名前・ほかのファイル）。
    Other,
}

/// パスから文書の種類を引く。
///
/// [`crate::project::Project::characters`] / [`crate::project::Project::chapters`] が
/// 人物・章として拾うファイルと、ちょうど同じ条件で判定する。
#[must_use]
pub fn document_kind(path: &RelPath) -> DocumentKind {
    if let Some(id) =
        markdown_stem_in(path, CHARACTERS_DIR).and_then(|stem| CharacterId::new(stem).ok())
    {
        return DocumentKind::Character(id);
    }
    if let Some(id) =
        markdown_stem_in(path, CHAPTERS_DIR).and_then(|stem| ChapterId::new(stem).ok())
    {
        return DocumentKind::Chapter(id);
    }
    DocumentKind::Other
}

/// `dir` の直下にある Markdown ファイルなら、その拡張子を除いた名前。
fn markdown_stem_in<'a>(path: &'a RelPath, dir: &str) -> Option<&'a str> {
    let is_directly_in_dir = path.parent().is_some_and(|parent| parent.as_str() == dir);
    (is_directly_in_dir && path.extension() == Some("md")).then(|| path.file_stem())
}

/// `manuscript/<NN>` のパス（章の本文を入れるフォルダ）。
#[must_use]
pub fn manuscript_chapter_dir(chapter: &ChapterId) -> RelPath {
    RelPath::trusted(format!("{MANUSCRIPT_DIR}/{chapter}"))
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
    fn world_document_path_builds_expected_location() {
        let name = WorldDocumentName::new("glossary").unwrap();
        assert_eq!(world_document_path(&name).as_str(), "world/glossary.md");
    }

    #[test]
    fn world_document_names_never_point_at_the_overview() {
        let overview_stem = RelPath::new(WORLD_OVERVIEW).unwrap();
        assert!(WorldDocumentName::new(overview_stem.file_stem()).is_err());
        let generated = WorldDocumentName::from_hint(overview_stem.file_stem(), &[]);
        assert_ne!(world_document_path(&generated).as_str(), WORLD_OVERVIEW);
    }

    #[test]
    fn additional_world_documents_are_markdown_files_directly_under_world_except_the_overview() {
        let is_additional = |path: &str| is_additional_world_document(&RelPath::new(path).unwrap());
        assert!(is_additional("world/glossary.md"));
        assert!(is_additional("world/用語集.md"));
        assert!(!is_additional("world/overview.md"));
        assert!(!is_additional("world/notes.txt"));
        assert!(is_additional("world/glossary.MD"));
        assert!(!is_additional("world/maps/town.md"));
        assert!(!is_additional("characters/rin.md"));
        assert!(!is_additional("world"));
    }

    #[test]
    fn character_documents_are_markdown_files_directly_under_characters_whatever_their_name() {
        let is_character = |path: &str| is_character_document(&RelPath::new(path).unwrap());
        assert!(is_character("characters/kirishima-rin.md"));
        assert!(is_character("characters/Rin.md"));
        assert!(is_character("characters/凛.md"));
        assert!(is_character("characters/rin.MD"));
        assert!(!is_character("characters/rin.txt"));
        assert!(!is_character("characters/old/rin.md"));
        assert!(!is_character("world/rin.md"));
        assert!(!is_character("characters"));
    }

    #[test]
    fn the_overview_is_never_an_additional_document_whatever_its_letter_case() {
        // Windows では、これらは全部 world/overview.md と同じファイル
        for path in [
            "world/Overview.md",
            "world/OVERVIEW.md",
            "world/overview.MD",
            "world/OVERVIEW.MD",
        ] {
            assert!(
                !is_additional_world_document(&RelPath::new(path).unwrap()),
                "{path}"
            );
        }
    }

    #[test]
    fn chapter_path_builds_expected_location() {
        let id = ChapterId::from_number(1);
        assert_eq!(chapter_path(&id).as_str(), "plot/chapters/01.md");
    }

    fn kind_of(path: &str) -> DocumentKind {
        document_kind(&RelPath::new(path).unwrap())
    }

    #[test]
    fn character_file_is_a_character_document() {
        assert_eq!(
            kind_of("characters/kirishima-rin.md"),
            DocumentKind::Character(CharacterId::new("kirishima-rin").unwrap())
        );
    }

    #[test]
    fn chapter_file_is_a_chapter_document() {
        assert_eq!(
            kind_of("plot/chapters/01.md"),
            DocumentKind::Chapter(ChapterId::from_number(1))
        );
        assert_eq!(
            kind_of("plot/chapters/100.md"),
            DocumentKind::Chapter(ChapterId::new("100").unwrap())
        );
    }

    #[test]
    fn paths_made_by_the_layout_functions_are_classified_back() {
        let character = CharacterId::new("sato-kenji").unwrap();
        let chapter = ChapterId::from_number(12);
        assert_eq!(
            document_kind(&character_path(&character)),
            DocumentKind::Character(character)
        );
        assert_eq!(
            document_kind(&chapter_path(&chapter)),
            DocumentKind::Chapter(chapter)
        );
    }

    #[test]
    fn invalid_ids_are_other_documents() {
        for path in [
            "characters/Kirishima.md",
            "characters/霧島.md",
            "characters/-rin.md",
            "characters/.md",
            "plot/chapters/1.md",
            "plot/chapters/1000.md",
            "plot/chapters/ab.md",
        ] {
            assert_eq!(kind_of(path), DocumentKind::Other, "path: {path}");
        }
    }

    #[test]
    fn files_below_a_subfolder_are_other_documents() {
        assert_eq!(kind_of("characters/old/rin.md"), DocumentKind::Other);
        assert_eq!(kind_of("plot/chapters/draft/01.md"), DocumentKind::Other);
    }

    #[test]
    fn files_in_other_folders_or_with_other_extensions_are_other_documents() {
        for path in [
            "concept.md",
            "world/rin.md",
            "plot/01.md",
            "plot/synopsis.md",
            "characters/rin.txt",
            "characters/rin.MD",
            "plot/chapters/01.txt",
            "characters",
            "plot/chapters",
        ] {
            assert_eq!(kind_of(path), DocumentKind::Other, "path: {path}");
        }
    }

    #[test]
    fn manuscript_chapter_dir_builds_expected_location() {
        let chapter = ChapterId::from_number(2);
        assert_eq!(manuscript_chapter_dir(&chapter).as_str(), "manuscript/02");
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
