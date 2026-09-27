//! テストだけで使う補助。複数のサブコマンドの単体テストで重複していた作品作成をまとめる。

use std::path::Path;

use kataribe_engine::{NewProject, create_project};
use kataribe_project::{Project, Rating};

/// テスト用の作品を作る。題名・ジャンル・企画・目標文字数は、複数のテストで共通に使う値に固定する。
pub(crate) fn new_test_project(folder: &Path) -> Project {
    create_project(
        folder,
        NewProject {
            title: "みさき館の殺人".into(),
            author: None,
            genre: "mystery".into(),
            genre_note: None,
            rating: Rating::General,
            target_length: 6000,
            idea: "嵐で孤立した洋館で起きる密室殺人。".into(),
        },
    )
    .unwrap()
}
