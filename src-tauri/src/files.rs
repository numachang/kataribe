//! 作品フォルダ内のファイルの読み書き。`read_file` / `write_file` コマンドの中身。

use kataribe_project::{
    BackupMode, ContentHash, EditableDocument, Project, RelPath, WriteCondition, WriteOptions,
};

use crate::error::{CommandError, CommandErrorKind};
use crate::text_file::{DocumentFile, TextFile};

/// 作品フォルダ内のファイルを読み込む。存在しなければ `not_found`。
pub fn read_file(project: &Project, path: &str) -> Result<TextFile, CommandError> {
    let path = RelPath::new(path)?;
    let file = project.store().read_text(&path)?;
    Ok(TextFile::from(file))
}

/// 作品フォルダ内のファイルを書き込み、新しい内容のハッシュを返す。
///
/// `expected_hash` が `None` なら新規作成としてのみ許可し、既にファイルがあれば競合になる。
/// `Some` なら、そのハッシュから内容が変わっていなければ上書きを許可する。
/// バックアップは間引きながら作る（[`BackupMode::Throttled`]）。
pub fn write_file(
    project: &Project,
    path: &str,
    content: &str,
    expected_hash: Option<&str>,
) -> Result<String, CommandError> {
    let path = RelPath::new(path)?;
    let condition = match expected_hash {
        None => WriteCondition::Absent,
        Some(hash) => WriteCondition::Matches(parse_content_hash(hash)?),
    };
    let hash = project.store().write_text(
        &path,
        content,
        WriteOptions {
            condition,
            backup: BackupMode::Throttled,
        },
    )?;
    Ok(hash.to_string())
}

/// 作品フォルダ内のファイルを、画面で編集する形（人物資料・章立ては front matter を項目に分けた形）で
/// 読み込む。存在しなければ `not_found`。
///
/// front matter を解釈できない人物資料・章立ては、直して保存できるよう文字列のまま返し、理由を添える。
pub fn read_document(project: &Project, path: &str) -> Result<DocumentFile, CommandError> {
    let path = RelPath::new(path)?;
    let file = project.read_document(&path)?;
    Ok(DocumentFile::from(file))
}

/// 画面で編集した文書を書き込み、新しい内容のハッシュを返す。
///
/// `expected_hash` の意味は [`write_file`] と同じ。パスの種類に合わない人物資料・章立ては `invalid_input`。
/// バックアップは間引きながら作る（[`BackupMode::Throttled`]）。
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
    use kataribe_project::{FORMAT_VERSION, Manifest, Project, Rating};
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

    #[test]
    fn write_file_creates_a_new_file_when_expected_hash_is_absent() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let hash = write_file(&project, "concept.md", "本文", None).unwrap();

        let read = read_file(&project, "concept.md").unwrap();
        assert_eq!(read.content, "本文");
        assert_eq!(read.hash, hash);
    }

    #[test]
    fn write_file_conflicts_when_creating_over_an_existing_file() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        write_file(&project, "concept.md", "初回", None).unwrap();

        let error = write_file(&project, "concept.md", "二回目", None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Conflict);
    }

    #[test]
    fn write_file_conflicts_when_expected_hash_is_stale() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = write_file(&project, "concept.md", "初回", None).unwrap();
        write_file(&project, "concept.md", "更新", Some(&hash)).unwrap();

        let error = write_file(&project, "concept.md", "さらに更新", Some(&hash)).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Conflict);
    }

    #[test]
    fn write_file_succeeds_when_expected_hash_matches() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = write_file(&project, "concept.md", "初回", None).unwrap();

        let new_hash = write_file(&project, "concept.md", "更新後", Some(&hash)).unwrap();

        assert_ne!(hash, new_hash);
        assert_eq!(read_file(&project, "concept.md").unwrap().content, "更新後");
    }

    #[test]
    fn write_file_rejects_a_malformed_expected_hash() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = write_file(&project, "concept.md", "内容", Some("not-a-hash")).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }

    #[test]
    fn read_file_reports_not_found_for_a_missing_file() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = read_file(&project, "concept.md").unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    const CHARACTER_TEXT: &str = "---\n# 手で足したコメント\nname: 霧島 凛\nrole: 主人公\nsecret: 実は依頼人の妹\n---\n古い本文\n";

    #[test]
    fn read_document_splits_a_character_into_meta_and_body() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = write_file(&project, "characters/rin.md", CHARACTER_TEXT, None).unwrap();

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
        write_file(&project, "characters/rin.md", "---\nname: [\n---\n", None).unwrap();

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
        let hash = write_file(&project, "characters/rin.md", CHARACTER_TEXT, None).unwrap();
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

        let read = read_file(&project, "characters/rin.md").unwrap();
        assert_eq!(
            read.content,
            CHARACTER_TEXT.replace("古い本文", "新しい本文")
        );
        assert_eq!(read.hash, new_hash);
    }

    #[test]
    fn write_document_conflicts_when_expected_hash_is_stale() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = write_file(&project, "concept.md", "初回", None).unwrap();
        write_file(&project, "concept.md", "外での更新", Some(&hash)).unwrap();
        let document = EditableDocument::Text {
            content: "画面の内容".to_owned(),
        };

        let error = write_document(&project, "concept.md", &document, Some(&hash)).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(
            read_file(&project, "concept.md").unwrap().content,
            "外での更新"
        );
    }

    #[test]
    fn write_document_creates_a_new_file_when_expected_hash_is_absent() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let document = EditableDocument::Text {
            content: "企画".to_owned(),
        };

        let hash = write_document(&project, "concept.md", &document, None).unwrap();

        assert_eq!(read_file(&project, "concept.md").unwrap().hash, hash);
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

    #[test]
    fn write_file_rejects_a_path_that_escapes_the_project() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = write_file(&project, "../escape.md", "内容", None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }
}
