use std::collections::BTreeMap;

use std::path::Path;

use kataribe_project::{FORMAT_VERSION, Manifest, Project, Rating};
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};
use crate::stages::materials::non_empty;

const MIN_TARGET_LENGTH: u32 = 1000;

/// 新しい作品を作るときに利用者が入力する項目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct NewProject {
    pub title: String,
    pub author: Option<String>,
    pub genre: String,
    pub genre_note: Option<String>,
    pub rating: Rating,
    pub target_length: u32,
    pub idea: String,
}

/// `folder`（空のフォルダ、または存在しないフォルダ）に新しい作品を作る。
pub fn create_project(folder: &Path, new_project: NewProject) -> Result<Project> {
    let title = new_project.title.trim();
    if title.is_empty() {
        return Err(EngineError::InvalidInput("題名を入力してください。".into()));
    }
    if new_project.target_length < MIN_TARGET_LENGTH {
        return Err(EngineError::InvalidInput(format!(
            "目標文字数は {MIN_TARGET_LENGTH} 字以上にしてください。"
        )));
    }
    let manifest = Manifest {
        format: FORMAT_VERSION,
        title: title.to_owned(),
        author: new_project.author.as_deref().and_then(non_empty),
        genre: new_project.genre,
        genre_note: new_project.genre_note.as_deref().and_then(non_empty),
        rating: new_project.rating,
        target_length: new_project.target_length,
        idea: new_project.idea.trim().to_owned(),
        settings: None,
        extra: BTreeMap::default(),
    };
    Ok(Project::create(folder, &manifest)?)
}
