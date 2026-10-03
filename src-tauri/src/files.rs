//! 作品フォルダ内の文書の読み書きと分解。`read_document` / `write_document` / `parse_document` コマンドの中身。

use kataribe_project::{ContentHash, EditableDocument, ParsedDocument, Project, RelPath};

use crate::error::{CommandError, CommandErrorKind};
use crate::hashed_file::DocumentFile;

/// 作品フォルダ内のファイルを、画面で編集する形（人物資料・章立ては front matter を項目に分けた形）で
/// 読み込む。存在しなければ `not_found`。
///
/// front matter を解釈できない人物資料・章立ては、直して保存できるよう文字列のまま返し、理由を添える。
pub fn read_document(project: &Project, path: &str) -> Result<DocumentFile, CommandError> {
    let path = RelPath::new(path)?;
    let file = project.read_document(&path)?;
    Ok(DocumentFile::from(file))
}

/// 文字列を、`path` の種類に応じて画面で編集する形に分ける。作品もファイルも使わない。
///
/// 生成した変更案のように、まだ書いていない内容を [`read_document`] と同じ分け方で見せるために使う。
/// パスが作品内の相対パスとして不正なら `invalid_input`。
pub fn parse_document(path: &str, content: &str) -> Result<ParsedDocument, CommandError> {
    let path = RelPath::new(path)?;
    Ok(kataribe_project::parse_document(&path, content))
}

/// 画面で編集した文書を書き込み、新しい内容のハッシュを返す。
///
/// `expected_hash` が `None` なら新規作成としてのみ許可し、既にファイルがあれば競合になる。
/// `Some` なら、そのハッシュから内容が変わっていなければ上書きを許可する。
/// パスの種類に合わない人物資料・章立ては `invalid_input`。バックアップは間引きながら作る。
pub fn write_document(
    project: &Project,
    path: &str,
    document: &EditableDocument,
    expected_hash: Option<&str>,
) -> Result<String, CommandError> {
    let path = RelPath::new(path)?;
    let expected = expected_hash.map(parse_content_hash).transpose()?;
    let hash = project.write_document(&path, document, expected.as_ref())?;
    Ok(hash.to_string())
}

