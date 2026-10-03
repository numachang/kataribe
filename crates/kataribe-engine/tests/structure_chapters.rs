//! 章の追加・削除（番号の振り直し）を、作品フォルダに対して確かめる。
#![allow(clippy::unwrap_used)]

mod common;

use kataribe_engine::{
    DraftUnit, EngineError, EntryKind, FileChange, OverviewEntry, RenumberedChapter, SectionKind,
    StepState, StructureEdit, StructurePlan, Task, overview, pipeline, plan_structure_edit,
};
use kataribe_project::{ChapterId, Project, ProjectError, RelPath};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use common::{new_project, put, read};

fn chapter_id(number: u32) -> ChapterId {
    ChapterId::from_number(number)
}

fn rel(path: &str) -> RelPath {
    RelPath::new(path).unwrap()
}

/// 章題とストーリーラインだけの章立て。
fn chapter_md(title: &str, storyline: &str) -> String {
    format!("---\ntitle: {title}\n---\n{storyline}\n")
}

/// 章立てと、本文を 1 つ持つ章。`number` の章は、本文が `本文<number>` になる。
fn put_chapter_with_text(project: &Project, number: &str, title: &str) {
    put(
        project,
        &format!("plot/chapters/{number}.md"),
        &chapter_md(title, &format!("{title}のあらすじ")),
    );
    put(
        project,
        &format!("manuscript/{number}/s01.txt"),
        &format!("{title}の本文\n"),
    );
}

fn add_chapter(before: Option<u32>, title: &str) -> StructureEdit {
    StructureEdit::AddChapter {
        before: before.map(chapter_id),
        title: title.to_owned(),
        storyline: "新しい章のあらすじ".to_owned(),
    }
}

fn remove_chapter(number: u32) -> StructureEdit {
    StructureEdit::RemoveChapter {
        chapter: chapter_id(number),
    }
}

fn plan_and_apply(project: &Project, edit: &StructureEdit) -> StructurePlan {
    let plan = plan_structure_edit(project, edit).unwrap();
    plan.change_set.apply(project).unwrap();
    plan
}

fn chapter_numbers(project: &Project) -> Vec<String> {
    project
        .chapter_ids()
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn chapter_titles(project: &Project) -> Vec<String> {
    project
        .chapters()
        .unwrap()
        .into_iter()
        .map(|chapter| chapter.meta.title)
        .collect()
}

fn invalid_input_message(result: Result<StructurePlan, EngineError>) -> String {
    match result {
        Err(EngineError::InvalidInput(message)) => message,
        other => panic!("入力の誤りになるはず: {other:?}"),
    }
}

fn section_entries(project: &Project, kind: SectionKind) -> Vec<OverviewEntry> {
    overview(project)
        .unwrap()
        .sections
        .into_iter()
        .find(|section| section.kind == kind)
        .unwrap()
        .entries
}

fn renumbering(plan: &StructurePlan) -> Vec<(u32, u32)> {
    plan.renumbered
        .iter()
        .map(|chapter| (chapter.from.number(), chapter.to.number()))
        .collect()
}

fn kinds(plan: &StructurePlan) -> Vec<&'static str> {
    plan.change_set
        .files
        .iter()
        .map(|change| match change {
            FileChange::Write { .. } => "write",
            FileChange::Trash { .. } => "trash",
            FileChange::Move { .. } => "move",
            FileChange::Expect { .. } => "expect",
        })
        .collect()
}

