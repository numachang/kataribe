//! 複数の変更（ファイルの書き込み・ゴミ箱へ移す・改名・状態の確認）を、すべて反映するか、何も反映しないかのどちらかで行う。
//!
//! 流れは「形の検証（[`shape`]）→ 条件の確認（[`prepare`]）→ 反映（[`commit`]）」。形の検証と条件の確認で
//! 失敗しても、作品のファイルはまだ何も変わっていない。反映の途中で失敗したら、反映済みの変更を逆順に元へ戻す
//! （[`applied`]）。反映の途中の置き場と操作の記録は [`staging`]・[`journal`]。

mod applied;
mod commit;
mod journal;
mod namespace;
mod prepare;
mod shape;
mod staging;
#[cfg(test)]
mod tests;

use super::{BackupMode, ContentHash, ProjectStore, WriteCondition};
use crate::error::ProjectError;
use crate::path::RelPath;
use namespace::Namespace;
use prepare::{Batch, PlannedTrash};
use std::path::PathBuf;

/// [`PendingChange::Write`] で書くファイルの 1 つ。
#[derive(Debug, Clone)]
pub struct PendingWrite<'a> {
    /// 書き込み先。
    pub path: &'a RelPath,
    /// 書き込む内容。
    pub content: &'a str,
    /// 書き込みを許す条件。
    pub condition: WriteCondition,
}

/// ゴミ箱へ移すものの、移してよい今の状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryCondition {
    /// ファイル。今のハッシュがこれと一致すること。
    File(ContentHash),
    /// フォルダ。中のファイル全部（サブフォルダの下も含む）のパスとハッシュの一覧が、これと完全に一致すること。
    /// 増えていても、変わっていても競合にする（利用者が確かめた中身だけを移すため）。
    Folder(Vec<(RelPath, ContentHash)>),
}

/// [`ProjectStore::apply_changes`] でまとめて反映する変更の 1 つ。
#[derive(Debug, Clone)]
pub enum PendingChange<'a> {
    /// ファイルの新規作成または上書き。条件は、改名を反映したあとの状態に対して確かめる。
    Write(PendingWrite<'a>),
    /// ファイルまたはフォルダをゴミ箱へ移す。
    Trash {
        /// 移すもの。
        path: &'a RelPath,
        /// 移してよい今の状態。違っていれば（読んだあとに外で変わった）競合にする。
        expected: EntryCondition,
    },
    /// ファイルまたはフォルダの改名。中身は変えない（中身のハッシュは見ない）。
    Move {
        /// 移動元。
        from: &'a RelPath,
        /// 移動先。
        to: &'a RelPath,
    },
    /// 何も書かず、今の状態を確かめるだけ。計画のあとに外で状態が変わったら競合にするために使う。
    Expect {
        /// 確かめるパス。
        path: &'a RelPath,
        /// そのファイルの今のハッシュ。`None` なら「何も無いこと」。
        expected: Option<ContentHash>,
    },
}

impl ProjectStore {
    /// 複数の変更を、すべて反映するか、何も反映しないかのどちらかで行う。
    ///
    /// 反映する順は、並び順ではなく「状態の確認（Expect）→ ゴミ箱へ移す → 改名 → 書く」。先に次を確かめ、
    /// 一つでも合わなければ何も変えない。
    /// - 形の誤り（[`ProjectError::InvalidChangeSet`]）: ゴミ箱へ移したり改名したりできるのは、`.kataribe/` の外で
    ///   `kataribe.yaml` 以外。同じパス（大文字小文字の違いは同じとみなす）への変更は重ねられない
    ///   （フォルダとその中のパスの重なりも同じ。改名の元と先がつながる場合などは例外で、`shape` の表を参照）。
    ///   フォルダを自分の中へ移したり、ゴミ箱へ移すフォルダの下に書いたりもできない。
    /// - 条件（[`ProjectError::Conflict`]）。Expect は今の状態が一致すること。ゴミ箱へ移すのは、ファイルなら
    ///   今のハッシュ、フォルダなら中のファイルの一覧（パスとハッシュ）が一致すること。改名は、移動元があり、移動先が
    ///   同じ変更で空く場所を除いて空いていること（中身は見ない）。書き込みは `condition`。ただし書き込みは改名した
    ///   あとの状態に対して確かめる（書き先が改名の行き先なら移動元の今の中身、改名で空く場所なら「無いこと」）。
    ///
    /// ゴミ箱へ移したものは、この呼び出しごとに 1 つ作る `.kataribe/trash/<日時>/` の下に、元の相対パスのまま
    /// 置く（同じ日時のフォルダが既にあれば `-1`, `-2`, … を付ける）。書き込む内容の一時ファイルと、改名の途中で
    /// 預けるものは `.kataribe/staging/<日時>/` に置く。改名は 2 段階（まず移動元をすべて預け、次にすべて行き先へ置く）
    /// なので、入れ替えのような循環も扱える。ゴミ箱や改名を伴うときは、動かす前に操作の記録を
    /// `.kataribe/staging/<日時>/journal.json` に書く。
    ///
    /// 反映の途中で失敗したら、反映済みの変更を逆順に元へ戻す。戻せなかったものがあれば
    /// [`ProjectError::PartialWrite`]（書き込み・ゴミ箱へ移したもの・改名したものを分けて持ち、実物の場所を知らせる。
    /// このときだけ、置き場と操作の記録を消さずに残す）。
    ///
    /// 戻り値は、`changes` に現れる書き込みごとの、書き込み後の内容のハッシュ（書き込みの並び順）。
    pub fn apply_changes(
        &self,
        changes: &[PendingChange<'_>],
        backup: BackupMode,
    ) -> Result<Vec<ContentHash>, ProjectError> {
        shape::validate(changes)?;
        let _guard = self.lock_writes();
        let batch = self.prepare(changes)?;
        self.commit(&batch, backup)?;
        Ok(batch.writes.iter().map(|plan| plan.hash.clone()).collect())
    }

    /// 1 ファイルの書き込み。条件に合わなければ何も書かずに `Conflict` を返す。
    pub(super) fn write_one(
        &self,
        write: &PendingWrite<'_>,
        backup: BackupMode,
    ) -> Result<ContentHash, ProjectError> {
        let _guard = self.lock_writes();
        let plan = self.plan_write(write, &Namespace::default())?;
        let hash = plan.hash.clone();
        let batch = Batch {
            writes: vec![plan],
            ..Batch::default()
        };
        self.commit(&batch, backup)?;
        Ok(hash)
    }

    /// 1 つのファイル（またはフォルダ）を、内容を確かめずにゴミ箱へ移す。呼び出し側が書き込みの排他を取っていること。
    pub(super) fn trash_only(&self, path: &RelPath, resolved: PathBuf) -> Result<(), ProjectError> {
        let batch = Batch {
            trashes: vec![PlannedTrash { path, resolved }],
            ..Batch::default()
        };
        self.commit(&batch, BackupMode::Never)
    }
}
