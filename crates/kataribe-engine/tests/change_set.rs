//! 変更案（書き込みとゴミ箱へ移す操作）の組み立てと適用を確かめる。
#![allow(clippy::unwrap_used)]

mod common;

use kataribe_engine::{ChangeSet, EngineError, FileChange, TrashedFile};
use kataribe_project::{Project, ProjectError, RelPath, TextFile};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use common::{new_project, put, read};

fn rel(path: &str) -> RelPath {
    RelPath::new(path).unwrap()
}

fn text_file(project: &Project, path: &str) -> TextFile {
    project.store().read_text(&rel(path)).unwrap()
}

fn assert_invalid_change_set(result: &Result<(), EngineError>) {
    assert!(
        matches!(
            result,
            Err(EngineError::Project(ProjectError::InvalidChangeSet { .. }))
        ),
        "形の誤りになるはず: {result:?}"
    );
}

#[test]
fn put_replaces_the_content_of_an_existing_write_to_the_same_path() {
    let mut changes = ChangeSet::new("まとめ");

    changes.put(rel("concept.md"), "一回目".into(), None);
    changes.put(rel("concept.md"), "二回目".into(), None);

    assert_eq!(changes.files.len(), 1);
    assert_eq!(changes.written(&rel("concept.md")), Some("二回目"));
    assert_eq!(changes.written(&rel("style.md")), None);
}

#[test]
fn trash_file_records_the_hash_and_the_character_count_without_ruby_and_spaces() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "manuscript/01/s01.txt", "|霧《きり》の 朝\n");
    let file = text_file(&project, "manuscript/01/s01.txt");
    let mut changes = ChangeSet::new("削除");

    changes.trash_file(rel("manuscript/01/s01.txt"), &file);

    assert_eq!(
        changes.files,
        vec![FileChange::Trash {
            path: rel("manuscript/01/s01.txt"),
            files: vec![TrashedFile {
                path: rel("manuscript/01/s01.txt"),
                base_hash: Some(file.hash),
                chars: 3,
            }],
        }]
    );
    assert_eq!(changes.written(&rel("manuscript/01/s01.txt")), None);
}

#[test]
fn file_changes_are_tagged_with_their_kind_in_json() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "凛");
    let mut changes = ChangeSet::new("まとめ");
    changes.put(rel("concept.md"), "企画".into(), None);
    changes.trash_file(
        rel("characters/rin.md"),
        &text_file(&project, "characters/rin.md"),
    );

    let json = serde_json::to_value(&changes).unwrap();

    assert_eq!(json["files"][0]["kind"], "write");
    assert_eq!(json["files"][0]["path"], "concept.md");
    assert_eq!(json["files"][1]["kind"], "trash");
    assert_eq!(json["files"][1]["files"][0]["path"], "characters/rin.md");
    let restored: ChangeSet = serde_json::from_value(json).unwrap();
    assert_eq!(restored, changes);
}

#[test]
fn a_change_set_with_a_write_and_a_trash_applies_both() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "凛");
    put(&project, "concept.md", "古い企画");
    let mut changes = ChangeSet::new("まとめ").made_for(&project);
    changes.trash_file(
        rel("characters/rin.md"),
        &text_file(&project, "characters/rin.md"),
    );
    changes.put(
        rel("concept.md"),
        "新しい企画".into(),
        Some(text_file(&project, "concept.md")),
    );

    changes.apply(&project).unwrap();

    assert_eq!(read(&project, "characters/rin.md"), None);
    assert_eq!(read(&project, "concept.md").unwrap(), "新しい企画");
}

#[test]
fn the_order_of_the_list_does_not_decide_what_is_applied() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "凛");
    let mut changes = ChangeSet::new("まとめ").made_for(&project);
    changes.put(rel("concept.md"), "企画".into(), None);
    changes.trash_file(
        rel("characters/rin.md"),
        &text_file(&project, "characters/rin.md"),
    );
    let (write, trash) = (changes.files[0].clone(), changes.files[1].clone());

    // 並び順を入れ替えても、どちらも反映される（並び順に頼らない）
    let reversed = ChangeSet {
        files: vec![trash, write],
        ..changes
    };
    reversed.apply(&project).unwrap();

    assert_eq!(read(&project, "characters/rin.md"), None);
    assert_eq!(read(&project, "concept.md").unwrap(), "企画");
}

