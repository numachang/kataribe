//! 改名（移動）・フォルダのゴミ箱・状態の確認（Expect）のテスト。

use std::fs;

use tempfile::TempDir;

use super::super::super::test_support::{read, rel, write};
use super::super::super::{BackupMode, ContentHash, ProjectStore, WriteCondition};
use super::super::{EntryCondition, PendingChange, PendingWrite};
use super::{open_store, open_store_with_fixed_clock};
use crate::error::ProjectError;
use crate::path::RelPath;

fn move_change<'a>(from: &'a RelPath, to: &'a RelPath) -> PendingChange<'a> {
    PendingChange::Move { from, to }
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

fn expect_change(path: &RelPath, expected: Option<ContentHash>) -> PendingChange<'_> {
    PendingChange::Expect { path, expected }
}

/// `folder` の中のファイル（サブフォルダの下も含む）を、パスとハッシュの一覧にする（フォルダのゴミ箱の条件）。
fn folder_condition(store: &ProjectStore, folder: &str) -> EntryCondition {
    let files = store.read_folder(&rel(folder)).unwrap().unwrap();
    EntryCondition::Folder(
        files
            .into_iter()
            .map(|file| (file.path, file.text.unwrap().hash))
            .collect(),
    )
}

fn trash_folder_change<'a>(path: &'a RelPath, store: &ProjectStore) -> PendingChange<'a> {
    PendingChange::Trash {
        path,
        expected: folder_condition(store, path.as_str()),
    }
}

fn apply(store: &ProjectStore, changes: &[PendingChange<'_>]) -> Result<(), ProjectError> {
    store.apply_changes(changes, BackupMode::Never).map(|_| ())
}

fn staging_entries(dir: &TempDir) -> usize {
    fs::read_dir(dir.path().join(".kataribe/staging")).map_or(0, Iterator::count)
}

// ---- 改名 ----

#[test]
fn a_file_and_a_folder_are_renamed_with_their_contents_unchanged() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "plot/chapters/03.md", "三章");
    write(&store, "manuscript/03/s01.txt", "一つ目");
    write(&store, "manuscript/03/notes/memo.txt", "メモ");
    let (chapter_from, chapter_to) = (rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));
    let (text_from, text_to) = (rel("manuscript/03"), rel("manuscript/04"));

    apply(
        &store,
        &[
            move_change(&chapter_from, &chapter_to),
            move_change(&text_from, &text_to),
        ],
    )
    .unwrap();

    assert_eq!(read(&store, "plot/chapters/04.md").unwrap(), "三章");
    assert_eq!(read(&store, "manuscript/04/s01.txt").unwrap(), "一つ目");
    assert_eq!(
        read(&store, "manuscript/04/notes/memo.txt").unwrap(),
        "メモ"
    );
    assert!(!store.exists(&chapter_from));
    assert!(!store.exists(&text_from));
    assert_eq!(staging_entries(&dir), 0, "置き場が残っている");
}

#[test]
fn moving_creates_the_missing_parent_folders_of_the_destination() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "concept.md", "企画");
    let (from, to) = (rel("concept.md"), rel("archive/old/concept.md"));

    apply(&store, &[move_change(&from, &to)]).unwrap();

    assert_eq!(read(&store, "archive/old/concept.md").unwrap(), "企画");
}

#[test]
fn consecutive_renames_shift_every_chapter_up_by_one_in_a_single_call() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    for number in 1..=3 {
        write(
            &store,
            &format!("plot/chapters/0{number}.md"),
            &format!("章{number}"),
        );
        write(
            &store,
            &format!("manuscript/0{number}/s01.txt"),
            &format!("本文{number}"),
        );
    }
    let paths: Vec<(RelPath, RelPath)> = (2..=3)
        .rev()
        .flat_map(|number| {
            [
                (
                    rel(&format!("plot/chapters/0{number}.md")),
                    rel(&format!("plot/chapters/0{}.md", number + 1)),
                ),
                (
                    rel(&format!("manuscript/0{number}")),
                    rel(&format!("manuscript/0{}", number + 1)),
                ),
            ]
        })
        .collect();
    let changes: Vec<PendingChange<'_>> = paths
        .iter()
        .map(|(from, to)| move_change(from, to))
        .collect();

    apply(&store, &changes).unwrap();

    assert_eq!(read(&store, "plot/chapters/01.md").unwrap(), "章1");
    assert_eq!(read(&store, "plot/chapters/03.md").unwrap(), "章2");
    assert_eq!(read(&store, "plot/chapters/04.md").unwrap(), "章3");
    assert_eq!(read(&store, "manuscript/03/s01.txt").unwrap(), "本文2");
    assert_eq!(read(&store, "manuscript/04/s01.txt").unwrap(), "本文3");
    assert!(!store.exists(&rel("plot/chapters/02.md")));
    assert!(!store.exists(&rel("manuscript/02")));
}

