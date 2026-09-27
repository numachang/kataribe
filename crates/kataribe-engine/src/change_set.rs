//! 生成結果の変更案。利用者が確認してから作品フォルダに適用する。

use kataribe_project::{
    BackupMode, ContentHash, PendingWrite, Project, RelPath, TextFile, WriteCondition,
};
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FileChange {
    pub path: RelPath,
    pub content: String,
    /// 変更前の内容。新規ファイルなら `None`。
    pub previous: Option<String>,
    /// 変更前の内容のハッシュ。適用時に、その後の編集と競合していないか確かめる。
    pub base_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ChangeSet {
    pub summary: String,
    pub files: Vec<FileChange>,
    /// この変更案を作った作品フォルダ（正規化した絶対パス）。生成中に別の作品へ開き直したとき、
    /// 前の作品の変更案を書き込まないよう、適用時に照合する。
    pub project_root: String,
}

impl ChangeSet {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            files: Vec::new(),
            project_root: String::new(),
        }
    }

    /// この変更案を `project` のものとして印を付ける。
    #[must_use]
    pub fn made_for(self, project: &Project) -> Self {
        Self {
            project_root: project_identity(project),
            ..self
        }
    }

    /// `path` を `content` に置き換える変更を加える。
    ///
    /// `base` は、この変更の元にしたファイル（生成を始める前に読んだもの。新規なら `None`）。
    /// 適用時には、そのあと利用者が同じファイルを編集していないかを `base` のハッシュで確かめる。
    /// 同じパスへの変更がすでにあれば、内容だけを差し替える。
    pub fn put(&mut self, path: RelPath, content: String, base: Option<TextFile>) {
        if let Some(existing) = self.files.iter_mut().find(|change| change.path == path) {
            existing.content = content;
            return;
        }
        self.files.push(FileChange {
            path,
            content,
            previous: base.as_ref().map(|file| file.content.clone()),
            base_hash: base.map(|file| file.hash),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// 変更案を作品フォルダに書き込む。
    ///
    /// 別の作品の変更案なら何も書かない。すべてのファイルを書くか、何も書かないかのどちらかで、
    /// 一つでも競合していれば何も書かない。LLM による置き換えなので、上書きするファイルは必ずバックアップする。
    pub fn apply(&self, project: &Project) -> Result<()> {
        if self.project_root != project_identity(project) {
            return Err(EngineError::InvalidInput(format!(
                "この変更案は別の作品（{}）のものなので、今開いている作品には適用できません。",
                self.project_root
            )));
        }
        let writes: Vec<PendingWrite<'_>> = self
            .files
            .iter()
            .map(|change| PendingWrite {
                path: &change.path,
                content: &change.content,
                condition: change.write_condition(),
            })
            .collect();
        project.store().write_all(&writes, BackupMode::Always)?;
        Ok(())
    }
}

fn project_identity(project: &Project) -> String {
    project.store().canonical_root().display().to_string()
}

impl FileChange {
    /// 生成したときから内容が変わっていない場合だけ書き込めるようにする条件。
    fn write_condition(&self) -> WriteCondition {
        match &self.base_hash {
            Some(hash) => WriteCondition::Matches(hash.clone()),
            None => WriteCondition::Absent,
        }
    }
}
