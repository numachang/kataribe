//! 変更案。利用者が確認してから作品フォルダに適用する。

use kataribe_project::{
    BackupMode, ContentHash, EntryCondition, FolderFile, PendingChange, PendingWrite, Project,
    ProjectError, RelPath, TextFile, WriteCondition,
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
    /// ファイルまたはフォルダをゴミ箱（`.kataribe/trash/`）へ移す。
    Trash {
        /// 移すもの。
        path: RelPath,
        /// 移すファイルの一覧。ファイルを移すときは `path` のファイル 1 つだけ。フォルダを移すときは、
        /// フォルダの中のファイル全部（サブフォルダの下も含む。空のフォルダなら空）。適用するときは、
        /// 今の中身がこの一覧と完全に一致しなければ競合にする（利用者が確かめた中身だけを移すため）。
        files: Vec<TrashedFile>,
    },
    /// ファイルまたはフォルダの改名。中身は変えない（章の番号の振り直し）。
    /// 中身のハッシュは条件にしない（移動では中身が失われず、関係のない自動保存のたびに競合になるため）。
    Move {
        /// 移動元。
        from: RelPath,
        /// 移動先。同じ変更案のゴミ箱や移動で空く場所を除いて、空いていなければならない。
        to: RelPath,
    },
    /// 何も書かず、適用するときにこのパスがこの状態であることだけを確かめる。
    /// 計画のあとに外で状態が変わったら競合にするために使う。
    Expect {
        /// 確かめるパス。
        path: RelPath,
        /// そのファイルの今のハッシュ。`None` なら「何も無いこと」。
        base_hash: Option<ContentHash>,
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
    /// 変更の対象のパス。改名は移動元。
    #[must_use]
    pub fn path(&self) -> &RelPath {
        match self {
            Self::Write { path, .. } | Self::Trash { path, .. } | Self::Expect { path, .. } => path,
            Self::Move { from, .. } => from,
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
                expected: trash_condition(path, files)?,
            }),
            Self::Move { from, to } => Ok(PendingChange::Move { from, to }),
            Self::Expect { path, base_hash } => Ok(PendingChange::Expect {
                path,
                expected: base_hash.clone(),
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

/// ゴミ箱へ移す変更が、適用のときに確かめる今の状態。
///
/// `path` のファイル 1 つだけの一覧ならファイル、そうでなければフォルダ（中のファイル全部の一覧）。
/// 形が違えば（一覧のファイルが `path` の中に無い・ハッシュが無い＝テキストとして読めない）変更案の誤りにする。
fn trash_condition(path: &RelPath, files: &[TrashedFile]) -> Result<EntryCondition> {
    if let [only] = files
        && only.path == *path
    {
        let hash = readable_hash(path, only)?;
        return Ok(EntryCondition::File(hash));
    }
    let folder_prefix = format!("{path}/");
    let mut listed = Vec::with_capacity(files.len());
    for file in files {
        if !file.path.as_str().starts_with(&folder_prefix) {
            return Err(invalid_change_set(format!(
                "{path} をゴミ箱へ移す内容に、{path} の中にないファイル（{}）が含まれています。",
                file.path
            )));
        }
        listed.push((file.path.clone(), readable_hash(path, file)?));
    }
    Ok(EntryCondition::Folder(listed))
}

fn readable_hash(trashed: &RelPath, file: &TrashedFile) -> Result<ContentHash> {
    file.base_hash.clone().ok_or_else(|| {
        invalid_change_set(format!(
            "{trashed} をゴミ箱へ移す内容に、テキストとして読めないファイル（{}）があるため、適用できません。",
            file.path
        ))
    })
}

fn invalid_change_set(reason: String) -> EngineError {
    ProjectError::InvalidChangeSet { reason }.into()
}

/// 作品フォルダへの変更案。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ChangeSet {
    pub summary: String,
    /// ファイルへの変更。適用の順は並び順に頼らず、状態の確認（Expect）→ ゴミ箱へ移す → 改名 → 書く。
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
        let trashed = trashed_file(path.clone(), Some(file));
        self.files.push(FileChange::Trash {
            path,
            files: vec![trashed],
        });
    }

    /// フォルダ `path` を、中のファイル全部ごとゴミ箱へ移す変更を加える。
    ///
    /// `files` は、移す前に読んだ中身（[`kataribe_project::ProjectStore::read_folder`]）。適用時には、
    /// そのあとにファイルが増えたり変わったりしていないかを、一覧のハッシュで確かめる。
    pub fn trash_folder(&mut self, path: RelPath, files: &[FolderFile]) {
        let trashed = files
            .iter()
            .map(|file| trashed_file(file.path.clone(), file.text.as_ref()))
            .collect();
        self.files.push(FileChange::Trash {
            path,
            files: trashed,
        });
    }

    /// ファイルまたはフォルダ `from` を `to` へ改名する変更を加える。
    pub fn move_entry(&mut self, from: RelPath, to: RelPath) {
        self.files.push(FileChange::Move { from, to });
    }

    /// 適用するときに、`path` が今この状態であることを確かめる変更を加える。何も書かない。
    ///
    /// `file` は読んだときのファイル（`None` なら「無いこと」。フォルダが無いことの確認にも使える）。
    pub fn expect(&mut self, path: RelPath, file: Option<&TextFile>) {
        self.files.push(FileChange::Expect {
            path,
            base_hash: file.map(|file| file.hash.clone()),
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
    /// 一つでも競合していれば何も変えない。反映の順は、並び順ではなく
    /// 「状態の確認（Expect）→ ゴミ箱へ移す → 改名（Move）→ 書く」。それぞれ次の条件を確かめる。
    ///
    /// - Expect: 今の状態が `base_hash` と一致する（`None` なら無い）。
    /// - Trash（ファイル）: 今のハッシュが `base_hash` と一致する。
    /// - Trash（フォルダ）: 中のファイルの一覧（パスとハッシュ）が計画のときと完全に一致する。増えても変わっても競合。
    /// - Move: 移動元があり、移動先が（同じ変更案のゴミ箱や改名で空く場所を除いて）空いている。中身は見ない。
    /// - Write: 条件は改名した後の状態に対して確かめる。書き先が改名の行き先なら移動元の今の中身の `base_hash`、
    ///   改名で空く場所なら「無いこと」として扱う（「03 を 04 へ移し、空いた 03 に新しい章を書く」を表すため）。
    ///
    /// 同じパスへの変更が重なっている・ゴミ箱へ移したり改名したりできないパス（`.kataribe/` と `kataribe.yaml`）
    /// を含む・フォルダを自分の中へ移す・ゴミ箱へ移すフォルダの下へ書く変更案は、画面から戻ってくる値なので
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

/// ゴミ箱へ移すファイル 1 つの記録。`file` が `None`（テキストとして読めない）ならハッシュは無い。
fn trashed_file(path: RelPath, file: Option<&TextFile>) -> TrashedFile {
    TrashedFile {
        path,
        base_hash: file.map(|file| file.hash.clone()),
        chars: file.map_or(0, |file| count_chars(&file.content)),
    }
}