#[test]
fn a_conflicting_trash_stops_the_write_in_the_same_change_set() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "読んだときの内容");
    let mut changes = ChangeSet::new("まとめ").made_for(&project);
    changes.trash_file(
        rel("characters/rin.md"),
        &text_file(&project, "characters/rin.md"),
    );
    changes.put(rel("concept.md"), "企画".into(), None);
    put(&project, "characters/rin.md", "外で書き換えられた内容");

    let error = changes.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(
        read(&project, "characters/rin.md").unwrap(),
        "外で書き換えられた内容"
    );
    assert_eq!(read(&project, "concept.md"), None);
}

#[test]
fn trashing_the_manifest_or_internal_data_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, ".kataribe/cache/summary.json", "{}");
    for path in ["kataribe.yaml", ".kataribe/cache/summary.json"] {
        let mut changes = ChangeSet::new("削除").made_for(&project);
        changes.trash_file(rel(path), &text_file(&project, path));

        assert_invalid_change_set(&changes.apply(&project));
    }

    assert!(read(&project, "kataribe.yaml").is_some());
    assert!(read(&project, ".kataribe/cache/summary.json").is_some());
}

#[test]
fn two_changes_to_the_same_path_are_refused() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "凛");
    let mut changes = ChangeSet::new("まとめ").made_for(&project);
    changes.trash_file(
        rel("characters/rin.md"),
        &text_file(&project, "characters/rin.md"),
    );
    changes.put(rel("characters/rin.md"), "書き直し".into(), None);

    assert_invalid_change_set(&changes.apply(&project));
    assert_eq!(read(&project, "characters/rin.md").unwrap(), "凛");
}

#[test]
fn a_trash_whose_file_list_does_not_fit_the_path_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "凛");
    let hash = text_file(&project, "characters/rin.md").hash;
    let trashed = |path: &str, base_hash| TrashedFile {
        path: rel(path),
        base_hash,
        chars: 1,
    };
    let malformed_lists = [
        // ハッシュが無い（テキストとして読めなかった）
        vec![trashed("characters/rin.md", None)],
        // フォルダの中にないファイル
        vec![trashed("characters/kenji.md", Some(hash.clone()))],
        vec![
            trashed("characters/rin.md", Some(hash.clone())),
            trashed("characters/rin.md", Some(hash)),
        ],
    ];

    for files in malformed_lists {
        let changes = ChangeSet {
            summary: "削除".into(),
            files: vec![FileChange::Trash {
                path: rel("characters/rin.md"),
                files,
            }],
            project_root: String::new(),
        }
        .made_for(&project);

        assert_invalid_change_set(&changes.apply(&project));
    }
    assert!(read(&project, "characters/rin.md").is_some());
}

#[test]
fn a_trash_with_an_empty_file_list_is_an_empty_folder_and_conflicts_for_a_file() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "凛");
    let changes = ChangeSet {
        summary: "削除".into(),
        files: vec![FileChange::Trash {
            path: rel("characters/rin.md"),
            files: Vec::new(),
        }],
        project_root: String::new(),
    }
    .made_for(&project);

    let error = changes.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert!(read(&project, "characters/rin.md").is_some());
}

#[test]
fn a_change_set_made_for_another_project_is_not_applied() {
    let first_dir = TempDir::new().unwrap();
    let second_dir = TempDir::new().unwrap();
    let first = new_project(first_dir.path());
    let second = new_project(second_dir.path());
    put(&first, "characters/rin.md", "凛");
    let mut changes = ChangeSet::new("削除").made_for(&first);
    changes.trash_file(
        rel("characters/rin.md"),
        &text_file(&first, "characters/rin.md"),
    );

    let error = changes.apply(&second).unwrap_err();

    assert!(matches!(error, EngineError::InvalidInput(_)), "{error:?}");
    assert!(read(&first, "characters/rin.md").is_some());
}

#[test]
fn move_and_expect_are_tagged_with_their_kind_in_json_and_survive_a_round_trip() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", "章");
    let mut changes = ChangeSet::new("まとめ");
    changes.move_entry(rel("plot/chapters/01.md"), rel("plot/chapters/02.md"));
    changes.expect(rel("manuscript/01"), None);
    changes.expect(
        rel("plot/chapters/01.md"),
        Some(&text_file(&project, "plot/chapters/01.md")),
    );

    let json = serde_json::to_value(&changes).unwrap();

    assert_eq!(json["files"][0]["kind"], "move");
    assert_eq!(json["files"][0]["from"], "plot/chapters/01.md");
    assert_eq!(json["files"][0]["to"], "plot/chapters/02.md");
    assert_eq!(json["files"][1]["kind"], "expect");
    assert_eq!(json["files"][1]["path"], "manuscript/01");
    assert_eq!(json["files"][1]["base_hash"], serde_json::Value::Null);
    assert_eq!(
        json["files"][2]["base_hash"],
        text_file(&project, "plot/chapters/01.md").hash.as_str()
    );
    let restored: ChangeSet = serde_json::from_value(json).unwrap();
    assert_eq!(restored, changes);
}