fn trash_batch_dir(dir: &TempDir) -> std::path::PathBuf {
    std::fs::read_dir(dir.path().join(".kataribe/trash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

// ---- 章を足す ----

#[test]
fn adding_the_first_chapter_to_an_empty_project_makes_chapter_01() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let plan = plan_and_apply(&project, &add_chapter(None, "雨の匂い"));

    assert_eq!(chapter_numbers(&project), vec!["01"]);
    let chapter = project.chapter(&chapter_id(1)).unwrap().unwrap();
    assert_eq!(chapter.meta.title, "雨の匂い");
    assert_eq!(chapter.meta.scenes, Vec::new());
    assert_eq!(chapter.storyline, "新しい章のあらすじ\n");
    assert_eq!(plan.created, Some(rel("plot/chapters/01.md")));
    assert_eq!(plan.renumbered, Vec::new());
    assert!(!dir.path().join("manuscript/01").exists());
}

#[test]
fn adding_at_the_end_renames_nothing_and_uses_the_next_number_after_the_largest() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");

    let plan = plan_and_apply(&project, &add_chapter(None, "三章"));

    assert_eq!(chapter_numbers(&project), vec!["01", "02", "03"]);
    assert_eq!(chapter_titles(&project), vec!["一章", "二章", "三章"]);
    assert_eq!(kinds(&plan), vec!["expect", "write"]);
    assert_eq!(plan.renumbered, Vec::new());
    assert_eq!(
        read(&project, "manuscript/02/s01.txt").unwrap(),
        "二章の本文\n"
    );
}

#[test]
fn adding_before_the_first_chapter_shifts_every_chapter_and_its_text_folder_back_by_one() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");

    let plan = plan_and_apply(&project, &add_chapter(Some(1), "序章"));

    assert_eq!(chapter_numbers(&project), vec!["01", "02", "03"]);
    assert_eq!(chapter_titles(&project), vec!["序章", "一章", "二章"]);
    assert_eq!(plan.created, Some(rel("plot/chapters/01.md")));
    assert_eq!(renumbering(&plan), vec![(1, 2), (2, 3)]);
    assert_eq!(
        read(&project, "manuscript/02/s01.txt").unwrap(),
        "一章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/03/s01.txt").unwrap(),
        "二章の本文\n"
    );
    assert!(
        !dir.path().join("manuscript/01").exists(),
        "新しい章には本文のフォルダがまだ無い"
    );
}

#[test]
fn adding_in_the_middle_shifts_only_the_chapters_from_that_position() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    put_chapter_with_text(&project, "03", "三章");

    let plan = plan_and_apply(&project, &add_chapter(Some(2), "間の章"));

    assert_eq!(
        chapter_titles(&project),
        vec!["一章", "間の章", "二章", "三章"]
    );
    assert_eq!(renumbering(&plan), vec![(2, 3), (3, 4)]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "一章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/03/s01.txt").unwrap(),
        "二章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/04/s01.txt").unwrap(),
        "三章の本文\n"
    );
}

#[test]
fn renumbered_chapters_carry_their_titles_and_a_chapter_with_broken_yaml_only_its_number() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put(
        &project,
        "plot/chapters/02.md",
        "---\ntitle: [壊れた\n---\n",
    );

    let plan = plan_and_apply(&project, &add_chapter(Some(1), "序章"));

    assert_eq!(
        plan.renumbered,
        vec![
            RenumberedChapter {
                from: chapter_id(1),
                to: chapter_id(2),
                title: Some("一章".to_owned()),
            },
            RenumberedChapter {
                from: chapter_id(2),
                to: chapter_id(3),
                title: None,
            },
        ]
    );
    assert_eq!(
        read(&project, "plot/chapters/03.md").unwrap(),
        "---\ntitle: [壊れた\n---\n",
        "壊れた章も、中身はそのまま移る"
    );
}

#[test]
fn a_gap_in_the_numbers_stays_and_only_chapters_after_the_position_move() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "03", "三章");
    put_chapter_with_text(&project, "05", "五章");

    let plan = plan_and_apply(&project, &add_chapter(Some(3), "新章"));

    assert_eq!(chapter_numbers(&project), vec!["01", "03", "04", "06"]);
    assert_eq!(
        chapter_titles(&project),
        vec!["一章", "新章", "三章", "五章"]
    );
    assert_eq!(renumbering(&plan), vec![(3, 4), (5, 6)]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "一章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/04/s01.txt").unwrap(),
        "三章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/06/s01.txt").unwrap(),
        "五章の本文\n"
    );
}

#[test]
fn adding_at_the_end_after_a_gap_continues_from_the_largest_number() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "03", "三章");

    plan_and_apply(&project, &add_chapter(None, "新章"));

    assert_eq!(chapter_numbers(&project), vec!["01", "03", "04"]);
}

