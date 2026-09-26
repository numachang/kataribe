//! 生成結果の変更案。利用者が確認してから作品フォルダに適用する。

use kataribe_project::{
    BackupMode, ContentHash, Project, RelPath, TextFile, WriteCondition, WriteOptions,
};
use serde::{Deserialize, Serialize};

use crate::error::Result;

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
}

impl ChangeSet {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            files: Vec::new(),
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
    /// 先にすべてのファイルの競合を確かめ、一つでも競合していれば何も書かない。
    /// LLM による置き換えなので、上書きするファイルは必ずバックアップする。
    pub fn apply(&self, project: &Project) -> Result<()> {
        let store = project.store();
        for change in &self.files {
            store.check_condition(&change.path, &change.write_condition())?;
        }
        for change in &self.files {
            store.write_text(
                &change.path,
                &change.content,
                WriteOptions {
                    condition: change.write_condition(),
                    backup: BackupMode::Always,
                },
            )?;
        }
        Ok(())
    }
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
