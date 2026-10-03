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
fn a_trash_whose_file_list_is_not_exactly_the_one_file_is_refused() {
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
        vec![],
        vec![trashed("characters/rin.md", None)],
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
