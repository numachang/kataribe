//! Tauri コマンドが画面へ返すエラー。
//!
//! `kind` は `src/api/backend.ts` の `BackendErrorKind` と対応する。`message` は
//! 各エラー型の（日本語の）`Display` をそのまま使い、ここで文言を作り直さない。

use kataribe_engine::EngineError;
use kataribe_llm::LlmError;
use kataribe_project::{PathError, ProjectError};
use serde::Serialize;

/// エラーの種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum CommandErrorKind {
    /// 保存対象が呼び出し側の想定と食い違っている（他所での書き換え・重複 ID など）。
    Conflict,
    /// 対象が見つからない（未生成のファイル、開いていない作品など）。
    NotFound,
    /// 引数や設定の値が不正。
    InvalidInput,
    /// LLM サーバーとのやり取りに関する失敗。
    Llm,
    /// 生成を中止した。
    Cancelled,
    /// ファイル・資格情報ストアなど、入出力の失敗。
    Io,
    /// 上記のどれにも当てはまらない失敗。
    Internal,
}

/// Tauri コマンドが返すエラー。`message` は利用者にそのまま見せられる。
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[error("{message}")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CommandError {
    pub kind: CommandErrorKind,
    pub message: String,
}

impl CommandError {
    #[must_use]
    pub fn new(kind: CommandErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// 作品を開いていない状態で、開いている作品を前提にした操作を呼んだときのエラー。
    #[must_use]
    pub fn project_not_open() -> Self {
        Self::new(CommandErrorKind::NotFound, "作品が開かれていません。")
    }
}

impl From<ProjectError> for CommandError {
    fn from(error: ProjectError) -> Self {
        let kind = project_error_kind(&error);
        Self::new(kind, error.to_string())
    }
}

impl From<PathError> for CommandError {
    fn from(error: PathError) -> Self {
        Self::new(CommandErrorKind::InvalidInput, error.to_string())
    }
}

impl From<LlmError> for CommandError {
    fn from(error: LlmError) -> Self {
        Self::new(CommandErrorKind::Llm, error.to_string())
    }
}

impl From<EngineError> for CommandError {
    fn from(error: EngineError) -> Self {
        let kind = engine_error_kind(&error);
        Self::new(kind, error.to_string())
    }
}

fn project_error_kind(error: &ProjectError) -> CommandErrorKind {
    match error {
        ProjectError::Conflict { .. } => CommandErrorKind::Conflict,
        ProjectError::NotFound { .. } | ProjectError::NotAProject { .. } => {
            CommandErrorKind::NotFound
        }
        ProjectError::InvalidPath(_)
        | ProjectError::InvalidId(_)
        | ProjectError::DocumentKindMismatch { .. }
        | ProjectError::DuplicateSceneId { .. }
        | ProjectError::InvalidChangeSet { .. }
        | ProjectError::AlreadyExists { .. }
        | ProjectError::PathEscapesRoot { .. }
        | ProjectError::DirectoryNotEmpty { .. } => CommandErrorKind::InvalidInput,
        ProjectError::Frontmatter { .. } | ProjectError::UnsupportedFormat { .. } => {
            CommandErrorKind::Internal
        }
        ProjectError::InvalidUtf8 { .. }
        | ProjectError::Io { .. }
        | ProjectError::FilesInUse { .. }
        | ProjectError::PartialWrite { .. }
        | ProjectError::RootNotOpenable { .. } => CommandErrorKind::Io,
    }
}

fn engine_error_kind(error: &EngineError) -> CommandErrorKind {
    match error {
        EngineError::Project(source) => project_error_kind(source),
        EngineError::Llm(_) | EngineError::InvalidOutput(_) => CommandErrorKind::Llm,
        EngineError::Yaml(_) | EngineError::Preset(_) | EngineError::Prompt(_) => {
            CommandErrorKind::Internal
        }
        EngineError::Cancelled => CommandErrorKind::Cancelled,
        EngineError::MissingPrerequisite(_)
        | EngineError::InvalidInput(_)
        | EngineError::ContextTooSmall(_) => CommandErrorKind::InvalidInput,
        EngineError::NotFound(_) => CommandErrorKind::NotFound,
        EngineError::Secret(_) | EngineError::Settings { .. } => CommandErrorKind::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_project::RelPath;
    use pretty_assertions::assert_eq;

    #[test]
    fn project_not_open_is_not_found_with_the_expected_message() {
        let error = CommandError::project_not_open();
        assert_eq!(error.kind, CommandErrorKind::NotFound);
        assert_eq!(error.message, "作品が開かれていません。");
    }

    #[test]
    fn project_conflict_maps_to_conflict() {
        let source = ProjectError::Conflict {
            path: RelPath::new("concept.md").unwrap(),
        };
        let message = source.to_string();

        let error = CommandError::from(source);

        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(error.message, message);
    }

    #[test]
    fn document_kind_mismatch_maps_to_invalid_input() {
        let source = ProjectError::DocumentKindMismatch {
            path: RelPath::new("concept.md").unwrap(),
            expected: "人物資料",
        };
        assert_eq!(
            CommandError::from(source).kind,
            CommandErrorKind::InvalidInput
        );
    }

    #[test]
    fn duplicate_scene_id_maps_to_invalid_input() {
        let source = ProjectError::DuplicateSceneId {
            path: RelPath::new("plot/chapters/01.md").unwrap(),
            id: kataribe_project::SceneId::from_number(1),
        };
        assert_eq!(
            CommandError::from(source).kind,
            CommandErrorKind::InvalidInput
        );
    }

    #[test]
    fn invalid_change_set_maps_to_invalid_input() {
        let source = ProjectError::InvalidChangeSet {
            reason: "kataribe.yaml はゴミ箱へ移せません".into(),
        };
        let message = source.to_string();

        let error = CommandError::from(EngineError::Project(source));

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
        assert_eq!(error.message, message);
    }

    #[test]
    fn project_not_a_project_maps_to_not_found() {
        let source = ProjectError::NotAProject {
            root: "C:/somewhere".into(),
        };
        let error = CommandError::from(source);
        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[test]
    fn project_directory_not_empty_maps_to_invalid_input() {
        let source = ProjectError::DirectoryNotEmpty {
            root: "C:/somewhere".into(),
        };
        assert_eq!(
            CommandError::from(source).kind,
            CommandErrorKind::InvalidInput
        );
    }

    #[test]
    fn project_io_maps_to_io() {
        let source = ProjectError::Io {
            path: RelPath::new("concept.md").unwrap(),
            source: std::io::Error::other("disk full"),
        };
        assert_eq!(CommandError::from(source).kind, CommandErrorKind::Io);
    }

    #[test]
    fn llm_error_maps_to_llm() {
        let error = CommandError::from(LlmError::Timeout);
        assert_eq!(error.kind, CommandErrorKind::Llm);
    }

    #[test]
    fn engine_cancelled_maps_to_cancelled() {
        assert_eq!(
            CommandError::from(EngineError::Cancelled).kind,
            CommandErrorKind::Cancelled
        );
    }

    #[test]
    fn engine_missing_prerequisite_maps_to_invalid_input() {
        let error = CommandError::from(EngineError::MissingPrerequisite(
            "題名がありません。".into(),
        ));
        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
        assert_eq!(error.message, "題名がありません。");
    }

    #[test]
    fn engine_wrapped_project_error_keeps_the_inner_kind_and_message() {
        let source = ProjectError::NotFound {
            path: RelPath::new("plot/synopsis.md").unwrap(),
        };
        let message = source.to_string();

        let error = CommandError::from(EngineError::Project(source));

        assert_eq!(error.kind, CommandErrorKind::NotFound);
        assert_eq!(error.message, message);
    }
}
