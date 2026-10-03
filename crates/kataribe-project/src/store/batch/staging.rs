//! 反映の途中の置き場（`.kataribe/staging/<日時>/`）。
//!
//! 書き込む内容の一時ファイルと、改名の途中で一時的に預けるファイル・フォルダを置く。置き換え先と同じフォルダに
//! 一時ファイルを置かないのは、置き換え先のフォルダ自体が改名で動くことがあるため。作品フォルダの中（同じボリューム）
//! なので、ここから置き場への移動は改名で済む。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ProjectError;
use crate::layout;
use crate::path::RelPath;
use crate::store::{ProjectStore, create_parent_dir, timestamp_stamp};

/// 操作の記録のファイル名。
const JOURNAL_FILE_NAME: &str = "journal.json";

/// 1 回の反映のための、作ったばかりの置き場。
#[derive(Debug)]
pub(super) struct Staging {
    dir: RelPath,
    resolved: PathBuf,
}

impl Staging {
    pub(super) fn resolved(&self) -> &Path {
        &self.resolved
    }

    /// `index` 番目の改名で、移動元を一時的に預ける場所。
    pub(super) fn slot(&self, index: usize) -> Result<(RelPath, PathBuf), ProjectError> {
        let name = format!("m{index}");
        Ok((self.dir.join(&name)?, self.resolved.join(name)))
    }

    /// 操作の記録の場所。
    pub(super) fn journal_path(&self) -> Result<RelPath, ProjectError> {
        Ok(self.dir.join(JOURNAL_FILE_NAME)?)
    }

    pub(super) fn journal_resolved(&self) -> PathBuf {
        self.resolved.join(JOURNAL_FILE_NAME)
    }

    /// 反映が済んだ（または、すべて元に戻った）あとの片付け。失敗しても無視する。
    ///
    /// 操作の記録を消し、空のフォルダだけを消す。中に何かが残っていれば（戻せなかったものがあれば）、
    /// 実物を消さないよう、そのまま残す。
    pub(super) fn discard(&self) {
        let _ = fs::remove_file(self.journal_resolved());
        let _ = fs::remove_dir(&self.resolved);
    }
}

impl ProjectStore {
    /// 1 回の反映のための置き場を作る。
    pub(super) fn create_staging_dir(&self) -> Result<Staging, ProjectError> {
        let dir = self.create_batch_dir(layout::STAGING_DIR)?;
        let resolved = self.resolve(&dir)?;
        Ok(Staging { dir, resolved })
    }

    /// `<parent_dir>/<日時>/` を新しく作り、そのパスを返す。同じ日時のフォルダが既にあれば `-1`, `-2`, … を付ける
    /// （同じミリ秒に 2 回反映しても、前のものを上書きしないため）。
    ///
    /// 「無いことを確かめてから作る」のではなく、作成そのものの成否で確保する。GUI と CLI が同じミリ秒に
    /// 反映しても、互いに別のフォルダになる（同じ置き場を共有して、操作の記録や預け先を上書きしない）。
    pub(super) fn create_batch_dir(&self, parent_dir: &str) -> Result<RelPath, ProjectError> {
        let stamp = timestamp_stamp(self.clock.now());
        let mut collision_counter = 0u32;
        loop {
            let name = if collision_counter == 0 {
                stamp.clone()
            } else {
                format!("{stamp}-{collision_counter}")
            };
            let candidate = RelPath::new(&format!("{parent_dir}/{name}"))?;
            let resolved = self.resolve(&candidate)?;
            create_parent_dir(&resolved, &candidate)?;
            match fs::create_dir(&resolved) {
                Ok(()) => return Ok(candidate),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    collision_counter += 1;
                }
                Err(source) => {
                    return Err(ProjectError::Io {
                        path: candidate,
                        source,
                    });
                }
            }
        }
    }
}