#[test]
fn two_files_can_be_swapped_in_one_call() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "plot/chapters/01.md", "最初");
    write(&store, "plot/chapters/02.md", "次");
    let (first, second) = (rel("plot/chapters/01.md"), rel("plot/chapters/02.md"));

    apply(
        &store,
        &[move_change(&first, &second), move_change(&second, &first)],
    )
    .unwrap();

    assert_eq!(read(&store, "plot/chapters/01.md").unwrap(), "次");
    assert_eq!(read(&store, "plot/chapters/02.md").unwrap(), "最初");
}

#[test]
fn three_folders_can_be_rotated_in_one_call() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    for (folder, text) in [("a", "A"), ("b", "B"), ("c", "C")] {
        write(&store, &format!("manuscript/{folder}/s01.txt"), text);
    }
    let (a, b, c) = (
        rel("manuscript/a"),
        rel("manuscript/b"),
        rel("manuscript/c"),
    );

    apply(
        &store,
        &[
            move_change(&a, &b),
            move_change(&b, &c),
            move_change(&c, &a),
        ],
    )
    .unwrap();

    assert_eq!(read(&store, "manuscript/a/s01.txt").unwrap(), "C");
    assert_eq!(read(&store, "manuscript/b/s01.txt").unwrap(), "A");
    assert_eq!(read(&store, "manuscript/c/s01.txt").unwrap(), "B");
}

#[test]
fn renaming_a_missing_source_conflicts_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "plot/chapters/01.md", "最初");
    let (present, missing, target) = (
        rel("plot/chapters/01.md"),
        rel("plot/chapters/02.md"),
        rel("plot/chapters/03.md"),
    );

    let result = apply(
        &store,
        &[
            move_change(&present, &target),
            move_change(&missing, &present),
        ],
    );

    assert!(matches!(result, Err(ProjectError::Conflict { path }) if path == missing));
    assert_eq!(read(&store, "plot/chapters/01.md").unwrap(), "最初");
    assert!(!store.exists(&target));
}

#[test]
fn renaming_onto_an_occupied_place_conflicts_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "manuscript/02/s01.txt", "二章");
    write(&store, "manuscript/03/s01.txt", "三章（章立ての無い本文）");
    let (from, to) = (rel("manuscript/02"), rel("manuscript/03"));

    let result = apply(&store, &[move_change(&from, &to)]);

    assert!(matches!(result, Err(ProjectError::Conflict { path }) if path == to));
    assert_eq!(read(&store, "manuscript/02/s01.txt").unwrap(), "二章");
    assert_eq!(
        read(&store, "manuscript/03/s01.txt").unwrap(),
        "三章（章立ての無い本文）"
    );
}

#[test]
fn renaming_onto_a_place_that_a_trash_empties_in_the_same_call_works() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let removed_hash = write(&store, "plot/chapters/02.md", "消す章");
    write(&store, "plot/chapters/03.md", "残る章");
    write(&store, "manuscript/02/s01.txt", "消す本文");
    write(&store, "manuscript/03/s01.txt", "残る本文");
    let (removed, next) = (rel("plot/chapters/02.md"), rel("plot/chapters/03.md"));
    let (removed_text, next_text) = (rel("manuscript/02"), rel("manuscript/03"));

    apply(
        &store,
        &[
            PendingChange::Trash {
                path: &removed,
                expected: EntryCondition::File(removed_hash),
            },
            trash_folder_change(&removed_text, &store),
            move_change(&next, &removed),
            move_change(&next_text, &removed_text),
        ],
    )
    .unwrap();

    assert_eq!(read(&store, "plot/chapters/02.md").unwrap(), "残る章");
    assert_eq!(read(&store, "manuscript/02/s01.txt").unwrap(), "残る本文");
    assert!(!store.exists(&next));
    assert!(!store.exists(&next_text));
    let trashed = fs::read_dir(dir.path().join(".kataribe/trash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(trashed.join("plot/chapters/02.md")).unwrap(),
        "消す章"
    );
    assert_eq!(
        fs::read_to_string(trashed.join("manuscript/02/s01.txt")).unwrap(),
        "消す本文"
    );
}

