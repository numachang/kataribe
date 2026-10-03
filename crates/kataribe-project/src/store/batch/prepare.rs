//! 反映の準備。条件を確かめ、反映する内容をそろえる。ここまでは作品のファイルを何も変えない。
//!
//! 条件を確かめる順は、反映の順と同じ「状態の確認（Expect）→ ゴミ箱 → 移動 → 書き込み」。
//! どれも、確かめるときのファイルの状態（変更を反映する前）に対して行うが、書き込みだけは
//! 移動した後の状態に対して確かめる（[`Namespace`]）。

use std::path::PathBuf;

use super::namespace::Namespace;
use super::{EntryCondition, PendingChange, PendingWrite};
use crate::error::ProjectError;
use crate::path::RelPath;
use crate::store::{
    ContentHash, ProjectStore, TextFile, WriteCondition, check_condition_against, decode_text,
    normalize_text,
};

/// 準備のできた、反映する変更。
#[derive(Default)]
pub(super) struct Batch<'a> {
    /// 書き込み。`changes` に現れる順。
    pub(super) writes: Vec<PlannedWrite<'a>>,
    pub(super) trashes: Vec<PlannedTrash<'a>>,
    pub(super) moves: Vec<PlannedMove<'a>>,
}

/// 書き込みの準備ができた 1 ファイル分。
pub(super) struct PlannedWrite<'a> {
    pub(super) path: &'a RelPath,
    pub(super) resolved: PathBuf,
    /// 書き込む前のファイル。新規（や、移動で空く場所）なら `None`（途中で失敗したときに元へ戻すのに使う）。
    pub(super) previous: Option<PreviousFile>,
    /// 正規化した、書き込む内容。
    pub(super) content: String,
    pub(super) hash: ContentHash,
}

/// 書き込む前のファイル。
pub(super) struct PreviousFile {
    /// 今あるパス。書き込み先が移動の行き先なら、移動元のパス（バックアップはこの名前で残る）。
    pub(super) source: RelPath,
    pub(super) text: TextFile,
    /// 元のバイト列（BOM や CRLF も含めて、そのまま元に戻すため）。
    pub(super) bytes: Vec<u8>,
}

impl PlannedWrite<'_> {
    pub(super) fn changes_file(&self) -> bool {
        self.previous
            .as_ref()
            .is_none_or(|previous| previous.text.content != self.content)
    }
}

/// ゴミ箱へ移す準備ができたファイルまたはフォルダ。
pub(super) struct PlannedTrash<'a> {
    pub(super) path: &'a RelPath,
    pub(super) resolved: PathBuf,
}

/// 改名（移動）する準備ができたファイルまたはフォルダ。
pub(super) struct PlannedMove<'a> {
    pub(super) from: &'a RelPath,
    pub(super) to: &'a RelPath,
    pub(super) from_resolved: PathBuf,
    pub(super) to_resolved: PathBuf,
}

