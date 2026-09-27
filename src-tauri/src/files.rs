//! 作品フォルダ内のファイルの読み書き。`read_file` / `write_file` コマンドの中身。

use kataribe_project::{BackupMode, ContentHash, Project, RelPath, WriteCondition, WriteOptions};

use crate::error::{CommandError, CommandErrorKind};
use crate::text_file::TextFile;

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

/// 画面から渡された文字列をハッシュとして検証する。
/// `ContentHash` 自身の検証（64 桁の 16 進小文字）をそのまま再利用する。
fn parse_content_hash(raw: &str) -> Result<ContentHash, CommandError> {
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

    #[test]
    fn write_file_rejects_a_path_that_escapes_the_project() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = write_file(&project, "../escape.md", "内容", None).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }
}