#[test]
fn the_backup_of_a_file_renamed_and_then_overwritten_is_kept_under_the_old_name() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "plot/chapters/01.md", "元の章");
    let (from, to) = (rel("plot/chapters/01.md"), rel("plot/chapters/02.md"));

    store
        .apply_changes(
            &[
                move_change(&from, &to),
                write_change(&to, "書き直した章", WriteCondition::Matches(hash)),
            ],
            BackupMode::Always,
        )
        .unwrap();

    assert_eq!(read(&store, "plot/chapters/02.md").unwrap(), "書き直した章");
    let backups = store.backups(&from).unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join(backups[0].path.as_str())).unwrap(),
        "元の章"
    );
}

// ---- 改名したあとの書き込みの条件 ----

#[test]
fn a_new_file_can_be_written_where_a_renamed_file_used_to_be() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "plot/chapters/03.md", "三章");
    let (from, to) = (rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));

    let hashes = store
        .apply_changes(
            &[
                move_change(&from, &to),
                write_change(&from, "新しい三章", WriteCondition::Absent),
            ],
            BackupMode::Always,
        )
        .unwrap();

    assert_eq!(read(&store, "plot/chapters/03.md").unwrap(), "新しい三章");
    assert_eq!(read(&store, "plot/chapters/04.md").unwrap(), "三章");
    assert_eq!(hashes[0], store.read_text(&from).unwrap().hash);
    assert_eq!(
        store.backups(&from).unwrap().len(),
        0,
        "空いた場所への書き込みは、上書きではないのでバックアップは無い"
    );
}

#[test]
fn a_write_into_a_place_a_rename_emptied_treats_the_old_content_as_gone() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let old_hash = write(&store, "plot/chapters/03.md", "三章");
    let (from, to) = (rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));

    let result = apply(
        &store,
        &[
            move_change(&from, &to),
            write_change(&from, "新しい三章", WriteCondition::Matches(old_hash)),
        ],
    );

    assert!(matches!(result, Err(ProjectError::Conflict { path }) if path == from));
    assert_eq!(read(&store, "plot/chapters/03.md").unwrap(), "三章");
    assert!(!store.exists(&to));
}

#[test]
fn a_write_onto_a_rename_destination_is_checked_against_the_renamed_source() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "plot/chapters/01.md", "元の章");
    let (from, to) = (rel("plot/chapters/01.md"), rel("plot/chapters/02.md"));
    let stale = ContentHash::of("違う内容".as_bytes());

    for (condition, should_pass) in [
        (WriteCondition::Matches(hash.clone()), true),
        (WriteCondition::Any, true),
        (WriteCondition::Matches(stale), false),
        (WriteCondition::Absent, false),
    ] {
        let label = format!("{condition:?}");
        let result = apply(
            &store,
            &[
                move_change(&from, &to),
                write_change(&to, "書き直し", condition),
            ],
        );
        assert_eq!(result.is_ok(), should_pass, "{label}: {result:?}");
        if should_pass {
            assert_eq!(read(&store, "plot/chapters/02.md").unwrap(), "書き直し");
            // 次の条件のために、元に戻す
            apply(&store, &[move_change(&to, &from)]).unwrap();
            write(&store, "plot/chapters/01.md", "元の章");
        }
    }
}

