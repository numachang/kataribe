//! 作品の作成・読み込み。`create_project` / `open_project` コマンドの中身。

use std::path::Path;

use kataribe_engine::{NewProject, ProjectOverview};
use kataribe_project::Project;

use crate::error::CommandError;

/// `folder` に新しい作品を作り、開いた状態にして概要を返す。
pub fn create_project(
    folder: &Path,
    new_project: NewProject,
) -> Result<(Project, ProjectOverview), CommandError> {
    let project = kataribe_engine::create_project(folder, new_project)?;
    let overview = kataribe_engine::overview(&project)?;
    Ok((project, overview))
}

/// 既存の作品フォルダを開き、概要を返す。
pub fn open_project(folder: &Path) -> Result<(Project, ProjectOverview), CommandError> {
    let project = Project::open(folder)?;
    let overview = kataribe_engine::overview(&project)?;
    Ok((project, overview))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_project::Rating;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use crate::error::CommandErrorKind;

    fn sample_new_project() -> NewProject {
        NewProject {
            title: "みさき館の殺人".to_owned(),
            author: Some("沼田".to_owned()),
            genre: "mystery".to_owned(),
            genre_note: None,
            rating: Rating::General,
            target_length: 30_000,
            idea: "嵐で孤立した岬の洋館で…".to_owned(),
        }
    }

    #[test]
    fn create_project_opens_the_new_project_and_returns_its_overview() {
        let dir = TempDir::new().unwrap();
        let folder = dir.path().join("my-novel");

        let (project, overview) = create_project(&folder, sample_new_project()).unwrap();

        assert_eq!(overview.title, "みさき館の殺人");
        assert_eq!(overview.target_length, 30_000);
        assert_eq!(project.root(), folder);
    }

    #[test]
    fn create_project_rejects_a_non_empty_folder() {
        let dir = TempDir::new().unwrap();
        let folder = dir.path().join("my-novel");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("existing.txt"), "何か").unwrap();

        let error = create_project(&folder, sample_new_project()).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }

    #[test]
    fn open_project_reads_back_what_create_project_wrote() {
        let dir = TempDir::new().unwrap();
        let folder = dir.path().join("my-novel");
        create_project(&folder, sample_new_project()).unwrap();

        let (_, overview) = open_project(&folder).unwrap();

        assert_eq!(overview.title, "みさき館の殺人");
    }

    #[test]
    fn open_project_reports_not_found_for_a_folder_without_a_manifest() {
        let dir = TempDir::new().unwrap();

        let error = open_project(dir.path()).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }
}