impl ProjectStore {
    /// 条件を確かめ、反映する内容をそろえる。
    pub(super) fn prepare<'a>(
        &self,
        changes: &[PendingChange<'a>],
    ) -> Result<Batch<'a>, ProjectError> {
        let namespace = Namespace::new(changes);
        for change in changes {
            if let PendingChange::Expect { path, expected } = change {
                self.check_expectation(path, expected.as_ref())?;
            }
        }
        let mut batch = Batch::default();
        for change in changes {
            if let PendingChange::Trash { path, expected } = change {
                batch.trashes.push(self.plan_trash(path, expected)?);
            }
        }
        for change in changes {
            if let PendingChange::Move { from, to } = change {
                batch.moves.push(self.plan_move(from, to, &namespace)?);
            }
        }
        for change in changes {
            if let PendingChange::Write(write) = change {
                batch.writes.push(self.plan_write(write, &namespace)?);
            }
        }
        Ok(batch)
    }

    /// 状態の確認。`expected` が `None` なら「無いこと」、あればそのハッシュのファイルであること。
    fn check_expectation(
        &self,
        path: &RelPath,
        expected: Option<&ContentHash>,
    ) -> Result<(), ProjectError> {
        let Some(expected) = expected else {
            return if self.resolve(path)?.exists() {
                Err(ProjectError::Conflict { path: path.clone() })
            } else {
                Ok(())
            };
        };
        let current = self.read_file_opt(path)?;
        check_condition_against(
            current.as_ref(),
            path,
            &WriteCondition::Matches(expected.clone()),
        )
    }

    /// 1 つのファイルまたはフォルダのゴミ箱への移動の準備。
    /// 利用者が確かめた中身と今の中身が違えば（無ければ）`Conflict`。
    pub(super) fn plan_trash<'a>(
        &self,
        path: &'a RelPath,
        expected: &EntryCondition,
    ) -> Result<PlannedTrash<'a>, ProjectError> {
        match expected {
            EntryCondition::File(hash) => {
                let current = self.read_file_opt(path)?;
                let condition = WriteCondition::Matches(hash.clone());
                check_condition_against(current.as_ref(), path, &condition)?;
            }
            EntryCondition::Folder(files) => self.check_folder_contents(path, files)?,
        }
        Ok(PlannedTrash {
            path,
            resolved: self.resolve(path)?,
        })
    }

    /// フォルダの中のファイルの一覧（パスとハッシュ）が `expected` と完全に一致すること。
    /// 増えていても、変わっていても、読めないファイルがあっても競合にする（確かめた中身だけを移すため）。
    fn check_folder_contents(
        &self,
        path: &RelPath,
        expected: &[(RelPath, ContentHash)],
    ) -> Result<(), ProjectError> {
        let current = self
            .read_folder(path)?
            .ok_or_else(|| ProjectError::Conflict { path: path.clone() })?;
        let mut expected: Vec<&(RelPath, ContentHash)> = expected.iter().collect();
        expected.sort();
        let matches = current.len() == expected.len()
            && current.iter().zip(expected).all(|(file, (path, hash))| {
                file.path == *path && file.text.as_ref().is_some_and(|text| text.hash == *hash)
            });
        if matches {
            Ok(())
        } else {
            Err(ProjectError::Conflict { path: path.clone() })
        }
    }

    /// 1 つのファイルまたはフォルダの改名の準備。移動元があり、移動先が（同じ変更案で空く場所を除いて）
    /// 空いていなければ `Conflict`。中身は見ない（移動では中身が失われないため）。
    fn plan_move<'a>(
        &self,
        from: &'a RelPath,
        to: &'a RelPath,
        namespace: &Namespace,
    ) -> Result<PlannedMove<'a>, ProjectError> {
        let from_resolved = self.resolve(from)?;
        if !from_resolved.exists() {
            return Err(ProjectError::Conflict { path: from.clone() });
        }
        let to_resolved = self.resolve(to)?;
        if to_resolved.exists() && !namespace.is_vacated(to) {
            return Err(ProjectError::Conflict { path: to.clone() });
        }
        Ok(PlannedMove {
            from,
            to,
            from_resolved,
            to_resolved,
        })
    }

    /// 1 ファイル分の書き込みの準備。条件は、移動した後の状態に対して確かめ、合わなければ `Conflict`。
    pub(super) fn plan_write<'a>(
        &self,
        write: &PendingWrite<'a>,
        namespace: &Namespace,
    ) -> Result<PlannedWrite<'a>, ProjectError> {
        let previous = match namespace.source_of(write.path)? {
            Some(source) => self.read_previous(source)?,
            None => None,
        };
        let current = previous.as_ref().map(|file| &file.text);
        check_condition_against(current, write.path, &write.condition)?;
        let content = normalize_text(write.content);
        Ok(PlannedWrite {
            path: write.path,
            resolved: self.resolve(write.path)?,
            hash: ContentHash::of(content.as_bytes()),
            content,
            previous,
        })
    }

    /// 書き込みで置き換わるファイルを読む。無ければ `None`。
    fn read_previous(&self, source: RelPath) -> Result<Option<PreviousFile>, ProjectError> {
        let Some(bytes) = self.read_bytes_opt(&source)? else {
            return Ok(None);
        };
        let text = decode_text(&source, &bytes)?;
        Ok(Some(PreviousFile {
            source,
            text,
            bytes,
        }))
    }

    /// ファイルを読む。無いときと、フォルダのときは `None`（ファイルとしては「無い」）。
    fn read_file_opt(&self, path: &RelPath) -> Result<Option<TextFile>, ProjectError> {
        if self.resolve(path)?.is_dir() {
            return Ok(None);
        }
        self.read_text_opt(path)
    }
}