#[test]
fn a_write_inside_a_renamed_folder_is_checked_against_the_same_file_in_the_source_folder() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "manuscript/03/s01.txt", "元の本文");
    let (from, to) = (rel("manuscript/03"), rel("manuscript/04"));
    let inside = rel("manuscript/04/s01.txt");

    apply(
        &store,
        &[
            move_change(&from, &to),
            write_change(&inside, "書き直した本文", WriteCondition::Matches(hash)),
        ],
    )
    .unwrap();

    assert_eq!(
        read(&store, "manuscript/04/s01.txt").unwrap(),
        "書き直した本文"
    );
}

// ---- フォルダのゴミ箱 ----

#[test]
fn a_folder_goes_to_the_trash_with_everything_in_it_when_its_files_are_as_confirmed() {
    let dir = TempDir::new().unwrap();
    let store = open_store_with_fixed_clock(&dir);
    write(&store, "manuscript/03/s01.txt", "一つ目");
    write(&store, "manuscript/03/notes/memo.txt", "メモ");
    write(&store, "manuscript/04/s01.txt", "別の章");
    let folder = rel("manuscript/03");

    apply(&store, &[trash_folder_change(&folder, &store)]).unwrap();

    assert!(!store.exists(&folder));
    let batch = ".kataribe/trash/20231114-221320-000";
    assert_eq!(
        read(&store, &format!("{batch}/manuscript/03/s01.txt")).unwrap(),
        "一つ目"
    );
    assert_eq!(
        read(&store, &format!("{batch}/manuscript/03/notes/memo.txt")).unwrap(),
        "メモ"
    );
    assert_eq!(read(&store, "manuscript/04/s01.txt").unwrap(), "別の章");
}

#[test]
fn an_empty_folder_can_go_to_the_trash_with_an_empty_list() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    fs::create_dir_all(dir.path().join("manuscript/03")).unwrap();
    let folder = rel("manuscript/03");

    apply(
        &store,
        &[PendingChange::Trash {
            path: &folder,
            expected: EntryCondition::Folder(Vec::new()),
        }],
    )
    .unwrap();

    assert!(!store.exists(&folder));
}

#[test]
fn a_folder_conflicts_when_a_file_was_added_after_it_was_confirmed() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "manuscript/03/s01.txt", "一つ目");
    let folder = rel("manuscript/03");
    let confirmed = trash_folder_change(&folder, &store);
    write(&store, "manuscript/03/s02.txt", "後から足された本文");

    let result = apply(&store, &[confirmed]);

    assert!(matches!(result, Err(ProjectError::Conflict { path }) if path == folder));
    assert_eq!(
        read(&store, "manuscript/03/s02.txt").unwrap(),
        "後から足された本文"
    );
    assert!(!store.exists(&rel(".kataribe/trash")));
}

#[test]
fn a_folder_conflicts_when_a_file_in_it_changed_was_removed_or_was_hidden() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "manuscript/03/s01.txt", "一つ目");
    write(&store, "manuscript/03/s02.txt", "二つ目");
    let folder = rel("manuscript/03");
    let confirmed = folder_condition(&store, "manuscript/03");

    let outside_edits: [&dyn Fn(); 3] = [
        &|| {
            write(&store, "manuscript/03/s01.txt", "外で書き換えられた");
        },
        &|| {
            store.remove(&rel("manuscript/03/s02.txt")).unwrap();
        },
        &|| {
            write(&store, "manuscript/03/.hidden", "隠しファイル");
        },
    ];
    for edit in outside_edits {
        write(&store, "manuscript/03/s01.txt", "一つ目");
        write(&store, "manuscript/03/s02.txt", "二つ目");
        let _ = store.remove(&rel("manuscript/03/.hidden"));
        edit();

        let result = apply(
            &store,
            &[PendingChange::Trash {
                path: &folder,
                expected: confirmed.clone(),
            }],
        );

        assert!(
            matches!(result, Err(ProjectError::Conflict { .. })),
            "競合になるはず: {result:?}"
        );
        assert!(store.exists(&folder));
    }
}

#[test]
fn a_folder_expectation_conflicts_when_the_path_is_a_file_or_missing() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "manuscript/03", "フォルダではなくファイル");
    let (file, missing) = (rel("manuscript/03"), rel("manuscript/04"));

    for path in [&file, &missing] {
        let result = apply(
            &store,
            &[PendingChange::Trash {
                path,
                expected: EntryCondition::Folder(Vec::new()),
            }],
        );
        assert!(
            matches!(result, Err(ProjectError::Conflict { .. })),
            "{path}: {result:?}"
        );
    }
}

