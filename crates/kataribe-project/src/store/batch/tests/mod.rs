//! `apply_changes` のテスト（書き込み・ゴミ箱・戻し）。改名・フォルダのゴミ箱・状態の確認は `moves`、
//! 失敗したときの巻き戻しは `rollback`。

mod moves;
mod rollback;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use jiff::Timestamp;
use tempfile::TempDir;

use super::super::test_support::{FixedClock, read, rel, write};
use super::super::{
    Backup, BackupPolicy, ProjectStoreOptions, TextFile, WriteCondition, WriteOptions,
};
use super::applied::{Applied, TrashMove, roll_back};
use super::prepare::{PlannedTrash, PlannedWrite, PreviousFile};
use super::*;
use crate::error::StillTrashed;

fn open_store(dir: &TempDir) -> ProjectStore {
    ProjectStore::open(dir.path()).unwrap()
}

fn open_store_with_fixed_clock(dir: &TempDir) -> ProjectStore {
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_700_000_000).unwrap(),
    ));
    ProjectStore::open_with(
        dir.path(),
        ProjectStoreOptions {
            backup_policy: BackupPolicy::default(),
            clock,
        },
    )
    .unwrap()
}

fn write_change<'a>(
    path: &'a RelPath,
    content: &'a str,
    condition: WriteCondition,
) -> PendingChange<'a> {
    PendingChange::Write(PendingWrite {
        path,
        content,
        condition,
    })
}

fn trash_change(path: &RelPath, expected: ContentHash) -> PendingChange<'_> {
    PendingChange::Trash {
        path,
        expected: EntryCondition::File(expected),
    }
}

#[test]
fn apply_changes_writes_every_file_and_returns_hashes_in_order() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let old_hash = write(&store, "plot/chapters/01.md", "旧");
    let (first, second) = (rel("plot/chapters/01.md"), rel("plot/chapters/02.md"));

    let hashes = store
        .apply_changes(
            &[
                write_change(&first, "新しい 1 章", WriteCondition::Matches(old_hash)),
                write_change(&second, "新しい 2 章", WriteCondition::Absent),
            ],
            BackupMode::Always,
        )
        .unwrap();

    assert_eq!(read(&store, "plot/chapters/01.md").unwrap(), "新しい 1 章");
    assert_eq!(read(&store, "plot/chapters/02.md").unwrap(), "新しい 2 章");
    assert_eq!(hashes[0], store.read_text(&first).unwrap().hash);
    assert_eq!(hashes[1], store.read_text(&second).unwrap().hash);
    assert_eq!(store.backups(&first).unwrap().len(), 1);
}

#[test]
fn apply_changes_writes_nothing_when_any_file_conflicts() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "concept.md", "企画");
    write(&store, "style.md", "文体");
    let (concept, style) = (rel("concept.md"), rel("style.md"));

    let result = store.apply_changes(
        &[
            write_change(&concept, "新しい企画", WriteCondition::Any),
            write_change(&style, "新しい文体", WriteCondition::Absent),
        ],
        BackupMode::Always,
    );

    assert!(matches!(result, Err(ProjectError::Conflict { path }) if path == style));
    assert_eq!(read(&store, "concept.md").unwrap(), "企画");
    assert_eq!(read(&store, "style.md").unwrap(), "文体");
    assert_eq!(store.backups(&concept).unwrap(), Vec::<Backup>::new());
}

#[test]
fn apply_changes_skips_files_whose_content_is_unchanged() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "concept.md", "同じ内容");
    let concept = rel("concept.md");

    let hashes = store
        .apply_changes(
            &[write_change(
                &concept,
                "同じ内容",
                WriteCondition::Matches(hash.clone()),
            )],
            BackupMode::Always,
        )
        .unwrap();

    assert_eq!(hashes, vec![hash]);
    assert_eq!(store.backups(&concept).unwrap(), Vec::<Backup>::new());
}

#[test]
fn trash_moves_the_file_when_its_hash_matches() {
    let dir = TempDir::new().unwrap();
    let store = open_store_with_fixed_clock(&dir);
    let hash = write(&store, "characters/rin.md", "霧島 凛");
    let path = rel("characters/rin.md");

    let hashes = store
        .apply_changes(&[trash_change(&path, hash)], BackupMode::Always)
        .unwrap();

    assert_eq!(hashes, Vec::<ContentHash>::new());
    assert!(!store.exists(&path));
    let trashed = store
        .read_text(&rel(
            ".kataribe/trash/20231114-221320-000/characters/rin.md",
        ))
        .unwrap();
    assert_eq!(trashed.content, "霧島 凛");
}

