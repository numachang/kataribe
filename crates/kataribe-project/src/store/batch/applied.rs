//! 反映済みの変更と、途中で失敗したときの巻き戻し。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::prepare::{PlannedMove, PlannedWrite};
use super::staging::Staging;
use crate::error::{ProjectError, StillMoved, StillTrashed};
use crate::path::RelPath;
use crate::store::{atomic_write, is_transient_lock_error, rename_with_retry};

/// 反映済みの変更。途中で失敗したとき、逆順に元へ戻すために覚えておく。
pub(super) enum Applied<'a> {
    Replaced(&'a PlannedWrite<'a>),
    Trashed(TrashMove<'a>),
    /// 改名の 1 段階目。移動元を置き場へ預けた。
    MovedAway(&'a MoveStep<'a>),
    /// 改名の 2 段階目。預けたものを移動先へ置いた。
    MovedIn(&'a MoveStep<'a>),
}

/// ゴミ箱へ移したファイルまたはフォルダ 1 つ。
pub(super) struct TrashMove<'a> {
    pub(super) path: &'a RelPath,
    pub(super) original: &'a Path,
    /// ゴミ箱の中の置き場所（元へ戻せなかったとき、利用者へ知らせる）。
    pub(super) trashed: RelPath,
    pub(super) resolved_trashed: PathBuf,
    /// 今回のゴミ箱フォルダ（`.kataribe/trash/<日時>/`）。戻したあとに空になったフォルダを片付ける範囲の上限。
    pub(super) batch_dir: PathBuf,
}

/// 改名 1 つ分。移動元を一度置き場（`.kataribe/staging/<日時>/m<n>`）へ預けてから、移動先へ置く。
///
/// 2 段階にするのは、入れ替えのような循環や、番号をずらす連続した改名でも、移動先がまだ埋まっている
/// ことにならないようにするため（全部を預けてから、全部を置く）。
pub(super) struct MoveStep<'a> {
    /// 改名の通し番号。
    pub(super) index: usize,
    pub(super) from: &'a RelPath,
    pub(super) to: &'a RelPath,
    from_resolved: &'a Path,
    to_resolved: &'a Path,
    /// 預け先（元へ戻せなかったとき、利用者へ知らせる）。
    pub(super) staged: RelPath,
    staged_resolved: PathBuf,
}

impl<'a> MoveStep<'a> {
    pub(super) fn plan(
        index: usize,
        planned: &'a PlannedMove<'a>,
        staging: &Staging,
    ) -> Result<Self, ProjectError> {
        let (staged, staged_resolved) = staging.slot(index)?;
        Ok(Self {
            index,
            from: planned.from,
            to: planned.to,
            from_resolved: &planned.from_resolved,
            to_resolved: &planned.to_resolved,
            staged,
            staged_resolved,
        })
    }

    /// 1 段階目: 移動元を置き場へ預ける。
    pub(super) fn move_away(&self) -> Result<(), ProjectError> {
        rename_into_free_place(self.from_resolved, &self.staged_resolved)
            .map_err(|source| rename_error(self.from, source))
    }

    /// 2 段階目: 預けたものを移動先へ置く。
    pub(super) fn move_in(&self) -> Result<(), ProjectError> {
        crate::store::create_parent_dir(self.to_resolved, self.to)?;
        rename_into_free_place(&self.staged_resolved, self.to_resolved)
            .map_err(|source| rename_error(self.to, source))
    }

    /// `move_away` を取り消す（預けたものを元の場所へ戻す）。
    ///
    /// 元の場所が埋まっていれば戻さない（そこにあるのは、取り消せなかった別の改名の実物かもしれない）。
    fn undo_move_away(&self) -> std::io::Result<()> {
        rename_into_free_place(&self.staged_resolved, self.from_resolved)
    }

    /// `move_in` を取り消す（置いたものを預け先へ戻す）。
    fn undo_move_in(&self) -> std::io::Result<()> {
        rename_into_free_place(self.to_resolved, &self.staged_resolved)
    }
}