#[test]
fn a_file_expectation_conflicts_when_the_path_is_a_folder() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "manuscript/03/s01.txt", "本文");
    let folder = rel("manuscript/03");

    let result = apply(
        &store,
        &[PendingChange::Trash {
            path: &folder,
            expected: EntryCondition::File(hash),
        }],
    );

    assert!(
        matches!(result, Err(ProjectError::Conflict { .. })),
        "{result:?}"
    );
    assert!(store.exists(&folder));
}

// ---- 状態の確認 ----

#[test]
fn expecting_that_a_path_is_absent_passes_when_nothing_is_there() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let path = rel("manuscript/01/s02.txt");

    apply(&store, &[expect_change(&path, None)]).unwrap();

    assert!(!store.exists(&path));
}

#[test]
fn expecting_that_a_path_is_absent_conflicts_when_a_file_or_folder_appeared() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "concept.md", "企画");
    write(&store, "manuscript/03/s01.txt", "本文");
    write(&store, "characters/rin.md", "凛");
    let (appeared_file, appeared_folder) = (rel("manuscript/03/s01.txt"), rel("manuscript/03"));
    let unrelated = rel("characters/rin.md");

    for path in [&appeared_file, &appeared_folder] {
        let result = apply(
            &store,
            &[
                write_change(&unrelated, "書き換え", WriteCondition::Any),
                expect_change(path, None),
            ],
        );
        assert!(
            matches!(result, Err(ProjectError::Conflict { path: ref conflicting }) if conflicting == path),
            "{path}: {result:?}"
        );
    }
    assert_eq!(
        read(&store, "characters/rin.md").unwrap(),
        "凛",
        "確認に失敗したら、ほかの書き込みも行わない"
    );
}

#[test]
fn expecting_a_hash_passes_for_the_same_content_and_conflicts_otherwise() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "concept.md", "企画");
    let path = rel("concept.md");
    let missing = rel("style.md");

    apply(&store, &[expect_change(&path, Some(hash.clone()))]).unwrap();

    let stale = ContentHash::of("違う内容".as_bytes());
    for (path, expected) in [(&path, stale), (&missing, hash)] {
        let result = apply(&store, &[expect_change(path, Some(expected))]);
        assert!(
            matches!(result, Err(ProjectError::Conflict { .. })),
            "{path}: {result:?}"
        );
    }
}

#[test]
fn an_expectation_writes_nothing_and_leaves_no_backup_or_staging_behind() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let hash = write(&store, "concept.md", "企画");
    let path = rel("concept.md");

    store
        .apply_changes(&[expect_change(&path, Some(hash))], BackupMode::Always)
        .unwrap();

    assert_eq!(store.backups(&path).unwrap(), Vec::new());
    assert_eq!(staging_entries(&dir), 0);
}

// ---- 形の検証が apply_changes につながっている ----

#[test]
fn invalid_shapes_are_refused_before_anything_is_changed() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "manuscript/03/s01.txt", "本文");
    write(&store, ".kataribe/cache/summary.json", "{}");
    let (folder, inside, internal, elsewhere) = (
        rel("manuscript/03"),
        rel("manuscript/03/old"),
        rel(".kataribe/cache/summary.json"),
        rel("manuscript/09"),
    );

    for invalid in [
        vec![move_change(&folder, &inside)],
        vec![move_change(&internal, &elsewhere)],
        vec![move_change(&folder, &internal)],
        vec![
            trash_folder_change(&folder, &store),
            write_change(&rel("manuscript/03/s09.txt"), "x", WriteCondition::Any),
        ],
    ] {
        let result = apply(&store, &invalid);
        assert!(
            matches!(result, Err(ProjectError::InvalidChangeSet { .. })),
            "{result:?}"
        );
    }
    assert_eq!(read(&store, "manuscript/03/s01.txt").unwrap(), "本文");
    assert!(store.exists(&internal));
}
