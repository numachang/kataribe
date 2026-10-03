//! フォルダの中身（ファイルの一覧と内容のハッシュ）を読む。
//!
//! フォルダごとゴミ箱へ移す前に、利用者が確かめた中身と今の中身が同じかを照らすために使う。

use std::fs;

use super::{ProjectStore, TextFile, decode_text};
use crate::error::ProjectError;
use crate::path::RelPath;

/// フォルダの中のファイル 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderFile {
    /// 作品フォルダからの相対パス。
    pub path: RelPath,
    /// 内容。UTF-8 のテキストとして読めなければ `None`。
    pub text: Option<TextFile>,
}

impl ProjectStore {
    /// フォルダの中のファイルを、サブフォルダの下も含め、パスの順で全部読む。
    ///
    /// `.` で始まる名前のファイルも含める（フォルダごと移すと一緒に動くので、利用者が確かめた中身に入れるため）。
    /// フォルダが無い、またはフォルダではないなら `Ok(None)`。名前が UTF-8 や作品フォルダのパスの規則に合わない
    /// ファイル、シンボリックリンクがあれば、中身を数え漏らさないようエラーにする。
    pub fn read_folder(&self, path: &RelPath) -> Result<Option<Vec<FolderFile>>, ProjectError> {
        if !self.resolve(path)?.is_dir() {
            return Ok(None);
        }
        let mut files = Vec::new();
        let mut pending_dirs = vec![path.clone()];
        while let Some(dir) = pending_dirs.pop() {
            self.collect_folder_entries(&dir, &mut pending_dirs, &mut files)?;
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Some(files))
    }

    /// `dir` の直下を調べ、サブフォルダは `pending_dirs` に、ファイルは `files` に足す。
    fn collect_folder_entries(
        &self,
        dir: &RelPath,
        pending_dirs: &mut Vec<RelPath>,
        files: &mut Vec<FolderFile>,
    ) -> Result<(), ProjectError> {
        let io_error = |source| ProjectError::Io {
            path: dir.clone(),
            source,
        };
        for entry in fs::read_dir(self.resolve(dir)?).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let Ok(name) = entry.file_name().into_string() else {
                return Err(io_error(std::io::Error::other(
                    "UTF-8 として読めない名前のファイルがあります",
                )));
            };
            let child = dir.join(&name)?;
            let file_type = entry.file_type().map_err(io_error)?;
            if file_type.is_symlink() {
                return Err(ProjectError::Io {
                    path: child,
                    source: std::io::Error::other("シンボリックリンクは扱えません"),
                });
            }
            if file_type.is_dir() {
                pending_dirs.push(child);
                continue;
            }
            let bytes = fs::read(self.resolve(&child)?).map_err(|source| ProjectError::Io {
                path: child.clone(),
                source,
            })?;
            let text = decode_text(&child, &bytes).ok();
            files.push(FolderFile { path: child, text });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::super::test_support::{rel, write};
    use super::*;

    fn paths(files: &[FolderFile]) -> Vec<&str> {
        files.iter().map(|file| file.path.as_str()).collect()
    }

    #[test]
    fn read_folder_lists_every_file_below_the_folder_in_path_order_with_hidden_ones() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let hash = write(&store, "manuscript/03/s02.txt", "二つ目");
        write(&store, "manuscript/03/s01.txt", "一つ目");
        write(&store, "manuscript/03/notes/memo.txt", "メモ");
        write(&store, "manuscript/03/.hidden", "隠し");
        write(&store, "manuscript/04/s01.txt", "別の章");

        let files = store.read_folder(&rel("manuscript/03")).unwrap().unwrap();

        assert_eq!(
            paths(&files),
            [
                "manuscript/03/.hidden",
                "manuscript/03/notes/memo.txt",
                "manuscript/03/s01.txt",
                "manuscript/03/s02.txt",
            ]
        );
        assert_eq!(files[3].text.as_ref().unwrap().hash, hash);
    }

    #[test]
    fn read_folder_is_none_for_a_missing_folder_and_for_a_file() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        write(&store, "concept.md", "企画");

        assert_eq!(store.read_folder(&rel("manuscript/03")).unwrap(), None);
        assert_eq!(store.read_folder(&rel("concept.md")).unwrap(), None);
    }

    #[test]
    fn read_folder_is_empty_for_a_folder_without_files() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        fs::create_dir_all(dir.path().join("manuscript/03/empty")).unwrap();

        let files = store.read_folder(&rel("manuscript/03")).unwrap().unwrap();

        assert_eq!(files, Vec::new());
    }

    #[test]
    fn a_file_that_is_not_utf8_text_is_listed_without_its_text() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        fs::create_dir_all(dir.path().join("manuscript/03")).unwrap();
        fs::write(
            dir.path().join("manuscript/03/cover.bin"),
            [0xff, 0xfe, 0x00],
        )
        .unwrap();

        let files = store.read_folder(&rel("manuscript/03")).unwrap().unwrap();

        assert_eq!(paths(&files), ["manuscript/03/cover.bin"]);
        assert_eq!(files[0].text, None);
    }
}
