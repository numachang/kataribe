//! 改名・フォルダのゴミ箱を含む反映が途中で失敗したときの巻き戻しと、操作の記録のテスト。

use std::fs;

use tempfile::TempDir;

use super::super::super::test_support::{read, rel, write};
use super::super::applied::{Applied, MoveStep, roll_back};
use super::super::journal::Journal;
use super::super::prepare::PlannedMove;
use super::super::{EntryCondition, PendingChange, PendingWrite};
use super::open_store;
use crate::error::{ProjectError, StillMoved};
use crate::path::RelPath;
use crate::store::{BackupMode, ProjectStore, WriteCondition};

/// 章 01〜03 の章立てと、それぞれ 1 つの本文を用意する（章の番号の振り直しを失敗させるテストで共有する）。
fn insert_chapter_setup(store: &ProjectStore) {
    for number in 1..=3 {
        write(
            store,
            &format!("plot/chapters/0{number}.md"),
            &format!("章{number}"),
        );
        write(
            store,
            &format!("manuscript/0{number}/s01.txt"),
            &format!("本文{number}"),
        );
    }
}

fn assert_nothing_changed(dir: &TempDir, store: &ProjectStore) {
    for number in 1..=3 {
        assert_eq!(
            read(store, &format!("plot/chapters/0{number}.md")).unwrap(),
            format!("章{number}")
        );
        assert_eq!(
            read(store, &format!("manuscript/0{number}/s01.txt")).unwrap(),
            format!("本文{number}")
        );
    }
    assert!(!store.exists(&rel("plot/chapters/04.md")));
    assert!(!store.exists(&rel("manuscript/04")));
    assert_eq!(
        fs::read_dir(dir.path().join(".kataribe/trash")).map_or(0, Iterator::count),
        0,
        "戻したあとに、ゴミ箱のフォルダが残っている"
    );
    assert_eq!(
        fs::read_dir(dir.path().join(".kataribe/staging")).map_or(0, Iterator::count),
        0,
        "戻したあとに、置き場が残っている"
    );
}

/// 後ろの章の本文のフォルダの改名が、ほかのアプリが開いているファイルのせいで失敗したら、
/// 先に済んだ改名とゴミ箱への移動をすべて元に戻して失敗する。
#[cfg(windows)]
#[test]
fn renames_and_trashes_are_all_undone_when_a_folder_is_locked_by_another_app() {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_SHARE_READ: u32 = 0x1;

    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    insert_chapter_setup(&store);
    write(&store, "world/glossary.md", "用語集");
    let glossary_hash = store.read_text(&rel("world/glossary.md")).unwrap().hash;
    let (glossary, md_03, md_04) = (
        rel("world/glossary.md"),
        rel("plot/chapters/03.md"),
        rel("plot/chapters/04.md"),
    );
    let (text_03, text_04) = (rel("manuscript/03"), rel("manuscript/04"));
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(dir.path().join("manuscript/03/s01.txt"))
        .unwrap();

    let result = store.apply_changes(
        &[
            PendingChange::Trash {
                path: &glossary,
                expected: EntryCondition::File(glossary_hash),
            },
            PendingChange::Move {
                from: &md_03,
                to: &md_04,
            },
            PendingChange::Move {
                from: &text_03,
                to: &text_04,
            },
        ],
        BackupMode::Always,
    );

    assert!(
        matches!(result, Err(ProjectError::Io { ref path, .. }) if *path == text_03),
        "{result:?}"
    );
    assert_eq!(read(&store, "world/glossary.md").unwrap(), "用語集");
    assert_nothing_changed(&dir, &store);
}