#[test]
fn a_chapter_without_a_text_folder_makes_a_rename_of_its_plan_only() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/01.md",
        &chapter_md("一章", "あらすじ"),
    );
    put_chapter_with_text(&project, "02", "二章");

    let plan = plan_structure_edit(&project, &add_chapter(Some(1), "序章")).unwrap();

    let moves: Vec<(String, String)> = plan
        .change_set
        .files
        .iter()
        .filter_map(|change| match change {
            FileChange::Move { from, to } => Some((from.to_string(), to.to_string())),
            _ => None,
        })
        .collect();
    assert_eq!(
        moves,
        vec![
            (
                "plot/chapters/01.md".to_owned(),
                "plot/chapters/02.md".to_owned()
            ),
            (
                "plot/chapters/02.md".to_owned(),
                "plot/chapters/03.md".to_owned()
            ),
            ("manuscript/02".to_owned(), "manuscript/03".to_owned()),
        ],
        "本文のフォルダの無い第 1 章は、フォルダの改名もゴミ箱への移動も作らない"
    );
    assert!(
        plan.change_set.files.iter().any(|change| matches!(
            change,
            FileChange::Expect { path, base_hash: None } if *path == rel("manuscript/01")
        )),
        "適用のときに、本文のフォルダがまだ無いことを確かめる"
    );

    plan.change_set.apply(&project).unwrap();

    assert_eq!(chapter_titles(&project), vec!["序章", "一章", "二章"]);
    assert!(!dir.path().join("manuscript/01").exists());
    assert!(!dir.path().join("manuscript/02").exists());
    assert_eq!(
        read(&project, "manuscript/03/s01.txt").unwrap(),
        "二章の本文\n"
    );
}

#[test]
fn a_text_folder_made_by_someone_else_after_the_plan_stops_the_renumbering() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/01.md",
        &chapter_md("一章", "あらすじ"),
    );
    let plan = plan_structure_edit(&project, &add_chapter(Some(1), "序章")).unwrap();
    // 計画してから適用するまでの間に、外のエディタで第 1 章の本文ができた
    put(&project, "manuscript/01/s01.txt", "あとから書かれた本文\n");

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(chapter_numbers(&project), vec!["01"]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "あとから書かれた本文\n"
    );
}

#[test]
fn a_text_folder_without_a_chapter_at_a_destination_is_refused_with_a_clear_message() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    put(&project, "manuscript/03/s01.txt", "章立ての無い本文\n");

    let message =
        invalid_input_message(plan_structure_edit(&project, &add_chapter(Some(2), "新章")));

    assert!(
        message.contains("manuscript/03 が既にあるため"),
        "{message}"
    );
    assert!(
        plan_structure_edit(&project, &add_chapter(None, "末尾")).is_err(),
        "末尾に足す章の本文のフォルダが残っているときも同じ"
    );
}

#[test]
fn a_leftover_folder_blocks_the_shift_even_when_the_shifted_chapter_has_no_text_folder() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/01.md",
        &chapter_md("一章", "あらすじ"),
    );
    put(&project, "manuscript/02/s01.txt", "章立ての無い本文\n");

    let message =
        invalid_input_message(plan_structure_edit(&project, &add_chapter(Some(1), "序章")));

    assert!(
        message.contains("manuscript/02 が既にあるため"),
        "{message}"
    );
    assert_eq!(
        read(&project, "manuscript/02/s01.txt").unwrap(),
        "章立ての無い本文\n"
    );
}

#[test]
fn a_chapter_added_by_someone_else_after_the_plan_conflicts_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    let plan = plan_structure_edit(&project, &add_chapter(None, "二章")).unwrap();
    // 計画してから適用するまでの間に、外で第 2 章ができた
    put(
        &project,
        "plot/chapters/02.md",
        &chapter_md("外で足された章", ""),
    );

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(chapter_titles(&project), vec!["一章", "外で足された章"]);
}

#[test]
fn a_chapter_renumbered_by_someone_else_after_the_plan_conflicts_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    let plan = plan_structure_edit(&project, &add_chapter(Some(1), "序章")).unwrap();
    // 外で第 3 章ができ、計画した改名の行き先が塞がった
    put(
        &project,
        "plot/chapters/03.md",
        &chapter_md("外で足された章", ""),
    );

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(
        chapter_titles(&project),
        vec!["一章", "二章", "外で足された章"]
    );
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "一章の本文\n"
    );
}