#[test]
fn trash_conflicts_and_changes_nothing_when_the_file_changed_after_it_was_read() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let stale_hash = write(&store, "characters/rin.md", "読んだときの内容");
    write(
        &store,
        "characters/rin.md",
        "そのあと外で書き換えられた内容",
    );
    write(&store, "concept.md", "企画");
    let (character, concept) = (rel("characters/rin.md"), rel("concept.md"));

    let result = store.apply_changes(
        &[
            write_change(&concept, "新しい企画", WriteCondition::Any),
            trash_change(&character, stale_hash),
        ],
        BackupMode::Always,
    );

    assert!(matches!(result, Err(ProjectError::Conflict { path }) if path == character));
    assert_eq!(
        read(&store, "characters/rin.md").unwrap(),
        "そのあと外で書き換えられた内容"
    );
    assert_eq!(read(&store, "concept.md").unwrap(), "企画");
    assert!(!store.exists(&rel(".kataribe/trash")));
}

#[test]
fn trash_conflicts_when_the_file_is_already_gone() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "characters/rin.md", "霧島 凛");
    let path = rel("characters/rin.md");
    store.remove(&path).unwrap();

    let result = store.apply_changes(&[trash_change(&path, hash)], BackupMode::Always);

    assert!(matches!(result, Err(ProjectError::Conflict { .. })));
}

#[test]
fn one_call_puts_all_trashed_files_in_one_folder_keeping_their_relative_paths() {
    let dir = TempDir::new().unwrap();
    let store = open_store_with_fixed_clock(&dir);
    let scene_hash = write(&store, "manuscript/01/s02.txt", "本文");
    let world_hash = write(&store, "world/glossary.md", "用語集");
    let (scene, world) = (rel("manuscript/01/s02.txt"), rel("world/glossary.md"));

    store
        .apply_changes(
            &[
                trash_change(&scene, scene_hash),
                trash_change(&world, world_hash),
            ],
            BackupMode::Always,
        )
        .unwrap();

    let batch = ".kataribe/trash/20231114-221320-000";
    assert_eq!(
        read(&store, &format!("{batch}/manuscript/01/s02.txt")).unwrap(),
        "本文"
    );
    assert_eq!(
        read(&store, &format!("{batch}/world/glossary.md")).unwrap(),
        "用語集"
    );
    assert_eq!(
        fs::read_dir(dir.path().join(".kataribe/trash"))
            .unwrap()
            .count(),
        1,
        "ゴミ箱のフォルダは 1 回の適用につき 1 つだけのはず"
    );
}

#[test]
fn trash_folders_made_in_the_same_millisecond_do_not_overwrite_each_other() {
    let dir = TempDir::new().unwrap();
    let store = open_store_with_fixed_clock(&dir);
    let path = rel("characters/rin.md");

    for content in ["一回目の内容", "二回目の内容"] {
        let hash = write(&store, "characters/rin.md", content);
        store
            .apply_changes(&[trash_change(&path, hash)], BackupMode::Always)
            .unwrap();
    }

    let first = ".kataribe/trash/20231114-221320-000/characters/rin.md";
    let second = ".kataribe/trash/20231114-221320-000-1/characters/rin.md";
    assert_eq!(read(&store, first).unwrap(), "一回目の内容");
    assert_eq!(read(&store, second).unwrap(), "二回目の内容");
}

#[test]
fn a_batch_dir_that_is_already_taken_is_never_shared() {
    let dir = TempDir::new().unwrap();
    let store = open_store_with_fixed_clock(&dir);

    let first = store.create_batch_dir(".kataribe/staging").unwrap();
    fs::write(
        dir.path().join(first.as_str()).join("journal.json"),
        "先の記録",
    )
    .unwrap();
    let second = store.create_batch_dir(".kataribe/staging").unwrap();
    let third = store.create_batch_dir(".kataribe/staging").unwrap();

    assert_eq!(first.as_str(), ".kataribe/staging/20231114-221320-000");
    assert_eq!(second.as_str(), ".kataribe/staging/20231114-221320-000-1");
    assert_eq!(third.as_str(), ".kataribe/staging/20231114-221320-000-2");
    assert_eq!(
        read(&store, ".kataribe/staging/20231114-221320-000/journal.json").unwrap(),
        "先の記録",
        "先に確保した置き場の中身は、あとから確保した置き場に触られない"
    );
}

