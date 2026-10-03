//! 変更案。利用者が確認してから作品フォルダに適用する。

use kataribe_project::{
    BackupMode, ContentHash, PendingChange, PendingWrite, Project, ProjectError, RelPath, TextFile,
    WriteCondition,
};
use kataribe_text::count::count_chars;
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};

/// 作品フォルダのファイルへの、1 つの変更。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileChange {
    /// ファイルの新規作成、または上書き。
    Write {
        /// 書き込み先。
        path: RelPath,
        /// 書き込む内容。
        content: String,
        /// 変更前の内容。新規ファイルなら `None`。
        previous: Option<String>,
        /// 変更前の内容のハッシュ。適用時に、その後の編集と競合していないか確かめる。
        base_hash: Option<ContentHash>,
    },
    /// ファイルをゴミ箱（`.kataribe/trash/`）へ移す。今はファイルだけを移せる（フォルダは扱わない）。
    Trash {
        /// 移すもの。
        path: RelPath,
        /// 移すファイルの一覧。今は `path` のファイル 1 つだけ。
        files: Vec<TrashedFile>,
    },
}

/// ゴミ箱へ移すファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TrashedFile {
    /// 作品フォルダからの相対パス。
    pub path: RelPath,
    /// 移す前の内容のハッシュ。適用時に、その後の編集と競合していないか確かめる。
    /// テキストとして読めなかったファイルは `None`（適用できない）。
    pub base_hash: Option<ContentHash>,
    /// 内容の文字数（ルビの読み・空白を除く）。何が失われるかを利用者に見せるため。
    pub chars: usize,
}

impl FileChange {
    /// 変更の対象のパス。
    #[must_use]
    pub fn path(&self) -> &RelPath {
        match self {
            Self::Write { path, .. } | Self::Trash { path, .. } => path,
        }
    }

    /// 作品フォルダへの反映に使う形にする。
    fn to_pending(&self) -> Result<PendingChange<'_>> {
        match self {
            Self::Write {
                path,
                content,
                base_hash,
                ..
            } => Ok(PendingChange::Write(PendingWrite {
                path,
                content,
                condition: write_condition(base_hash.as_ref()),
            })),
            Self::Trash { path, files } => Ok(PendingChange::Trash {
                path,
                expected: sole_file_hash(path, files)?,
            }),
        }
    }
}

/// 生成したときから内容が変わっていない場合だけ書き込めるようにする条件。
fn write_condition(base_hash: Option<&ContentHash>) -> WriteCondition {
    match base_hash {
        Some(hash) => WriteCondition::Matches(hash.clone()),
        None => WriteCondition::Absent,
    }
}

/// `path` のファイル 1 つをゴミ箱へ移す変更の、競合の確認に使うハッシュ。
/// 形が違えば（ファイルが複数・別のパス・ハッシュが無い）変更案の誤りにする。
fn sole_file_hash(path: &RelPath, files: &[TrashedFile]) -> Result<ContentHash> {
    match files {
        [
            TrashedFile {
                path: file_path,
                base_hash: Some(hash),
                ..
            },
        ] if file_path == path => Ok(hash.clone()),
        _ => Err(ProjectError::InvalidChangeSet {
            reason: format!("{path} をゴミ箱へ移す内容が、そのファイル 1 つの形になっていません。"),
        }
        .into()),
    }
}

/// 作品フォルダへの変更案。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ChangeSet {
    pub summary: String,
    /// ファイルへの変更。適用の順は並び順に頼らず、ゴミ箱へ移す → 書く。
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
    /// 同じパスへの書き込みがすでにあれば、内容だけを差し替える。
    pub fn put(&mut self, path: RelPath, content: String, base: Option<TextFile>) {
        let existing = self.files.iter_mut().find_map(|change| match change {
            FileChange::Write {
                path: existing_path,
                content: existing_content,
                ..
            } if *existing_path == path => Some(existing_content),
            _ => None,
        });
        if let Some(existing_content) = existing {
            *existing_content = content;
            return;
        }
        self.files.push(FileChange::Write {
            path,
            content,
            previous: base.as_ref().map(|file| file.content.clone()),
            base_hash: base.map(|file| file.hash),
        });
    }

    /// `path` のファイル（`file` は、移す前に読んだ内容）をゴミ箱へ移す変更を加える。
    ///
    /// 適用時には、そのあと利用者が同じファイルを編集していないかを `file` のハッシュで確かめる。
    pub fn trash_file(&mut self, path: RelPath, file: &TextFile) {
        let trashed = TrashedFile {
            path: path.clone(),
            base_hash: Some(file.hash.clone()),
            chars: count_chars(&file.content),
        };
        self.files.push(FileChange::Trash {
            path,
            files: vec![trashed],
        });
    }

    /// `path` に書き込む予定の内容。書き込みの変更が無ければ `None`。
    #[must_use]
    pub fn written(&self, path: &RelPath) -> Option<&str> {
        self.files.iter().find_map(|change| match change {
            FileChange::Write {
                path: written_path,
                content,
                ..
            } if written_path == path => Some(content.as_str()),
            _ => None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// 変更案を作品フォルダに適用する。
    ///
    /// 別の作品の変更案なら何も適用しない。すべての変更を反映するか、何も反映しないかのどちらかで、
    /// 一つでも競合していれば何も変えない。反映の順は、並び順ではなく「ゴミ箱へ移す → 書く」。
    /// 同じパスへの変更が重なっている・ゴミ箱へ移せないパスを含む変更案は、画面から戻ってくる値なので
    /// 作品フォルダ側で検証して断る（[`ProjectError::InvalidChangeSet`]）。
    /// LLM による置き換えなので、上書きするファイルは必ずバックアップする。
    pub fn apply(&self, project: &Project) -> Result<()> {
        if self.project_root != project_identity(project) {
            return Err(EngineError::InvalidInput(format!(
                "この変更案は別の作品（{}）のものなので、今開いている作品には適用できません。",
                self.project_root
            )));
        }
        let changes = self
            .files
            .iter()
            .map(FileChange::to_pending)
            .collect::<Result<Vec<_>>>()?;
        project
            .store()
            .apply_changes(&changes, BackupMode::Always)?;
        Ok(())
    }
}

fn project_identity(project: &Project) -> String {
    project.store().canonical_root().display().to_string()
}