#[test]
fn hand_made_three_digit_numbers_become_two_digits_only_for_the_shifted_chapters() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "001", "一章");
    put_chapter_with_text(&project, "002", "二章");

    plan_and_apply(
        &project,
        &StructureEdit::AddChapter {
            before: Some(ChapterId::new("001").unwrap()),
            title: "序章".to_owned(),
            storyline: String::new(),
        },
    );

    assert_eq!(chapter_numbers(&project), vec!["001", "02", "03"]);
    assert_eq!(chapter_titles(&project), vec!["序章", "一章", "二章"]);
}

#[test]
fn chapters_cannot_go_beyond_999() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/998.md",
        &chapter_md("後ろの章", ""),
    );
    put(
        &project,
        "plot/chapters/999.md",
        &chapter_md("最後の章", ""),
    );

    let at_the_end = invalid_input_message(plan_structure_edit(
        &project,
        &add_chapter(None, "さらに後ろ"),
    ));
    let before_the_last = invalid_input_message(plan_structure_edit(
        &project,
        &add_chapter(Some(998), "途中"),
    ));

    assert!(at_the_end.contains("999"), "{at_the_end}");
    assert!(before_the_last.contains("999"), "{before_the_last}");
    assert_eq!(chapter_numbers(&project), vec!["998", "999"]);
}

#[test]
fn adding_before_a_chapter_that_does_not_exist_is_not_found() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");

    let result = plan_structure_edit(&project, &add_chapter(Some(5), "新章"));

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "{result:?}"
    );
}

#[test]
fn adding_a_chapter_needs_a_one_line_title_and_trims_the_input() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let empty = plan_structure_edit(&project, &add_chapter(None, "   "));
    let two_lines = plan_structure_edit(&project, &add_chapter(None, "一行目\n二行目"));
    assert!(
        matches!(empty, Err(EngineError::InvalidInput(_))),
        "{empty:?}"
    );
    assert!(
        matches!(two_lines, Err(EngineError::InvalidInput(_))),
        "{two_lines:?}"
    );

    plan_and_apply(
        &project,
        &StructureEdit::AddChapter {
            before: None,
            title: "  雨の匂い  ".to_owned(),
            storyline: "\n  あらすじ  \n\n".to_owned(),
        },
    );
    let chapter = project.chapter(&chapter_id(1)).unwrap().unwrap();
    assert_eq!(chapter.meta.title, "雨の匂い");
    assert_eq!(chapter.storyline, "あらすじ\n");
}

#[test]
fn a_chapter_with_a_colon_in_its_title_is_written_as_valid_yaml() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    plan_and_apply(&project, &add_chapter(None, "第一部: 雨 #1"));

    assert_eq!(chapter_titles(&project), vec!["第一部: 雨 #1"]);
}

#[test]
fn adding_a_chapter_says_what_it_does_before_and_after() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");

    let plan = plan_structure_edit(&project, &add_chapter(None, "雨の匂い")).unwrap();

    assert_eq!(plan.change_set.summary, "第2章「雨の匂い」を追加します。");
    assert_eq!(plan.completed_summary, "第2章「雨の匂い」を追加しました。");
}

#[test]
fn a_new_chapter_shows_up_in_the_overview_and_waits_for_its_scene_plan() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");

    plan_and_apply(&project, &add_chapter(Some(1), "序章"));

    let plot = section_entries(&project, SectionKind::Plot);
    let chapter_entries: Vec<&OverviewEntry> = plot
        .iter()
        .filter(|entry| entry.kind == EntryKind::Chapter)
        .collect();
    assert_eq!(chapter_entries.len(), 2);
    assert_eq!(
        chapter_entries[0].path.as_deref(),
        Some("plot/chapters/01.md")
    );
    assert_eq!(chapter_entries[0].label, "第1章「序章」");
    assert_eq!(
        chapter_entries[1].path.as_deref(),
        Some("plot/chapters/02.md")
    );
    let manuscript = section_entries(&project, SectionKind::Manuscript);
    assert_eq!(
        manuscript
            .iter()
            .filter_map(|entry| entry.chapter.map(|chapter| chapter.number()))
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let steps = pipeline(&project, DraftUnit::Beat).unwrap();
    let new_chapter_scene_plan = steps
        .iter()
        .find(|step| {
            step.task
                == Task::ScenePlan {
                    chapter: chapter_id(1),
                }
        })
        .unwrap();
    assert_eq!(new_chapter_scene_plan.state, StepState::Ready);
}