#[test]
fn trash_folder_records_every_file_with_its_hash_and_character_count() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "manuscript/03/s01.txt", "|霧《きり》の 朝\n");
    put(&project, "manuscript/03/s02.txt", "昼\n");
    let folder = project
        .store()
        .read_folder(&rel("manuscript/03"))
        .unwrap()
        .unwrap();
    let mut changes = ChangeSet::new("削除");

    changes.trash_folder(rel("manuscript/03"), &folder);

    let [FileChange::Trash { path, files }] = changes.files.as_slice() else {
        panic!("ゴミ箱へ移す変更が 1 つのはず: {:?}", changes.files);
    };
    assert_eq!(path, &rel("manuscript/03"));
    let listed: Vec<(&str, usize)> = files
        .iter()
        .map(|file| (file.path.as_str(), file.chars))
        .collect();
    assert_eq!(
        listed,
        vec![("manuscript/03/s01.txt", 3), ("manuscript/03/s02.txt", 1)]
    );
    assert_eq!(
        files[0].base_hash,
        Some(text_file(&project, "manuscript/03/s01.txt").hash)
    );
}

#[test]
fn a_change_set_that_moves_a_chapter_and_writes_a_new_one_in_its_place_applies_in_any_order() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/03.md", "元の三章");
    put(&project, "manuscript/03/s01.txt", "三章の本文");
    let mut changes = ChangeSet::new("章を足す").made_for(&project);
    changes.put(rel("plot/chapters/03.md"), "新しい三章".into(), None);
    changes.move_entry(rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));
    changes.move_entry(rel("manuscript/03"), rel("manuscript/04"));
    changes.expect(rel("manuscript/05"), None);

    changes.apply(&project).unwrap();

    assert_eq!(read(&project, "plot/chapters/03.md").unwrap(), "新しい三章");
    assert_eq!(read(&project, "plot/chapters/04.md").unwrap(), "元の三章");
    assert_eq!(
        read(&project, "manuscript/04/s01.txt").unwrap(),
        "三章の本文"
    );
    assert_eq!(read(&project, "manuscript/03/s01.txt"), None);
}

#[test]
fn a_failed_expectation_stops_the_moves_and_writes_in_the_same_change_set() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/03.md", "三章");
    put(&project, "manuscript/05/s01.txt", "外で作られた本文");
    let mut changes = ChangeSet::new("章を足す").made_for(&project);
    changes.move_entry(rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));
    changes.expect(rel("manuscript/05"), None);

    let error = changes.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(read(&project, "plot/chapters/03.md").unwrap(), "三章");
    assert_eq!(read(&project, "plot/chapters/04.md"), None);
}

#[test]
fn moving_internal_data_or_into_itself_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "manuscript/03/s01.txt", "本文");
    put(&project, ".kataribe/cache/summary.json", "{}");
    for (from, to) in [
        (".kataribe/cache/summary.json", "manuscript/09"),
        ("kataribe.yaml", "manuscript/09"),
        ("manuscript/03", ".kataribe/trash"),
        ("manuscript/03", "manuscript/03/old"),
    ] {
        let mut changes = ChangeSet::new("移動").made_for(&project);
        changes.move_entry(rel(from), rel(to));

        assert_invalid_change_set(&changes.apply(&project));
    }

    assert_eq!(read(&project, "manuscript/03/s01.txt").unwrap(), "本文");
    assert!(read(&project, "kataribe.yaml").is_some());
}

#[test]
fn a_move_is_not_applied_to_another_project() {
    let first_dir = TempDir::new().unwrap();
    let second_dir = TempDir::new().unwrap();
    let first = new_project(first_dir.path());
    let second = new_project(second_dir.path());
    put(&second, "concept.md", "別の作品の企画");
    let mut changes = ChangeSet::new("移動").made_for(&first);
    changes.move_entry(rel("concept.md"), rel("moved.md"));

    let error = changes.apply(&second).unwrap_err();

    assert!(matches!(error, EngineError::InvalidInput(_)), "{error:?}");
    assert!(read(&second, "concept.md").is_some());
}