/// 置き換えの段階（改名のあと）で失敗したら、改名もゴミ箱も元に戻す。
/// 書き込み先の親になるはずの場所がファイルなので、フォルダを作れず失敗する。
#[test]
fn renames_and_trashes_are_all_undone_when_the_last_write_cannot_be_made() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    insert_chapter_setup(&store);
    let (md_03, md_04) = (rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));
    let (text_03, text_04) = (rel("manuscript/03"), rel("manuscript/04"));
    let (md_01, unwritable) = (
        rel("plot/chapters/01.md"),
        rel("plot/chapters/01.md/new.md"),
    );
    let hash = store.read_text(&md_01).unwrap().hash;

    let result = store.apply_changes(
        &[
            PendingChange::Trash {
                path: &md_01,
                expected: EntryCondition::File(hash),
            },
            PendingChange::Move {
                from: &md_03,
                to: &md_04,
            },
            PendingChange::Move {
                from: &text_03,
                to: &text_04,
            },
            PendingChange::Write(PendingWrite {
                path: &unwritable,
                content: "書けない",
                condition: WriteCondition::Absent,
            }),
        ],
        BackupMode::Always,
    );

    assert!(result.is_err(), "{result:?}");
    assert_nothing_changed(&dir, &store);
}

/// 改名の 2 段階目（移動先へ置く）で失敗したら、1 段階目で預けたものも元の場所へ戻す。
/// 移動先の親になるはずの場所がファイルなので、フォルダを作れず失敗する。
#[test]
fn renames_are_undone_when_a_destination_cannot_be_made() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "plot/chapters/01.md", "章");
    write(&store, "manuscript/01/s01.txt", "本文");
    write(&store, "concept.md", "企画");
    let (chapter, text) = (rel("plot/chapters/01.md"), rel("manuscript/01"));
    let (chapter_to, text_to) = (rel("plot/chapters/02.md"), rel("concept.md/01"));

    let result = store.apply_changes(
        &[
            PendingChange::Move {
                from: &chapter,
                to: &chapter_to,
            },
            PendingChange::Move {
                from: &text,
                to: &text_to,
            },
        ],
        BackupMode::Always,
    );

    assert!(result.is_err(), "{result:?}");
    assert_eq!(read(&store, "plot/chapters/01.md").unwrap(), "章");
    assert!(!store.exists(&chapter_to));
    assert_eq!(read(&store, "manuscript/01/s01.txt").unwrap(), "本文");
    assert_eq!(read(&store, "concept.md").unwrap(), "企画");
    assert_eq!(
        fs::read_dir(dir.path().join(".kataribe/staging")).map_or(0, Iterator::count),
        0,
        "戻したあとに、置き場が残っている"
    );
}

// ---- 戻せなかったとき ----

/// 作品フォルダの外の、存在しない場所を指す改名。預け先からも移動先からも、実物を動かせない。
fn unreachable_move<'a>(dir: &TempDir, from: &'a RelPath, to: &'a RelPath) -> PlannedMove<'a> {
    PlannedMove {
        from,
        to,
        from_resolved: dir.path().join("gone").join(from.as_str()),
        to_resolved: dir.path().join("gone").join(to.as_str()),
    }
}

#[test]
fn a_rename_that_cannot_be_undone_is_reported_with_where_it_was_left_and_the_journal() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let staging = store.create_staging_dir().unwrap();
    let (from, to) = (rel("manuscript/03"), rel("manuscript/04"));
    let planned = unreachable_move(&dir, &from, &to);
    let step = MoveStep::plan(0, &planned, &staging).unwrap();
    let journal = staging.journal_path().unwrap();

    let error = roll_back(
        &[Applied::MovedAway(&step)],
        Some(&journal),
        super::some_failure(),
    );

    let ProjectError::PartialWrite {
        not_restored,
        still_trashed,
        still_moved,
        journal: reported_journal,
        ..
    } = &error
    else {
        panic!("PartialWrite になるはず: {error:?}");
    };
    assert_eq!(*not_restored, Vec::<RelPath>::new());
    assert_eq!(*still_trashed, Vec::new());
    assert_eq!(
        *still_moved,
        vec![StillMoved {
            original: from,
            current: staging.slot(0).unwrap().0,
        }]
    );
    assert_eq!(reported_journal.as_ref(), Some(&journal));
    let message = error.to_string();
    assert!(message.contains(journal.as_str()), "{message}");
    assert!(message.contains("m0 にあります"), "{message}");
}