/// 画面から渡された文字列をハッシュとして検証する。
/// `ContentHash` 自身の検証（64 桁の 16 進小文字）をそのまま再利用する。
pub(crate) fn parse_content_hash(raw: &str) -> Result<ContentHash, CommandError> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned())).map_err(|_| {
        CommandError::new(
            CommandErrorKind::InvalidInput,
            format!("ハッシュの形式が正しくありません: {raw}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_project::{
        BackupMode, FORMAT_VERSION, Manifest, Project, Rating, TextFile, WriteCondition,
        WriteOptions,
    };
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn sample_manifest() -> Manifest {
        Manifest {
            format: FORMAT_VERSION,
            title: "テスト作品".to_owned(),
            author: None,
            genre: "general".to_owned(),
            genre_note: None,
            rating: Rating::General,
            target_length: 10_000,
            idea: "静かな夜の物語".to_owned(),
            settings: None,
            extra: std::collections::BTreeMap::new(),
        }
    }

    fn open_project(dir: &TempDir) -> Project {
        Project::create(dir.path(), &sample_manifest()).unwrap();
        Project::open(dir.path()).unwrap()
    }

    /// 画面を介さずに、作品フォルダへファイルを置く（テストの準備）。置いた内容のハッシュを返す。
    fn put(project: &Project, path: &str, content: &str) -> String {
        project
            .store()
            .write_text(
                &RelPath::new(path).unwrap(),
                content,
                WriteOptions {
                    condition: WriteCondition::Any,
                    backup: BackupMode::Never,
                },
            )
            .unwrap()
            .to_string()
    }

    fn read_text(project: &Project, path: &str) -> TextFile {
        project
            .store()
            .read_text(&RelPath::new(path).unwrap())
            .unwrap()
    }

    const CHARACTER_TEXT: &str = "---\n# 手で足したコメント\nname: 霧島 凛\nrole: 主人公\nsecret: 実は依頼人の妹\n---\n古い本文\n";

    #[test]
    fn read_document_splits_a_character_into_meta_and_body() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = put(&project, "characters/rin.md", CHARACTER_TEXT);

        let file = read_document(&project, "characters/rin.md").unwrap();

        let EditableDocument::Character { meta, body } = file.document else {
            panic!("人物資料として読めるはず");
        };
        assert_eq!(meta.name, "霧島 凛");
        assert_eq!(body, "古い本文\n");
        assert_eq!(file.hash, hash);
        assert_eq!(file.parse_error, None);
    }

    #[test]
    fn read_document_returns_a_broken_character_as_text_with_the_reason() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put(&project, "characters/rin.md", "---\nname: [\n---\n");

        let file = read_document(&project, "characters/rin.md").unwrap();

        assert_eq!(
            file.document,
            EditableDocument::Text {
                content: "---\nname: [\n---\n".to_owned()
            }
        );
        let reason = file.parse_error.expect("解釈できなかった理由が付くはず");
        assert!(reason.contains("characters/rin.md"), "reason: {reason}");
    }

    #[test]
    fn parse_document_splits_a_character_without_a_project() {
        let parsed = parse_document("characters/rin.md", CHARACTER_TEXT).unwrap();

        let EditableDocument::Character { meta, body } = parsed.document else {
            panic!("人物資料として分けられるはず");
        };
        assert_eq!(meta.name, "霧島 凛");
        assert_eq!(body, "古い本文\n");
        assert_eq!(parsed.parse_error, None);
    }

    #[test]
    fn parse_document_returns_broken_yaml_as_text_with_the_reason() {
        let parsed = parse_document("characters/rin.md", "---\nname: [\n---\n").unwrap();

        assert_eq!(
            parsed.document,
            EditableDocument::Text {
                content: "---\nname: [\n---\n".to_owned()
            }
        );
        let reason = parsed.parse_error.expect("解釈できなかった理由が付くはず");
        assert!(reason.contains("characters/rin.md"), "reason: {reason}");
    }

    #[test]
    fn parse_document_rejects_an_invalid_path_as_invalid_input() {
        let error = parse_document("../characters/rin.md", CHARACTER_TEXT).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }

    #[test]
    fn read_document_reports_not_found_for_a_missing_file() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = read_document(&project, "characters/rin.md").unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[test]
    fn write_document_keeps_the_yaml_when_only_the_body_changes() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = put(&project, "characters/rin.md", CHARACTER_TEXT);
        let EditableDocument::Character { meta, .. } = read_document(&project, "characters/rin.md")
            .unwrap()
            .document
        else {
            panic!("人物資料として読めるはず");
        };
        let document = EditableDocument::Character {
            meta,
            body: "新しい本文\n".to_owned(),
        };

        let new_hash =
            write_document(&project, "characters/rin.md", &document, Some(&hash)).unwrap();

        let read = read_text(&project, "characters/rin.md");
        assert_eq!(
            read.content,
            CHARACTER_TEXT.replace("古い本文", "新しい本文")
        );
        assert_eq!(read.hash.to_string(), new_hash);
    }

    #[test]
    fn write_document_conflicts_when_expected_hash_is_stale() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = put(&project, "concept.md", "初回");
        put(&project, "concept.md", "外での更新");
        let document = EditableDocument::Text {
            content: "画面の内容".to_owned(),
        };

        let error = write_document(&project, "concept.md", &document, Some(&hash)).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(read_text(&project, "concept.md").content, "外での更新");
    }

    #[test]
    fn write_document_creates_a_new_file_when_expected_hash_is_absent() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let document = EditableDocument::Text {
            content: "企画".to_owned(),
        };

        let hash = write_document(&project, "concept.md", &document, None).unwrap();

        assert_eq!(read_text(&project, "concept.md").hash.to_string(), hash);
    }

    #[test]
    fn write_document_conflicts_when_creating_over_an_existing_file() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put(&project, "concept.md", "既にある企画");
        let document = EditableDocument::Text {
            content: "新規のつもりの企画".to_owned(),
        };

        let error = write_document(&project, "concept.md", &document, None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(read_text(&project, "concept.md").content, "既にある企画");
    }

    #[test]
    fn write_document_rejects_a_document_that_does_not_fit_the_path() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let document = EditableDocument::Chapter {
            meta: kataribe_project::ChapterMeta::default(),
            body: String::new(),
        };

        let error = write_document(&project, "characters/rin.md", &document, None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }

    #[test]
    fn write_document_rejects_a_malformed_expected_hash() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let document = EditableDocument::Text {
            content: "内容".to_owned(),
        };

        let error =
            write_document(&project, "concept.md", &document, Some("not-a-hash")).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }

    #[test]
    fn write_document_rejects_a_path_that_escapes_the_project() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let document = EditableDocument::Text {
            content: "内容".to_owned(),
        };

        let error = write_document(&project, "../escape.md", &document, None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }
}