#[test]
fn a_batch_dir_name_taken_by_a_file_is_skipped() {
    let dir = TempDir::new().unwrap();
    let store = open_store_with_fixed_clock(&dir);
    fs::create_dir_all(dir.path().join(".kataribe/staging")).unwrap();
    fs::write(
        dir.path().join(".kataribe/staging/20231114-221320-000"),
        "ファイル",
    )
    .unwrap();

    let created = store.create_batch_dir(".kataribe/staging").unwrap();

    assert_eq!(created.as_str(), ".kataribe/staging/20231114-221320-000-1");
    assert!(dir.path().join(created.as_str()).is_dir());
}

#[test]
fn trash_and_write_in_one_call_both_take_effect() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let chapter_hash = write(&store, "plot/chapters/01.md", "シーンが二つの章");
    let scene_hash = write(&store, "manuscript/01/s02.txt", "二つ目の本文");
    let (chapter, scene) = (rel("plot/chapters/01.md"), rel("manuscript/01/s02.txt"));

    store
        .apply_changes(
            &[
                write_change(
                    &chapter,
                    "シーンが一つの章",
                    WriteCondition::Matches(chapter_hash),
                ),
                trash_change(&scene, scene_hash),
            ],
            BackupMode::Always,
        )
        .unwrap();

    assert_eq!(
        read(&store, "plot/chapters/01.md").unwrap(),
        "シーンが一つの章"
    );
    assert!(!store.exists(&scene));
}

#[test]
fn a_conflicting_write_stops_the_trash_in_the_same_call() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let scene_hash = write(&store, "manuscript/01/s02.txt", "二つ目の本文");
    write(&store, "plot/chapters/01.md", "外で変わった章");
    let (chapter, scene) = (rel("plot/chapters/01.md"), rel("manuscript/01/s02.txt"));

    let result = store.apply_changes(
        &[
            trash_change(&scene, scene_hash),
            write_change(&chapter, "書こうとした章", WriteCondition::Absent),
        ],
        BackupMode::Always,
    );

    assert!(matches!(result, Err(ProjectError::Conflict { path }) if path == chapter));
    assert_eq!(
        read(&store, "manuscript/01/s02.txt").unwrap(),
        "二つ目の本文"
    );
    assert!(!store.exists(&rel(".kataribe/trash")));
}

#[test]
fn trash_is_refused_for_internal_data_and_the_manifest() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "kataribe.yaml", "format: 1");
    write(&store, ".kataribe/cache/summary.json", "{}");

    for path in [
        "kataribe.yaml",
        "Kataribe.yaml",
        ".kataribe/cache/summary.json",
        ".Kataribe/cache/summary.json",
        ".kataribe",
    ] {
        let path = rel(path);
        let result = store.apply_changes(&[trash_change(&path, hash.clone())], BackupMode::Always);
        assert!(
            matches!(result, Err(ProjectError::InvalidChangeSet { .. })),
            "{path} はゴミ箱へ移せないはず: {result:?}"
        );
    }
    assert_eq!(read(&store, "kataribe.yaml").unwrap(), "format: 1");
    assert!(store.exists(&rel(".kataribe/cache/summary.json")));
}

#[test]
fn two_changes_to_the_same_path_are_refused_before_anything_is_changed() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "characters/rin.md", "霧島 凛");
    write(&store, "concept.md", "企画");
    let character = rel("characters/rin.md");
    let concept = rel("concept.md");
    let same_file_in_other_case = rel("Characters/Rin.md");

    for overlapping in [
        vec![
            trash_change(&character, hash.clone()),
            write_change(&character, "書き直し", WriteCondition::Any),
        ],
        vec![
            write_change(&concept, "一つ目", WriteCondition::Any),
            write_change(&concept, "二つ目", WriteCondition::Any),
        ],
        vec![
            trash_change(&character, hash.clone()),
            write_change(&same_file_in_other_case, "書き直し", WriteCondition::Any),
        ],
    ] {
        let result = store.apply_changes(&overlapping, BackupMode::Always);
        assert!(
            matches!(result, Err(ProjectError::InvalidChangeSet { .. })),
            "重なった変更は形の誤りになるはず: {result:?}"
        );
    }
    assert_eq!(read(&store, "characters/rin.md").unwrap(), "霧島 凛");
    assert_eq!(read(&store, "concept.md").unwrap(), "企画");
}