#[test]
fn a_rename_whose_second_stage_cannot_be_undone_is_reported_once_at_the_destination() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let staging = store.create_staging_dir().unwrap();
    let (from, to) = (rel("manuscript/03"), rel("manuscript/04"));
    let planned = unreachable_move(&dir, &from, &to);
    let step = MoveStep::plan(0, &planned, &staging).unwrap();

    let error = roll_back(
        &[Applied::MovedAway(&step), Applied::MovedIn(&step)],
        None,
        super::some_failure(),
    );

    let ProjectError::PartialWrite { still_moved, .. } = error else {
        panic!("PartialWrite になるはず: {error:?}");
    };
    assert_eq!(
        still_moved,
        vec![StillMoved {
            original: from,
            current: to,
        }],
        "実物は移動先にある。預け先の取り消しは試さず、預け先にあると誤って案内しない"
    );
}

#[test]
fn renames_that_were_made_are_put_back_where_they_came_from() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    write(&store, "manuscript/03/s01.txt", "本文");
    let staging = store.create_staging_dir().unwrap();
    let (from, to) = (rel("manuscript/03"), rel("manuscript/04"));
    let planned = PlannedMove {
        from: &from,
        to: &to,
        from_resolved: dir.path().join("manuscript/03"),
        to_resolved: dir.path().join("manuscript/04"),
    };
    let step = MoveStep::plan(0, &planned, &staging).unwrap();
    step.move_away().unwrap();
    step.move_in().unwrap();
    assert_eq!(read(&store, "manuscript/04/s01.txt").unwrap(), "本文");

    let error = roll_back(
        &[Applied::MovedAway(&step), Applied::MovedIn(&step)],
        None,
        super::some_failure(),
    );

    assert!(matches!(error, ProjectError::NotFound { .. }), "{error:?}");
    assert_eq!(read(&store, "manuscript/03/s01.txt").unwrap(), "本文");
    assert!(!store.exists(&to));
}

// ---- 操作の記録 ----

#[test]
fn the_journal_lists_the_trashed_the_renamed_and_the_written() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let removed_hash = write(&store, "plot/chapters/02.md", "消す章");
    write(&store, "plot/chapters/03.md", "残る章");
    let (removed, from, to, added) = (
        rel("plot/chapters/02.md"),
        rel("plot/chapters/03.md"),
        rel("plot/chapters/04.md"),
        rel("plot/chapters/05.md"),
    );
    let changes = [
        PendingChange::Trash {
            path: &removed,
            expected: EntryCondition::File(removed_hash),
        },
        PendingChange::Move {
            from: &from,
            to: &to,
        },
        PendingChange::Write(PendingWrite {
            path: &added,
            content: "新しい章",
            condition: WriteCondition::Absent,
        }),
    ];
    let batch = store.prepare(&changes).unwrap();
    let staging = store.create_staging_dir().unwrap();
    let steps: Vec<MoveStep<'_>> = batch
        .moves
        .iter()
        .enumerate()
        .map(|(index, planned)| MoveStep::plan(index, planned, &staging).unwrap())
        .collect();
    let written: Vec<_> = batch.writes.iter().collect();
    let trash_dir = rel(".kataribe/trash/20231114-221320-000");

    let journal = Journal::describe(&batch, &written, Some(&trash_dir), &steps).unwrap();
    let journal_path = journal.write(&staging).unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&read(&store, journal_path.as_str()).unwrap()).unwrap();
    assert_eq!(json["trash_dir"], ".kataribe/trash/20231114-221320-000");
    assert_eq!(json["trashed"], serde_json::json!(["plot/chapters/02.md"]));
    assert_eq!(json["moves"][0]["from"], "plot/chapters/03.md");
    assert_eq!(json["moves"][0]["to"], "plot/chapters/04.md");
    assert!(
        json["moves"][0]["staged"]
            .as_str()
            .unwrap()
            .starts_with(".kataribe/staging/"),
        "{json}"
    );
    assert_eq!(json["written"], serde_json::json!(["plot/chapters/05.md"]));
}

#[test]
fn a_batch_of_writes_only_has_no_journal() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let path = rel("concept.md");
    let changes = [PendingChange::Write(PendingWrite {
        path: &path,
        content: "企画",
        condition: WriteCondition::Absent,
    })];
    let batch = store.prepare(&changes).unwrap();
    let written: Vec<_> = batch.writes.iter().collect();

    assert!(Journal::describe(&batch, &written, None, &[]).is_none());
}