/// `to` に何も無いことを確かめてから、`from` を `to` へ改名する。
///
/// Windows の改名は、行き先がファイルだと黙って置き換える。そこにある別のファイル（外で作られたもの、
/// 取り消せなかった別の改名の実物）の中身を消さないための確認。確認と改名の間に外で作られる場合までは
/// 防げない（std には置き換えない改名が無い）が、同じ変更案の中で起こる取り違えは防げる。
fn rename_into_free_place(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(to) {
        Ok(_) => return Err(std::io::ErrorKind::AlreadyExists.into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    rename_with_retry(from, to)
}

/// 改名の失敗を、利用者へ知らせる形にする。
///
/// 行き先が埋まっていたのは外で変更されたということ、ロックで失敗したのはほかのアプリがファイルを
/// 開いているということ。後者は、閉じれば再試行できるので、権限のエラーとは別に案内する。
pub(super) fn rename_error(path: &RelPath, source: std::io::Error) -> ProjectError {
    if source.kind() == std::io::ErrorKind::AlreadyExists {
        ProjectError::Conflict { path: path.clone() }
    } else if is_transient_lock_error(&source) {
        ProjectError::FilesInUse {
            path: path.clone(),
            restored: false,
        }
    } else {
        ProjectError::Io {
            path: path.clone(),
            source,
        }
    }
}

impl PlannedWrite<'_> {
    /// 置き換える前の状態に戻す。
    fn restore(&self) -> Result<(), ProjectError> {
        match &self.previous {
            Some(previous) => atomic_write(&self.resolved, self.path, &previous.bytes),
            None => fs::remove_file(&self.resolved).map_err(|source| ProjectError::Io {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

impl TrashMove<'_> {
    /// ゴミ箱から元の場所へ戻す。元の場所が埋まっていれば戻さない（実物はゴミ箱に残る）。
    fn restore(&self) -> Result<(), ProjectError> {
        rename_into_free_place(&self.resolved_trashed, self.original)
            .map_err(|source| rename_error(self.path, source))?;
        remove_empty_dirs(self.resolved_trashed.parent(), &self.batch_dir);
        Ok(())
    }
}

impl Applied<'_> {
    fn undo(&self) -> Result<(), ProjectError> {
        match self {
            Self::Replaced(plan) => plan.restore(),
            Self::Trashed(moved) => moved.restore(),
            Self::MovedAway(step) => step.undo_move_away().map_err(|source| ProjectError::Io {
                path: step.from.clone(),
                source,
            }),
            Self::MovedIn(step) => step.undo_move_in().map_err(|source| ProjectError::Io {
                path: step.to.clone(),
                source,
            }),
        }
    }
}

/// `start` から `top`（含む）まで、空になったフォルダを内側から消す。
///
/// 戻したあとに、ゴミ箱の中へ空のフォルダが残らないようにするため。空でないフォルダは消せないので、
/// そこで止める（中身を消してしまうことはない）。
pub(super) fn remove_empty_dirs(start: Option<&Path>, top: &Path) {
    let mut current = start;
    while let Some(dir) = current {
        if fs::remove_dir(dir).is_err() || dir == top {
            return;
        }
        current = dir.parent();
    }
}

/// 反映の途中で失敗したとき、反映済みの変更を逆順に元へ戻し、返すエラーを決める。
///
/// `journal` は、反映の前に書いた操作の記録（あれば）。戻せなかったものがあるとき、利用者へ知らせる。
pub(super) fn roll_back(
    applied: &[Applied<'_>],
    journal: Option<&RelPath>,
    error: ProjectError,
) -> ProjectError {
    let mut not_restored = Vec::new();
    let mut still_trashed = Vec::new();
    let mut still_moved = Vec::new();
    // 2 段階目を戻せなかった改名は、実物が移動先にある。1 段階目の取り消しは、預け先に何も無いので試さない
    let mut stranded_moves = HashSet::new();
    for step in applied.iter().rev() {
        if let Applied::MovedAway(moved) = step
            && stranded_moves.contains(&moved.index)
        {
            continue;
        }
        if step.undo().is_ok() {
            continue;
        }
        match step {
            Applied::Replaced(plan) => not_restored.push(plan.path.clone()),
            Applied::Trashed(moved) => still_trashed.push(StillTrashed {
                original: moved.path.clone(),
                trashed: moved.trashed.clone(),
            }),
            Applied::MovedAway(moved) => still_moved.push(StillMoved {
                original: moved.from.clone(),
                current: moved.staged.clone(),
            }),
            Applied::MovedIn(moved) => {
                stranded_moves.insert(moved.index);
                still_moved.push(StillMoved {
                    original: moved.from.clone(),
                    current: moved.to.clone(),
                });
            }
        }
    }
    if not_restored.is_empty() && still_trashed.is_empty() && still_moved.is_empty() {
        declare_restored(error)
    } else {
        ProjectError::PartialWrite {
            not_restored,
            still_trashed,
            still_moved,
            journal: journal.cloned(),
            source: Box::new(error),
        }
    }
}

/// すべて元に戻せたことを、ロックの失敗の案内に反映する（戻せなかったものがあるときは、別に案内する）。
fn declare_restored(error: ProjectError) -> ProjectError {
    match error {
        ProjectError::FilesInUse { path, .. } => ProjectError::FilesInUse {
            path,
            restored: true,
        },
        other => other,
    }
}
