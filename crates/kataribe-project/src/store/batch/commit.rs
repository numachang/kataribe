//! 準備した変更の反映。
//!
//! 反映の順は「一時ファイルへの書き込み → バックアップ → 操作の記録 → ゴミ箱へ移す → 改名（2 段階）→
//! 一時ファイルでの置き換え」。ゴミ箱へ移す前に失敗しても、作品のファイルはまだ何も変わっていない。
//! それ以降に失敗したら、反映済みの変更を逆順に元へ戻す（[`roll_back`]）。

use std::path::Path;

use tempfile::NamedTempFile;

use super::applied::{Applied, MoveStep, TrashMove, remove_empty_dirs, roll_back};
use super::journal::Journal;
use super::prepare::{Batch, PlannedTrash, PlannedWrite};
use super::staging::Staging;
use crate::error::ProjectError;
use crate::layout;
use crate::path::RelPath;
use crate::store::{
    BackupMode, ProjectStore, create_parent_dir, persist_temp_file, rename_with_retry,
    stage_temp_file,
};

impl ProjectStore {
    /// 準備した変更を反映する。内容の変わらない書き込みは何もしない。
    ///
    /// 戻せなかったもの（[`ProjectError::PartialWrite`]）があるときは、実物の場所と操作の記録を
    /// 人が見て戻せるよう、置き場（`.kataribe/staging/<日時>/`）を残す。それ以外は片付ける。
    pub(super) fn commit(&self, batch: &Batch<'_>, backup: BackupMode) -> Result<(), ProjectError> {
        let changed: Vec<&PlannedWrite<'_>> = batch
            .writes
            .iter()
            .filter(|plan| plan.changes_file())
            .collect();
        if changed.is_empty() && batch.trashes.is_empty() && batch.moves.is_empty() {
            return Ok(());
        }
        let staging = self.create_staging_dir()?;
        let result = self.commit_in(batch, &changed, &staging, backup);
        if !matches!(result, Err(ProjectError::PartialWrite { .. })) {
            staging.discard();
        }
        result
    }

    fn commit_in<'a>(
        &self,
        batch: &'a Batch<'a>,
        changed: &[&'a PlannedWrite<'a>],
        staging: &Staging,
        backup: BackupMode,
    ) -> Result<(), ProjectError> {
        let temp_files = changed
            .iter()
            .map(|plan| stage_temp_file(staging.resolved(), plan.path, plan.content.as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;
        for previous in changed.iter().filter_map(|plan| plan.previous.as_ref()) {
            self.maybe_backup(&previous.source, backup)?;
        }
        let trash_dir = if batch.trashes.is_empty() {
            None
        } else {
            Some(self.new_batch_dir(layout::TRASH_DIR)?)
        };
        let steps = batch
            .moves
            .iter()
            .enumerate()
            .map(|(index, planned)| MoveStep::plan(index, planned, staging))
            .collect::<Result<Vec<_>, _>>()?;
        let journal = match Journal::describe(batch, changed, trash_dir.as_ref(), &steps) {
            Some(journal) => Some(journal.write(staging)?),
            None => None,
        };

        let mut applied = Vec::new();
        self.apply_steps(
            batch,
            trash_dir.as_ref(),
            &steps,
            changed,
            temp_files,
            &mut applied,
        )
        .map_err(|error| roll_back(&applied, journal.as_ref(), error))
    }

    /// ゴミ箱へ移す → 改名（預ける → 置く）→ 置き換え、の順に行う。済んだものを `applied` に記録する。
    fn apply_steps<'a>(
        &self,
        batch: &'a Batch<'a>,
        trash_dir: Option<&RelPath>,
        steps: &'a [MoveStep<'a>],
        changed: &[&'a PlannedWrite<'a>],
        temp_files: Vec<NamedTempFile>,
        applied: &mut Vec<Applied<'a>>,
    ) -> Result<(), ProjectError> {
        if let Some(trash_dir) = trash_dir {
            self.move_to_trash(&batch.trashes, trash_dir, applied)?;
        }
        for step in steps {
            step.move_away()?;
            applied.push(Applied::MovedAway(step));
        }
        for step in steps {
            step.move_in()?;
            applied.push(Applied::MovedIn(step));
        }
        for (plan, temp_file) in changed.iter().copied().zip(temp_files) {
            create_parent_dir(&plan.resolved, plan.path)?;
            persist_temp_file(temp_file, &plan.resolved, plan.path)?;
            applied.push(Applied::Replaced(plan));
        }
        Ok(())
    }

    /// `trashes` を、この呼び出し用の 1 つのゴミ箱フォルダ `batch_dir` へ、元の相対パスのまま移す。
    /// 移し終えたものから `applied` に記録する（途中で失敗したときに元へ戻せるように）。
    pub(super) fn move_to_trash<'a>(
        &self,
        trashes: &'a [PlannedTrash<'a>],
        batch_dir: &RelPath,
        applied: &mut Vec<Applied<'a>>,
    ) -> Result<(), ProjectError> {
        let resolved_batch_dir = self.resolve(batch_dir)?;
        for trash in trashes {
            let moved = self.move_into_trash(trash, batch_dir, &resolved_batch_dir)?;
            applied.push(Applied::Trashed(moved));
        }
        Ok(())
    }

    /// 1 つのファイルまたはフォルダを今回のゴミ箱フォルダへ移す。失敗したら、そのために作った空のフォルダを片付ける
    /// （移せたものが 1 つも無いまま、`.kataribe/trash/<日時>/` の空のフォルダが残らないように）。
    fn move_into_trash<'a>(
        &self,
        trash: &'a PlannedTrash<'a>,
        batch_dir: &RelPath,
        resolved_batch_dir: &Path,
    ) -> Result<TrashMove<'a>, ProjectError> {
        let destination = RelPath::new(&format!("{batch_dir}/{}", trash.path))?;
        let resolved_destination = self.resolve(&destination)?;
        let moved = create_parent_dir(&resolved_destination, &destination).and_then(|()| {
            rename_with_retry(&trash.resolved, &resolved_destination).map_err(|source| {
                ProjectError::Io {
                    path: trash.path.clone(),
                    source,
                }
            })
        });
        if let Err(error) = moved {
            remove_empty_dirs(resolved_destination.parent(), resolved_batch_dir);
            return Err(error);
        }
        Ok(TrashMove {
            path: trash.path,
            original: &trash.resolved,
            trashed: destination,
            resolved_trashed: resolved_destination,
            batch_dir: resolved_batch_dir.to_path_buf(),
        })
    }
}