/// 置き換えの途中でロックに当たって失敗したら、置き換え済みのファイルを元に戻す。
/// 2 つ目のファイルを「削除・改名を許さない」共有モードで開いておき、実際に置き換えを失敗させる。
#[cfg(windows)]
#[test]
fn apply_changes_restores_replaced_files_when_a_later_replacement_fails() {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_SHARE_READ: u32 = 0x1;

    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "manuscript/01/s02.txt", "元の二");
    // メモ帳などで保存された、BOM と CRLF のあるファイル
    let first_bytes = "\u{feff}元の一\r\n二行目\r\n".as_bytes();
    fs::write(dir.path().join("manuscript/01/s01.txt"), first_bytes).unwrap();
    let (first, second, added) = (
        rel("manuscript/01/s01.txt"),
        rel("manuscript/01/s02.txt"),
        rel("manuscript/01/s00.txt"),
    );
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(dir.path().join("manuscript/01/s02.txt"))
        .unwrap();

    let result = store.apply_changes(
        &[
            write_change(&added, "新しいシーン", WriteCondition::Absent),
            write_change(&first, "新しい一", WriteCondition::Any),
            write_change(&second, "新しい二", WriteCondition::Any),
        ],
        BackupMode::Never,
    );

    assert!(matches!(result, Err(ProjectError::Io { path, .. }) if path == second));
    assert_eq!(
        fs::read(dir.path().join("manuscript/01/s01.txt")).unwrap(),
        first_bytes
    );
    assert_eq!(read(&store, "manuscript/01/s02.txt").unwrap(), "元の二");
    assert_eq!(read(&store, "manuscript/01/s00.txt"), None);
    let leftovers: Vec<_> = fs::read_dir(dir.path().join("manuscript/01"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        leftovers.len(),
        2,
        "一時ファイルが残っている: {leftovers:?}"
    );
}

/// ゴミ箱へ移す途中で、ほかのアプリが開いているファイルに当たったら、移し終えたものを戻して失敗する。
#[cfg(windows)]
#[test]
fn trash_moves_are_undone_when_a_later_file_is_locked_by_another_app() {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_SHARE_READ: u32 = 0x1;

    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let first_hash = write(&store, "manuscript/01/s01.txt", "一つ目の本文");
    let second_hash = write(&store, "manuscript/01/s02.txt", "二つ目の本文");
    let (first, second) = (rel("manuscript/01/s01.txt"), rel("manuscript/01/s02.txt"));
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(dir.path().join("manuscript/01/s02.txt"))
        .unwrap();

    let result = store.apply_changes(
        &[
            trash_change(&first, first_hash),
            trash_change(&second, second_hash),
        ],
        BackupMode::Always,
    );

    assert!(
        matches!(result, Err(ProjectError::FilesInUse { path, restored: true }) if path == second)
    );
    assert_eq!(
        read(&store, "manuscript/01/s01.txt").unwrap(),
        "一つ目の本文"
    );
    assert_eq!(
        read(&store, "manuscript/01/s02.txt").unwrap(),
        "二つ目の本文"
    );
    assert_eq!(
        fs::read_dir(dir.path().join(".kataribe/trash"))
            .unwrap()
            .count(),
        0,
        "戻したあとに、空のゴミ箱フォルダが残らないはず"
    );
}

