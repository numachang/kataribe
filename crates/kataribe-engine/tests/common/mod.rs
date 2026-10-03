//! 統合テストで共有する、作品の準備の補助。
#![allow(dead_code, clippy::unwrap_used)]

use std::path::Path;

use kataribe_engine::{NewProject, create_project};
use kataribe_project::{BackupMode, Project, Rating, RelPath, WriteCondition, WriteOptions};

/// 空の作品を作る。
pub fn new_project(folder: &Path) -> Project {
    create_project(
        folder,
        NewProject {
            title: "みさき館の殺人".into(),
            author: None,
            genre: "mystery".into(),
            genre_note: None,
            rating: Rating::General,
            target_length: 6000,
            idea: "嵐で孤立した洋館で、盲目の少女探偵が密室殺人を解く。".into(),
        },
    )
    .unwrap()
}

/// 作品にファイルを置く（テストの準備。壊れた YAML もそのまま置ける）。
pub fn put(project: &Project, path: &str, content: &str) {
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
        .unwrap();
}

/// 作品のファイルの内容。無ければ `None`。
pub fn read(project: &Project, path: &str) -> Option<String> {
    project
        .store()
        .read_text_opt(&RelPath::new(path).unwrap())
        .unwrap()
        .map(|file| file.content)
}