// ---- 章を消す ----

#[test]
fn removing_a_middle_chapter_trashes_its_plan_and_text_and_closes_the_gap_behind_it() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    put_chapter_with_text(&project, "03", "三章");
    put_chapter_with_text(&project, "04", "四章");

    let plan = plan_and_apply(&project, &remove_chapter(2));

    assert_eq!(chapter_numbers(&project), vec!["01", "02", "03"]);
    assert_eq!(chapter_titles(&project), vec!["一章", "三章", "四章"]);
    assert_eq!(renumbering(&plan), vec![(3, 2), (4, 3)]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "一章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/02/s01.txt").unwrap(),
        "三章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/03/s01.txt").unwrap(),
        "四章の本文\n"
    );
    assert!(!dir.path().join("manuscript/04").exists());
    let trash = trash_batch_dir(&dir);
    assert_eq!(
        std::fs::read_to_string(trash.join("plot/chapters/02.md")).unwrap(),
        chapter_md("二章", "二章のあらすじ")
    );
    assert_eq!(
        std::fs::read_to_string(trash.join("manuscript/02/s01.txt")).unwrap(),
        "二章の本文\n"
    );
    assert_eq!(plan.created, None);
}

#[test]
fn the_trash_of_a_removed_chapter_lists_every_file_of_the_text_folder_with_its_size() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put(&project, "manuscript/01/s02.txt", "二つ目の本文です\n");
    put(&project, "manuscript/01/notes/memo.txt", "メモ\n");

    let plan = plan_structure_edit(&project, &remove_chapter(1)).unwrap();

    let folder = plan
        .change_set
        .files
        .iter()
        .find_map(|change| match change {
            FileChange::Trash { path, files } if *path == rel("manuscript/01") => Some(files),
            _ => None,
        })
        .unwrap();
    let listed: Vec<(String, usize)> = folder
        .iter()
        .map(|file| (file.path.to_string(), file.chars))
        .collect();
    assert_eq!(
        listed,
        vec![
            ("manuscript/01/notes/memo.txt".to_owned(), 2),
            ("manuscript/01/s01.txt".to_owned(), 5),
            ("manuscript/01/s02.txt".to_owned(), 8),
        ]
    );
    assert!(folder.iter().all(|file| file.base_hash.is_some()));
}

#[test]
fn removing_the_first_and_the_last_chapter() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    put_chapter_with_text(&project, "03", "三章");

    let last = plan_and_apply(&project, &remove_chapter(3));
    assert_eq!(chapter_numbers(&project), vec!["01", "02"]);
    assert_eq!(last.renumbered, Vec::new());

    let first = plan_and_apply(&project, &remove_chapter(1));
    assert_eq!(chapter_numbers(&project), vec!["01"]);
    assert_eq!(chapter_titles(&project), vec!["二章"]);
    assert_eq!(renumbering(&first), vec![(2, 1)]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "二章の本文\n"
    );
}

#[test]
fn removing_a_chapter_keeps_the_gaps_in_the_numbers_before_it_and_shifts_only_the_chapters_after_it()
 {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "03", "三章");
    put_chapter_with_text(&project, "05", "五章");

    let plan = plan_and_apply(&project, &remove_chapter(3));

    assert_eq!(chapter_numbers(&project), vec!["01", "04"]);
    assert_eq!(renumbering(&plan), vec![(5, 4)]);
    assert_eq!(
        read(&project, "manuscript/04/s01.txt").unwrap(),
        "五章の本文\n"
    );
}

#[test]
fn removing_a_chapter_without_a_text_folder_trashes_only_the_plan() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/01.md",
        &chapter_md("一章", "あらすじ"),
    );
    put_chapter_with_text(&project, "02", "二章");

    let plan = plan_structure_edit(&project, &remove_chapter(1)).unwrap();

    assert_eq!(kinds(&plan), vec!["trash", "expect", "move", "move"]);
    plan.change_set.apply(&project).unwrap();
    assert_eq!(chapter_titles(&project), vec!["二章"]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "二章の本文\n"
    );
}