/// ゴミ箱へ移したあとの置き換えで失敗したら、ゴミ箱へ移したファイルも元の場所へ戻す。
#[cfg(windows)]
#[test]
fn trashed_files_are_put_back_when_a_later_replacement_fails() {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_SHARE_READ: u32 = 0x1;

    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let scene_hash = write(&store, "manuscript/01/s02.txt", "二つ目の本文");
    write(&store, "plot/chapters/01.md", "元の章立て");
    let (scene, chapter) = (rel("manuscript/01/s02.txt"), rel("plot/chapters/01.md"));
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(dir.path().join("plot/chapters/01.md"))
        .unwrap();

    let result = store.apply_changes(
        &[
            write_change(&chapter, "新しい章立て", WriteCondition::Any),
            trash_change(&scene, scene_hash),
        ],
        BackupMode::Never,
    );

    assert!(matches!(result, Err(ProjectError::Io { path, .. }) if path == chapter));
    assert_eq!(
        read(&store, "manuscript/01/s02.txt").unwrap(),
        "二つ目の本文"
    );
    assert_eq!(read(&store, "plot/chapters/01.md").unwrap(), "元の章立て");
    assert_eq!(
        fs::read_dir(dir.path().join(".kataribe/trash"))
            .unwrap()
            .count(),
        0,
        "戻したあとに、空のゴミ箱フォルダが残らないはず"
    );
}

/// 同じ内容を前提にした、書き込みとゴミ箱への移動が同時に来ても、通るのは 1 つだけ（残りは競合）。
/// 排他が無いと、どちらも条件を通り、書いた内容が消える（移した直後に書き直される）ことがある。
#[test]
fn a_write_and_a_trash_based_on_the_same_content_let_only_one_through() {
    const ROUNDS: usize = 20;
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let path = rel("characters/rin.md");

    for round in 0..ROUNDS {
        let base = write(&store, "characters/rin.md", &format!("元の内容 {round}"));
        let barrier = std::sync::Barrier::new(2);
        let (written, trashed) = std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                barrier.wait();
                store.write_text(
                    &path,
                    "書き換えた内容",
                    WriteOptions {
                        condition: WriteCondition::Matches(base.clone()),
                        backup: BackupMode::Never,
                    },
                )
            });
            let trasher = scope.spawn(|| {
                barrier.wait();
                store.apply_changes(&[trash_change(&path, base.clone())], BackupMode::Never)
            });
            (writer.join().unwrap(), trasher.join().unwrap())
        });

        assert_ne!(
            written.is_ok(),
            trashed.is_ok(),
            "round {round}: 通るのは片方だけのはず（書き込み {written:?}、ゴミ箱 {trashed:?}）"
        );
        if written.is_ok() {
            assert_eq!(read(&store, "characters/rin.md").unwrap(), "書き換えた内容");
        } else {
            assert!(!store.exists(&path));
        }
        if store.exists(&path) {
            store.remove(&path).unwrap();
        }
    }
}

/// 元の場所の親フォルダが無い（戻せない）、ゴミ箱へ移したあとの状態を作る。
struct StrandedTrash {
    path: RelPath,
    original: PathBuf,
    trashed: RelPath,
    resolved_trashed: PathBuf,
    batch_dir: PathBuf,
}

fn stranded_trash(dir: &TempDir, path: &str) -> StrandedTrash {
    let trashed = rel(&format!(".kataribe/trash/20231114-221320-000/{path}"));
    let resolved_trashed = dir.path().join(trashed.as_str());
    fs::create_dir_all(resolved_trashed.parent().unwrap()).unwrap();
    fs::write(&resolved_trashed, "ゴミ箱の中の本文").unwrap();
    StrandedTrash {
        path: rel(path),
        original: dir.path().join("moved-away").join(path),
        trashed,
        resolved_trashed,
        batch_dir: dir.path().join(".kataribe/trash/20231114-221320-000"),
    }
}

impl StrandedTrash {
    fn applied(&self) -> Applied<'_> {
        Applied::Trashed(TrashMove {
            path: &self.path,
            original: &self.original,
            trashed: self.trashed.clone(),
            resolved_trashed: self.resolved_trashed.clone(),
            batch_dir: self.batch_dir.clone(),
        })
    }
}

fn some_failure() -> ProjectError {
    ProjectError::NotFound {
        path: rel("plot/chapters/01.md"),
    }
}

