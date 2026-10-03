//! 反映の操作の記録（`journal.json`）。
//!
//! 何かを動かす前に、これから行う操作を書いておく。途中でプロセスが落ちたときに、人がこれを見て、
//! どれを元の場所へ戻せばよいかが分かるようにするため（自動では使わない）。
//! 移動やゴミ箱を伴う反映だけが書く。正常に終われば（または、すべて元に戻れば）消える。

use std::fs;
use std::io::Write as _;

use serde::Serialize;

use super::applied::MoveStep;
use super::prepare::{Batch, PlannedWrite};
use super::staging::Staging;
use crate::error::ProjectError;
use crate::path::RelPath;

/// これから行う操作の記録。
#[derive(Debug, Serialize)]
pub(super) struct Journal<'a> {
    /// 記録の読み方。
    note: &'static str,
    /// ゴミ箱へ移すものを入れるフォルダ。`trashed` の各パスは、この下に元の相対パスのまま入る。
    trash_dir: Option<&'a RelPath>,
    /// ゴミ箱へ移すもの。
    trashed: Vec<&'a RelPath>,
    /// 改名するもの。まず `from` を `staged` へ移し、次に `staged` を `to` へ移す。
    moves: Vec<JournalMove<'a>>,
    /// 書き込むファイル。
    written: Vec<&'a RelPath>,
}

#[derive(Debug, Serialize)]
struct JournalMove<'a> {
    from: &'a RelPath,
    to: &'a RelPath,
    staged: &'a RelPath,
}

impl<'a> Journal<'a> {
    /// 反映する内容から記録を作る。ゴミ箱も改名も無ければ（書き込みだけなら）記録は要らないので `None`。
    pub(super) fn describe(
        batch: &'a Batch<'a>,
        written: &[&'a PlannedWrite<'a>],
        trash_dir: Option<&'a RelPath>,
        steps: &'a [MoveStep<'a>],
    ) -> Option<Self> {
        if batch.trashes.is_empty() && steps.is_empty() {
            return None;
        }
        Some(Self {
            note: "この反映が途中で止まったときの記録です。trashed は trash_dir の下へ、\
                   moves は staged を経由して from から to へ移す途中でした。\
                   戻せなかったものは、この記録を見て手で元の場所へ戻してください。",
            trash_dir,
            trashed: batch.trashes.iter().map(|trash| trash.path).collect(),
            moves: steps
                .iter()
                .map(|step| JournalMove {
                    from: step.from,
                    to: step.to,
                    staged: &step.staged,
                })
                .collect(),
            written: written.iter().map(|plan| plan.path).collect(),
        })
    }

    /// 置き場に書き出す。書き出した記録のパスを返す。
    pub(super) fn write(&self, staging: &Staging) -> Result<RelPath, ProjectError> {
        let path = staging.journal_path()?;
        let io_error = |source| ProjectError::Io {
            path: path.clone(),
            source,
        };
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| io_error(std::io::Error::other(error)))?;
        let mut file = fs::File::create(staging.journal_resolved()).map_err(io_error)?;
        file.write_all(json.as_bytes()).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        Ok(path)
    }
}