#[test]
fn a_file_added_to_the_text_folder_after_the_plan_stops_the_removal() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    let plan = plan_structure_edit(&project, &remove_chapter(1)).unwrap();
    // 確認している間に、外のエディタで第 1 章に本文が足された。確かめていない本文を黙ってゴミ箱へ移さない
    put(&project, "manuscript/01/s02.txt", "あとから足された本文\n");

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(chapter_numbers(&project), vec!["01", "02"]);
    assert_eq!(
        read(&project, "manuscript/01/s02.txt").unwrap(),
        "あとから足された本文\n"
    );
    assert!(!dir.path().join(".kataribe/trash").exists());
}

#[test]
fn a_text_changed_after_the_plan_stops_the_removal_too() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    let plan = plan_structure_edit(&project, &remove_chapter(1)).unwrap();
    put(&project, "manuscript/01/s01.txt", "書き足した本文\n");

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(chapter_numbers(&project), vec!["01"]);
}

#[test]
fn removing_a_chapter_whose_text_folder_has_a_file_that_is_not_text_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    std::fs::write(
        dir.path().join("manuscript/01/cover.bin"),
        [0xff, 0xfe, 0x00],
    )
    .unwrap();

    let message = invalid_input_message(plan_structure_edit(&project, &remove_chapter(1)));

    assert!(message.contains("manuscript/01/cover.bin"), "{message}");
    assert_eq!(chapter_numbers(&project), vec!["01"]);
}

#[test]
fn removing_a_chapter_that_does_not_exist_is_not_found() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let result = plan_structure_edit(&project, &remove_chapter(3));

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "{result:?}"
    );
}

#[test]
fn removing_a_chapter_says_its_title_or_only_its_number_when_the_plan_is_unreadable() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "雨の匂い");
    put(
        &project,
        "plot/chapters/02.md",
        "---\ntitle: [壊れた\n---\n",
    );

    let readable = plan_structure_edit(&project, &remove_chapter(1)).unwrap();
    let broken = plan_structure_edit(&project, &remove_chapter(2)).unwrap();

    assert_eq!(
        readable.change_set.summary,
        "第1章「雨の匂い」をゴミ箱へ移します。"
    );
    assert_eq!(
        readable.completed_summary,
        "第1章「雨の匂い」をゴミ箱へ移しました。"
    );
    assert_eq!(broken.change_set.summary, "第2章をゴミ箱へ移します。");
}

#[test]
fn a_leftover_folder_where_a_later_chapter_would_move_refuses_the_removal() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    put_chapter_with_text(&project, "04", "四章");
    put(&project, "manuscript/03/s01.txt", "章立ての無い本文\n");

    let message = invalid_input_message(plan_structure_edit(&project, &remove_chapter(2)));

    assert!(
        message.contains("manuscript/03 が既にあるため"),
        "{message}"
    );
}

#[test]
fn removing_a_chapter_updates_the_overview_and_the_pipeline() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");

    plan_and_apply(&project, &remove_chapter(1));

    let plot = section_entries(&project, SectionKind::Plot);
    let paths: Vec<Option<&str>> = plot
        .iter()
        .filter(|entry| entry.kind == EntryKind::Chapter)
        .map(|entry| entry.path.as_deref())
        .collect();
    assert_eq!(paths, vec![Some("plot/chapters/01.md")]);
    let steps = pipeline(&project, DraftUnit::Beat).unwrap();
    assert!(
        steps.iter().all(|step| step.task
            != Task::ScenePlan {
                chapter: chapter_id(2)
            }),
        "存在しない第 2 章の工程が残っている"
    );
}

#[test]
fn removing_a_chapter_and_adding_one_back_gives_the_same_numbers() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    put_chapter_with_text(&project, "03", "三章");

    plan_and_apply(&project, &remove_chapter(2));
    plan_and_apply(&project, &add_chapter(Some(2), "新しい二章"));

    assert_eq!(chapter_numbers(&project), vec!["01", "02", "03"]);
    assert_eq!(chapter_titles(&project), vec!["一章", "新しい二章", "三章"]);
    assert_eq!(
        read(&project, "manuscript/03/s01.txt").unwrap(),
        "三章の本文\n"
    );
}