#[test]
fn a_file_that_cannot_leave_the_trash_is_reported_with_its_place_in_the_trash() {
    let dir = TempDir::new().unwrap();
    let stranded = stranded_trash(&dir, "characters/rin.md");

    let error = roll_back(&[stranded.applied()], None, some_failure());

    let ProjectError::PartialWrite {
        not_restored,
        still_trashed,
        ..
    } = &error
    else {
        panic!("PartialWrite になるはず: {error:?}");
    };
    assert_eq!(*not_restored, Vec::<RelPath>::new());
    assert_eq!(
        *still_trashed,
        vec![StillTrashed {
            original: rel("characters/rin.md"),
            trashed: rel(".kataribe/trash/20231114-221320-000/characters/rin.md"),
        }]
    );
    let message = error.to_string();
    assert!(
        message.contains(".kataribe/trash/20231114-221320-000/characters/rin.md"),
        "{message}"
    );
    assert!(
        !message.contains(".kataribe/backups"),
        "ゴミ箱へ移したファイルのバックアップは無い: {message}"
    );
    assert_eq!(
        fs::read_to_string(&stranded.resolved_trashed).unwrap(),
        "ゴミ箱の中の本文",
        "戻せなかった実物はゴミ箱に残す"
    );
}

#[test]
fn a_written_file_that_cannot_be_restored_is_reported_apart_from_the_trashed_ones() {
    let dir = TempDir::new().unwrap();
    let stranded = stranded_trash(&dir, "manuscript/01/s02.txt");
    let chapter = rel("plot/chapters/01.md");
    let unwritable = PlannedWrite {
        path: &chapter,
        resolved: dir.path().join("moved-away/plot/chapters/01.md"),
        previous: Some(PreviousFile {
            source: chapter.clone(),
            text: TextFile {
                content: "元".to_owned(),
                hash: ContentHash::of("元".as_bytes()),
            },
            bytes: "元".as_bytes().to_vec(),
        }),
        content: "新".to_owned(),
        hash: ContentHash::of("新".as_bytes()),
    };

    let error = roll_back(
        &[stranded.applied(), Applied::Replaced(&unwritable)],
        None,
        some_failure(),
    );

    let ProjectError::PartialWrite {
        not_restored,
        still_trashed,
        ..
    } = error
    else {
        panic!("PartialWrite になるはず: {error:?}");
    };
    assert_eq!(not_restored, vec![chapter]);
    assert_eq!(still_trashed.len(), 1);
    assert_eq!(still_trashed[0].original, rel("manuscript/01/s02.txt"));
}

#[test]
fn roll_back_returns_the_original_error_when_everything_was_restored() {
    let error = roll_back(&[], None, some_failure());

    assert!(matches!(error, ProjectError::NotFound { .. }), "{error:?}");
}

#[test]
fn a_failed_first_move_to_the_trash_leaves_no_empty_folder_behind() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let missing = rel("characters/rin.md");

    let result = store.trash_only(&missing, dir.path().join("characters/rin.md"));

    assert!(matches!(result, Err(ProjectError::Io { .. })), "{result:?}");
    assert_eq!(
        fs::read_dir(dir.path().join(".kataribe/trash")).map_or(0, Iterator::count),
        0,
        "失敗したのに、空のゴミ箱フォルダが残っている"
    );
}

#[test]
fn a_failed_move_cleans_only_the_folders_it_made_and_keeps_the_ones_already_moved() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "characters/rin.md", "霧島 凛");
    let (moved, missing) = (rel("characters/rin.md"), rel("manuscript/01/s02.txt"));
    let trashes = [
        PlannedTrash {
            path: &moved,
            resolved: dir.path().join("characters/rin.md"),
        },
        PlannedTrash {
            path: &missing,
            resolved: dir.path().join("manuscript/01/s02.txt"),
        },
    ];
    let batch_dir = rel(".kataribe/trash/20231114-221320-000");
    let mut applied = Vec::new();

    let result = store.move_to_trash(&trashes, &batch_dir, &mut applied);

    assert!(matches!(result, Err(ProjectError::Io { path, .. }) if path == missing));
    assert_eq!(applied.len(), 1, "移せた 1 つ目は記録されているはず");
    let batch_dir = fs::read_dir(dir.path().join(".kataribe/trash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(batch_dir.join("characters/rin.md").exists());
    assert!(
        !batch_dir.join("manuscript").exists(),
        "失敗した 2 つ目のために作ったフォルダが残っている"
    );
}
