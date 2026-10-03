//! 人物・章・シーンの並べ替えを、作品フォルダに対して確かめる。
#![allow(clippy::unwrap_used)]

mod common;

use kataribe_engine::{
    DraftUnit, EngineError, EntryKind, FileChange, OverviewEntry, RenumberedChapter, SectionKind,
    StepState, StructureEdit, StructurePlan, Task, overview, pipeline, plan_structure_edit,
};
use kataribe_project::{ChapterId, Project, ProjectError, RelPath, SceneId};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use common::{new_project, put, read};

fn rel(path: &str) -> RelPath {
    RelPath::new(path).unwrap()
}

fn chapter_id(number: u32) -> ChapterId {
    ChapterId::from_number(number)
}

fn plan_and_apply(project: &Project, edit: &StructureEdit) -> StructurePlan {
    let plan = plan_structure_edit(project, edit).unwrap();
    plan.change_set.apply(project).unwrap();
    plan
}

fn invalid_input_message(result: Result<StructurePlan, EngineError>) -> String {
    match result {
        Err(EngineError::InvalidInput(message)) => message,
        other => panic!("入力の誤りになるはず: {other:?}"),
    }
}

fn assert_conflict(error: &EngineError) {
    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
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

// ---- 人物 ----

fn character_md(name: &str, order: Option<u32>) -> String {
    let order_line = order.map_or_else(String::new, |order| format!("order: {order}\n"));
    format!("---\nname: {name}\nrole: 役回り\nsummary: 一言紹介\n{order_line}---\n{name}の本文\n")
}

fn put_character(project: &Project, id: &str, name: &str, order: Option<u32>) {
    put(
        project,
        &format!("characters/{id}.md"),
        &character_md(name, order),
    );
}

fn move_character(id: &str, position: usize) -> StructureEdit {
    StructureEdit::MoveCharacter {
        path: rel(&format!("characters/{id}.md")),
        position,
    }
}

/// 目次の「登場人物」の並び（表示名）。
fn character_labels(project: &Project) -> Vec<String> {
    section_entries(project, SectionKind::Characters)
        .into_iter()
        .map(|entry| entry.label)
        .collect()
}

fn character_order(project: &Project, id: &str) -> Option<u32> {
    project
        .character(&kataribe_project::CharacterId::new(id).unwrap())
        .unwrap()
        .unwrap()
        .meta
        .order
}

fn written_paths(plan: &StructurePlan) -> Vec<String> {
    plan.change_set
        .files
        .iter()
        .filter_map(|change| match change {
            FileChange::Write { path, .. } => Some(path.to_string()),
            _ => None,
        })
        .collect()
}

/// 凛・健二・舞・翔の 4 人が order 1〜4 で並んだ作品。
fn project_with_four_characters(dir: &TempDir) -> Project {
    let project = new_project(dir.path());
    for (id, name, order) in [
        ("rin", "凛", 1),
        ("kenji", "健二", 2),
        ("mai", "舞", 3),
        ("sho", "翔", 4),
    ] {
        put_character(&project, id, name, Some(order));
    }
    project
}

#[test]
fn moving_a_character_up_swaps_it_with_its_neighbour() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);

    plan_and_apply(&project, &move_character("mai", 1));

    assert_eq!(character_labels(&project), vec!["凛", "舞", "健二", "翔"]);
    assert_eq!(character_order(&project, "mai"), Some(2));
    assert_eq!(character_order(&project, "kenji"), Some(3));
}

#[test]
fn moving_a_character_to_the_first_and_to_the_last_position() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);

    plan_and_apply(&project, &move_character("sho", 0));
    assert_eq!(character_labels(&project), vec!["翔", "凛", "健二", "舞"]);

    plan_and_apply(&project, &move_character("sho", 3));
    assert_eq!(character_labels(&project), vec!["凛", "健二", "舞", "翔"]);
    assert_eq!(character_order(&project, "sho"), Some(4));
}

#[test]
fn every_move_lands_the_character_at_the_requested_position_in_the_overview() {
    let names = ["凛", "健二", "舞", "翔"];
    let ids = ["rin", "kenji", "mai", "sho"];
    for (from, id) in ids.iter().enumerate() {
        for to in (0..names.len()).filter(|to| *to != from) {
            let dir = TempDir::new().unwrap();
            let project = project_with_four_characters(&dir);
            let mut expected: Vec<&str> = names.to_vec();
            let moved = expected.remove(from);
            expected.insert(to, moved);

            plan_and_apply(&project, &move_character(id, to));

            assert_eq!(
                character_labels(&project),
                expected,
                "{from} 番目を {to} 番目へ"
            );
        }
    }
}

#[test]
fn only_the_characters_whose_order_changes_are_rewritten() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);
    let untouched = read(&project, "characters/mai.md").unwrap();

    let plan = plan_and_apply(&project, &move_character("kenji", 0));

    let mut written = written_paths(&plan);
    written.sort();
    assert_eq!(written, vec!["characters/kenji.md", "characters/rin.md"]);
    assert_eq!(read(&project, "characters/mai.md").unwrap(), untouched);
    assert_eq!(
        read(&project, "characters/sho.md").unwrap(),
        character_md("翔", Some(4))
    );
}

#[test]
fn orders_with_gaps_are_renumbered_from_one() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_character(&project, "rin", "凛", Some(10));
    put_character(&project, "kenji", "健二", Some(20));
    put_character(&project, "mai", "舞", Some(30));

    plan_and_apply(&project, &move_character("mai", 0));

    assert_eq!(character_labels(&project), vec!["舞", "凛", "健二"]);
    assert_eq!(character_order(&project, "mai"), Some(1));
    assert_eq!(character_order(&project, "rin"), Some(2));
    assert_eq!(character_order(&project, "kenji"), Some(3));
}

#[test]
fn characters_without_an_order_come_last_and_get_a_number_when_the_list_is_rearranged() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_character(&project, "rin", "凛", Some(1));
    put_character(&project, "kenji", "健二", None);
    put_character(&project, "mai", "舞", None);
    assert_eq!(character_labels(&project), vec!["凛", "健二", "舞"]);

    plan_and_apply(&project, &move_character("mai", 0));

    assert_eq!(character_labels(&project), vec!["舞", "凛", "健二"]);
    assert_eq!(character_order(&project, "kenji"), Some(3));
}

#[test]
fn rewriting_a_character_keeps_its_body_and_the_fields_the_app_does_not_know() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "characters/rin.md",
        "---\nname: 凛\nrole: 探偵\nsummary: 盲目\norder: 1\nnickname: りんりん\n---\n## 外見\n黒髪。\n\n## 口調\n丁寧。\n",
    );
    put_character(&project, "kenji", "健二", Some(2));

    plan_and_apply(&project, &move_character("rin", 1));

    let rewritten = read(&project, "characters/rin.md").unwrap();
    assert!(rewritten.contains("order: 2"), "{rewritten}");
    assert!(rewritten.contains("nickname: りんりん"), "{rewritten}");
    assert!(
        rewritten.ends_with("---\n## 外見\n黒髪。\n\n## 口調\n丁寧。\n"),
        "{rewritten}"
    );
    let character = project
        .character(&kataribe_project::CharacterId::new("rin").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(character.meta.name, "凛");
    assert_eq!(character.meta.role, "探偵");
}

#[test]
fn a_character_with_broken_yaml_cannot_be_moved() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);
    put(
        &project,
        "characters/broken.md",
        "---\nname: [壊れた\n---\n",
    );

    let message =
        invalid_input_message(plan_structure_edit(&project, &move_character("broken", 0)));

    assert!(message.contains("characters/broken.md"), "{message}");
    assert!(message.contains("読めない"), "{message}");
}

#[test]
fn broken_characters_keep_their_file_and_stay_at_the_end_while_the_others_are_rearranged() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_character(&project, "rin", "凛", Some(1));
    put_character(&project, "kenji", "健二", Some(2));
    put(
        &project,
        "characters/broken.md",
        "---\nname: [壊れた\n---\n",
    );

    let plan = plan_and_apply(&project, &move_character("kenji", 0));

    assert_eq!(
        read(&project, "characters/broken.md").unwrap(),
        "---\nname: [壊れた\n---\n"
    );
    assert_eq!(
        character_labels(&project),
        vec!["健二", "凛", "broken"],
        "読めない資料は目次の最後に残る"
    );
    assert_eq!(plan.notices.len(), 1);
    assert!(
        plan.notices[0].contains("characters/broken.md"),
        "{:?}",
        plan.notices
    );
}

#[test]
fn moving_to_a_slot_taken_by_a_broken_character_puts_the_character_last_among_the_readable_ones() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_character(&project, "rin", "凛", Some(1));
    put_character(&project, "kenji", "健二", Some(2));
    put(
        &project,
        "characters/broken.md",
        "---\nname: [壊れた\n---\n",
    );

    plan_and_apply(&project, &move_character("rin", 2));

    assert_eq!(character_labels(&project), vec!["健二", "凛", "broken"]);
}

#[test]
fn a_move_that_changes_nothing_because_of_a_broken_character_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_character(&project, "rin", "凛", Some(1));
    put_character(&project, "kenji", "健二", Some(2));
    put(
        &project,
        "characters/broken.md",
        "---\nname: [壊れた\n---\n",
    );

    let message = invalid_input_message(plan_structure_edit(&project, &move_character("kenji", 2)));

    assert!(message.contains("順番は変わりません"), "{message}");
}

#[test]
fn a_file_name_that_breaks_the_id_rules_counts_as_unreadable_like_in_the_overview() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_character(&project, "rin", "凛", Some(1));
    put_character(&project, "kenji", "健二", Some(2));
    put(&project, "characters/Mai.md", &character_md("舞", Some(3)));

    let moving_it = invalid_input_message(plan_structure_edit(
        &project,
        &StructureEdit::MoveCharacter {
            path: rel("characters/Mai.md"),
            position: 0,
        },
    ));
    plan_and_apply(&project, &move_character("kenji", 0));

    assert!(moving_it.contains("characters/Mai.md"), "{moving_it}");
    assert_eq!(character_labels(&project), vec!["健二", "凛", "Mai"]);
    assert_eq!(
        read(&project, "characters/Mai.md").unwrap(),
        character_md("舞", Some(3))
    );
}

#[test]
fn a_character_position_outside_the_list_or_the_current_one_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);

    let outside = invalid_input_message(plan_structure_edit(&project, &move_character("rin", 4)));
    let same = invalid_input_message(plan_structure_edit(&project, &move_character("mai", 2)));

    assert!(outside.contains("5 番目"), "{outside}");
    assert!(outside.contains("4 件"), "{outside}");
    assert!(same.contains("すでに 3 番目"), "{same}");
    assert_eq!(
        character_labels(&project),
        vec!["凛", "健二", "舞", "翔"],
        "断った操作は何も変えない"
    );
}

#[test]
fn only_a_document_directly_under_characters_can_be_moved() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);

    for path in ["world/overview.md", "characters/old/rin.md", "concept.md"] {
        let result = plan_structure_edit(
            &project,
            &StructureEdit::MoveCharacter {
                path: rel(path),
                position: 0,
            },
        );
        let message = invalid_input_message(result);
        assert!(message.contains("並べ替えられる"), "{path}: {message}");
    }
    let missing = plan_structure_edit(&project, &move_character("nobody", 0));
    assert!(
        matches!(missing, Err(EngineError::NotFound(_))),
        "{missing:?}"
    );
}

#[test]
fn a_character_edited_after_the_plan_conflicts_and_nothing_is_rewritten() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);
    let plan = plan_structure_edit(&project, &move_character("sho", 0)).unwrap();
    // 計画してから適用するまでの間に、外のエディタで人物資料が書き換わった
    put(
        &project,
        "characters/mai.md",
        &character_md("舞（改稿）", Some(3)),
    );

    let error = plan.change_set.apply(&project).unwrap_err();

    assert_conflict(&error);
    assert_eq!(character_order(&project, "sho"), Some(4));
    assert_eq!(character_order(&project, "rin"), Some(1));
    assert_eq!(
        read(&project, "characters/mai.md").unwrap(),
        character_md("舞（改稿）", Some(3))
    );
}

#[test]
fn moving_a_character_says_what_it_does_before_and_after() {
    let dir = TempDir::new().unwrap();
    let project = project_with_four_characters(&dir);

    let plan = plan_structure_edit(&project, &move_character("mai", 0)).unwrap();

    assert_eq!(plan.change_set.summary, "人物「舞」を 1 番目に移します。");
    assert_eq!(plan.completed_summary, "人物「舞」を 1 番目に移しました。");
    assert_eq!(plan.created, None);
    assert_eq!(plan.renumbered, Vec::new());
}

// ---- 章 ----

fn chapter_md(title: &str) -> String {
    format!("---\ntitle: {title}\n---\n{title}のあらすじ\n")
}

/// 章立てと、本文を 1 つ持つ章。本文は `<title>の本文` になる。
fn put_chapter_with_text(project: &Project, number: &str, title: &str) {
    put(
        project,
        &format!("plot/chapters/{number}.md"),
        &chapter_md(title),
    );
    put(
        project,
        &format!("manuscript/{number}/s01.txt"),
        &format!("{title}の本文\n"),
    );
}

fn move_chapter(number: u32, position: usize) -> StructureEdit {
    StructureEdit::MoveChapter {
        chapter: chapter_id(number),
        position,
    }
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

fn project_with_chapters(dir: &TempDir, titles: &[&str]) -> Project {
    let project = new_project(dir.path());
    for (index, title) in titles.iter().enumerate() {
        put_chapter_with_text(&project, &format!("{:02}", index + 1), title);
    }
    project
}

#[test]
fn swapping_two_chapters_exchanges_their_plans_and_text_folders() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章"]);

    let plan = plan_and_apply(&project, &move_chapter(2, 0));

    assert_eq!(chapter_numbers(&project), vec!["01", "02"]);
    assert_eq!(chapter_titles(&project), vec!["二章", "一章"]);
    assert_eq!(renumbering(&plan), vec![(1, 2), (2, 1)]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "二章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/02/s01.txt").unwrap(),
        "一章の本文\n"
    );
    assert!(
        !dir.path().join(".kataribe/trash").exists(),
        "入れ替えで何もゴミ箱へ移らない"
    );
}

#[test]
fn moving_the_last_chapter_to_the_front_rotates_every_chapter_and_its_text() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章", "三章"]);

    let plan = plan_and_apply(&project, &move_chapter(3, 0));

    assert_eq!(chapter_titles(&project), vec!["三章", "一章", "二章"]);
    assert_eq!(renumbering(&plan), vec![(1, 2), (2, 3), (3, 1)]);
    for (number, text) in [("01", "三章"), ("02", "一章"), ("03", "二章")] {
        assert_eq!(
            read(&project, &format!("manuscript/{number}/s01.txt")).unwrap(),
            format!("{text}の本文\n")
        );
    }
}

#[test]
fn moving_the_first_chapter_to_the_end_rotates_the_other_way() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章", "三章"]);

    let plan = plan_and_apply(&project, &move_chapter(1, 2));

    assert_eq!(chapter_titles(&project), vec!["二章", "三章", "一章"]);
    assert_eq!(renumbering(&plan), vec![(1, 3), (2, 1), (3, 2)]);
    assert_eq!(
        read(&project, "manuscript/03/s01.txt").unwrap(),
        "一章の本文\n"
    );
}

#[test]
fn chapters_outside_the_moved_range_keep_their_files() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章", "三章", "四章"]);

    let plan = plan_and_apply(&project, &move_chapter(3, 1));

    assert_eq!(
        chapter_titles(&project),
        vec!["一章", "三章", "二章", "四章"]
    );
    assert_eq!(renumbering(&plan), vec![(2, 3), (3, 2)]);
    let moved: Vec<String> = plan
        .change_set
        .files
        .iter()
        .filter_map(|change| match change {
            FileChange::Move { from, .. } => Some(from.to_string()),
            _ => None,
        })
        .collect();
    assert!(
        moved
            .iter()
            .all(|from| !from.contains("01") && !from.contains("04")),
        "{moved:?}"
    );
}

#[test]
fn the_numbers_in_the_range_are_reassigned_as_they_are_so_gaps_stay() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "02", "二章");
    put_chapter_with_text(&project, "05", "五章");

    let plan = plan_and_apply(&project, &move_chapter(5, 0));

    assert_eq!(chapter_numbers(&project), vec!["01", "02", "05"]);
    assert_eq!(chapter_titles(&project), vec!["五章", "一章", "二章"]);
    assert_eq!(renumbering(&plan), vec![(1, 2), (2, 5), (5, 1)]);
    assert_eq!(
        read(&project, "manuscript/05/s01.txt").unwrap(),
        "二章の本文\n"
    );
}

#[test]
fn a_hand_made_three_digit_number_keeps_its_width_when_chapters_swap() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "001", "一章");
    put_chapter_with_text(&project, "02", "二章");

    plan_and_apply(&project, &move_chapter(2, 0));

    assert_eq!(chapter_numbers(&project), vec!["001", "02"]);
    assert_eq!(chapter_titles(&project), vec!["二章", "一章"]);
    assert_eq!(
        read(&project, "manuscript/001/s01.txt").unwrap(),
        "二章の本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/02/s01.txt").unwrap(),
        "一章の本文\n"
    );
}

#[test]
fn a_chapter_without_a_text_folder_swaps_with_one_that_has_text() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", &chapter_md("一章"));
    put_chapter_with_text(&project, "02", "二章");

    let plan = plan_structure_edit(&project, &move_chapter(2, 0)).unwrap();
    assert_eq!(
        kinds(&plan).iter().filter(|kind| **kind == "move").count(),
        3,
        "章立て 2 つと、本文のある第 2 章のフォルダだけを移す"
    );
    plan.change_set.apply(&project).unwrap();

    assert_eq!(chapter_titles(&project), vec!["二章", "一章"]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "二章の本文\n"
    );
    assert!(!dir.path().join("manuscript/02").exists());
}

#[test]
fn consecutive_chapters_without_text_do_not_repeat_the_same_expectation() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    for number in ["01", "02", "03"] {
        put(
            &project,
            &format!("plot/chapters/{number}.md"),
            &chapter_md(number),
        );
    }

    let plan = plan_and_apply(&project, &move_chapter(3, 0));

    let mut expected_paths: Vec<String> = plan
        .change_set
        .files
        .iter()
        .filter_map(|change| match change {
            FileChange::Expect { path, .. } => Some(path.to_string()),
            _ => None,
        })
        .collect();
    expected_paths.sort();
    assert_eq!(
        expected_paths,
        vec!["manuscript/01", "manuscript/02", "manuscript/03"],
        "移動元の本文がまだ無いことを、章ごとに 1 回ずつ"
    );
    assert_eq!(chapter_titles(&project), vec!["03", "01", "02"]);
}

#[test]
fn a_text_folder_made_after_the_plan_stops_the_move_of_a_chapter_without_text() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", &chapter_md("一章"));
    put_chapter_with_text(&project, "02", "二章");
    let plan = plan_structure_edit(&project, &move_chapter(2, 0)).unwrap();
    // 計画してから適用するまでの間に、外のエディタで第 1 章の本文ができた
    put(&project, "manuscript/01/s01.txt", "あとから書かれた本文\n");

    let error = plan.change_set.apply(&project).unwrap_err();

    assert_conflict(&error);
    assert_eq!(chapter_titles(&project), vec!["一章", "二章"]);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "あとから書かれた本文\n"
    );
    assert_eq!(
        read(&project, "manuscript/02/s01.txt").unwrap(),
        "二章の本文\n"
    );
}

/// 循環の改名は、まず元をすべて預け、次にすべてを行き先へ置く 2 段階で行うので、全部か無しかになる。
/// 計画のあとに第 2 章が外で消えたら、改名は 1 つも行われず、3 つの章がそのまま残る。
#[test]
fn a_rotation_is_all_or_nothing_when_a_chapter_disappears_after_the_plan() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章", "三章"]);
    let plan = plan_structure_edit(&project, &move_chapter(3, 0)).unwrap();
    std::fs::remove_file(dir.path().join("plot/chapters/02.md")).unwrap();

    let error = plan.change_set.apply(&project).unwrap_err();

    assert_conflict(&error);
    assert_eq!(chapter_numbers(&project), vec!["01", "03"]);
    assert_eq!(chapter_titles(&project), vec!["一章", "三章"]);
    for (number, text) in [("01", "一章"), ("02", "二章"), ("03", "三章")] {
        assert_eq!(
            read(&project, &format!("manuscript/{number}/s01.txt")).unwrap(),
            format!("{text}の本文\n"),
            "本文のフォルダは元の場所のまま"
        );
    }
    let staging = dir.path().join(".kataribe/staging");
    assert!(
        !staging.exists() || std::fs::read_dir(staging).unwrap().next().is_none(),
        "預け先に何も残らない"
    );
}

#[test]
fn a_chapter_with_broken_yaml_moves_too_and_is_named_by_its_number_only() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "雨の匂い");
    put(
        &project,
        "plot/chapters/02.md",
        "---\ntitle: [壊れた\n---\n",
    );

    let plan = plan_structure_edit(&project, &move_chapter(2, 0)).unwrap();

    assert_eq!(plan.change_set.summary, "第2章を第1章へ移します。");
    assert_eq!(
        plan.renumbered,
        vec![
            RenumberedChapter {
                from: chapter_id(1),
                to: chapter_id(2),
                title: Some("雨の匂い".to_owned()),
            },
            RenumberedChapter {
                from: chapter_id(2),
                to: chapter_id(1),
                title: None,
            },
        ]
    );
    plan.change_set.apply(&project).unwrap();
    assert_eq!(
        read(&project, "plot/chapters/01.md").unwrap(),
        "---\ntitle: [壊れた\n---\n",
        "壊れた章も、中身はそのまま移る"
    );
}

#[test]
fn moving_a_chapter_says_where_it_goes_before_and_after() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章", "雨の匂い"]);

    let plan = plan_structure_edit(&project, &move_chapter(3, 0)).unwrap();

    assert_eq!(
        plan.change_set.summary,
        "第3章「雨の匂い」を第1章へ移します。"
    );
    assert_eq!(
        plan.completed_summary,
        "第3章「雨の匂い」を第1章へ移しました。"
    );
    assert_eq!(plan.created, None);
}

#[test]
fn the_destination_is_named_by_the_number_the_position_holds_when_numbers_have_gaps() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put_chapter_with_text(&project, "01", "一章");
    put_chapter_with_text(&project, "05", "五章");

    let plan = plan_structure_edit(&project, &move_chapter(1, 1)).unwrap();

    assert_eq!(plan.change_set.summary, "第1章「一章」を第5章へ移します。");
}

#[test]
fn a_chapter_position_outside_the_list_or_the_current_one_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章", "三章"]);

    let outside = invalid_input_message(plan_structure_edit(&project, &move_chapter(1, 3)));
    let same = invalid_input_message(plan_structure_edit(&project, &move_chapter(2, 1)));

    assert!(outside.contains("4 番目"), "{outside}");
    assert!(outside.contains("3 件"), "{outside}");
    assert!(same.contains("すでに 2 番目"), "{same}");
    assert_eq!(chapter_titles(&project), vec!["一章", "二章", "三章"]);
}

#[test]
fn moving_a_chapter_that_does_not_exist_is_not_found() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章"]);

    let result = plan_structure_edit(&project, &move_chapter(7, 0));

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "{result:?}"
    );
}

#[test]
fn a_leftover_folder_where_a_chapter_would_move_is_not_in_the_way_of_a_swap() {
    let dir = TempDir::new().unwrap();
    let project = project_with_chapters(&dir, &["一章", "二章"]);
    put(&project, "manuscript/03/s01.txt", "章立ての無い本文\n");

    plan_and_apply(&project, &move_chapter(2, 0));

    assert_eq!(chapter_titles(&project), vec!["二章", "一章"]);
    assert_eq!(
        read(&project, "manuscript/03/s01.txt").unwrap(),
        "章立ての無い本文\n"
    );
}

#[test]
fn a_moved_chapter_shows_up_in_the_overview_and_the_pipeline_in_its_new_place() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/01.md",
        "---\ntitle: 一章\nscenes:\n  - id: s01\n    title: 一章の場面\n    summary: 要約\n---\nあらすじ\n",
    );
    put(&project, "manuscript/01/s01.txt", "一章の本文です。\n");
    put(&project, "style.md", "文体ガイド\n");
    put(
        &project,
        "plot/chapters/02.md",
        "---\ntitle: 二章\nscenes:\n  - id: s01\n    title: 二章の場面\n    summary: 要約\n---\nあらすじ\n",
    );

    plan_and_apply(&project, &move_chapter(2, 0));

    let plot = section_entries(&project, SectionKind::Plot);
    let labels: Vec<&str> = plot
        .iter()
        .filter(|entry| entry.kind == EntryKind::Chapter)
        .map(|entry| entry.label.as_str())
        .collect();
    assert_eq!(labels, vec!["第1章「二章」", "第2章「一章」"]);
    let manuscript = section_entries(&project, SectionKind::Manuscript);
    let written_scene_chapters: Vec<u32> = manuscript
        .iter()
        .flat_map(|chapter| &chapter.children)
        .filter(|scene| scene.exists)
        .filter_map(|scene| scene.chapter.map(|chapter| chapter.number()))
        .collect();
    assert_eq!(
        written_scene_chapters,
        vec![2],
        "本文は一章（今は第 2 章）に付いている"
    );
    let steps = pipeline(&project, DraftUnit::Scene).unwrap();
    let first_chapter_draft = steps
        .iter()
        .find(|step| {
            step.task
                == Task::Draft {
                    chapter: chapter_id(1),
                    scene: SceneId::from_number(1),
                }
        })
        .unwrap();
    assert_eq!(first_chapter_draft.state, StepState::Ready);
    let second_chapter_draft = steps
        .iter()
        .find(|step| {
            step.task
                == Task::Draft {
                    chapter: chapter_id(2),
                    scene: SceneId::from_number(1),
                }
        })
        .unwrap();
    assert_eq!(second_chapter_draft.state, StepState::Done);
}

// ---- シーン ----

const CHAPTER_WITH_THREE_SCENES: &str = "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 二人が洋館に着く。\n    pov: 霧島 凛\n    characters:\n      - 霧島 凛\n      - 佐藤 健二\n    memo: 一つ目の覚え書き\n  - id: s02\n    title: 閉ざされた書斎\n    summary: 書斎で死体が見つかる。\n    pov: 佐藤 健二\n    time: 夜\n  - id: s03\n    title: 嵐の夜\n    summary: 電話が通じなくなる。\n    target_chars: 1500\n---\n凛と健二が洋館を訪れる。\n";

fn move_scene(chapter: u32, scene: &str, position: usize) -> StructureEdit {
    StructureEdit::MoveScene {
        chapter: chapter_id(chapter),
        scene: SceneId::new(scene).unwrap(),
        position,
    }
}

fn project_with_three_scenes(dir: &TempDir) -> Project {
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_THREE_SCENES);
    put(&project, "manuscript/01/s01.txt", "一つ目の本文。\n");
    put(&project, "manuscript/01/s02.txt", "二つ目の本文。\n");
    project
}

fn scene_ids(project: &Project) -> Vec<String> {
    project
        .chapter(&chapter_id(1))
        .unwrap()
        .unwrap()
        .meta
        .scenes
        .iter()
        .map(|scene| scene.id.to_string())
        .collect()
}

#[test]
fn moving_a_scene_changes_the_order_of_the_chapter_plan() {
    let dir = TempDir::new().unwrap();
    let project = project_with_three_scenes(&dir);

    plan_and_apply(&project, &move_scene(1, "s03", 0));
    assert_eq!(scene_ids(&project), vec!["s03", "s01", "s02"]);

    plan_and_apply(&project, &move_scene(1, "s03", 2));
    assert_eq!(scene_ids(&project), vec!["s01", "s02", "s03"]);

    plan_and_apply(&project, &move_scene(1, "s01", 1));
    assert_eq!(scene_ids(&project), vec!["s02", "s01", "s03"]);
}

#[test]
fn moving_a_scene_changes_neither_the_ids_nor_the_text_files_and_the_fields_follow_the_scene() {
    let dir = TempDir::new().unwrap();
    let project = project_with_three_scenes(&dir);

    let plan = plan_and_apply(&project, &move_scene(1, "s01", 2));

    assert_eq!(
        written_paths(&plan),
        vec!["plot/chapters/01.md"],
        "書き換わるのは章立てだけ"
    );
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "一つ目の本文。\n"
    );
    assert_eq!(
        read(&project, "manuscript/01/s02.txt").unwrap(),
        "二つ目の本文。\n"
    );
    let chapter = project.chapter(&chapter_id(1)).unwrap().unwrap();
    let moved = chapter.scene(&SceneId::new("s01").unwrap()).unwrap();
    assert_eq!(moved.title, "洋館への道");
    assert_eq!(moved.pov.as_deref(), Some("霧島 凛"));
    assert_eq!(moved.characters, vec!["霧島 凛", "佐藤 健二"]);
    assert_eq!(
        moved.extra.get("memo"),
        Some(&serde_json::json!("一つ目の覚え書き")),
        "知らない項目も、シーンについていく"
    );
    assert_eq!(chapter.meta.title, "雨の匂い");
    assert_eq!(chapter.storyline, "凛と健二が洋館を訪れる。\n");
}

#[test]
fn a_moved_scene_shows_up_in_the_overview_in_its_new_place_with_its_text() {
    let dir = TempDir::new().unwrap();
    let project = project_with_three_scenes(&dir);

    plan_and_apply(&project, &move_scene(1, "s02", 0));

    let manuscript = section_entries(&project, SectionKind::Manuscript);
    let scenes: Vec<(Option<String>, bool)> = manuscript[0]
        .children
        .iter()
        .map(|scene| (scene.scene.map(|id| id.to_string()), scene.exists))
        .collect();
    assert_eq!(
        scenes,
        vec![
            (Some("s02".to_owned()), true),
            (Some("s01".to_owned()), true),
            (Some("s03".to_owned()), false),
        ]
    );
}

#[test]
fn a_chapter_with_duplicate_scene_ids_cannot_be_reordered() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    let duplicated = CHAPTER_WITH_THREE_SCENES.replace("id: s02", "id: s01");
    put(&project, "plot/chapters/01.md", &duplicated);

    let message = invalid_input_message(plan_structure_edit(&project, &move_scene(1, "s03", 0)));

    assert!(message.contains("重複"), "{message}");
    assert_eq!(read(&project, "plot/chapters/01.md").unwrap(), duplicated);
}

#[test]
fn a_chapter_with_broken_yaml_cannot_have_its_scenes_reordered() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/01.md",
        "---\ntitle: [壊れた\n---\n",
    );

    let result = plan_structure_edit(&project, &move_scene(1, "s01", 0));

    assert!(
        matches!(result, Err(EngineError::InvalidInput(_))),
        "{result:?}"
    );
}

#[test]
fn moving_a_scene_or_a_chapter_that_does_not_exist_is_not_found() {
    let dir = TempDir::new().unwrap();
    let project = project_with_three_scenes(&dir);

    let no_scene = plan_structure_edit(&project, &move_scene(1, "s09", 0));
    let no_chapter = plan_structure_edit(&project, &move_scene(4, "s01", 0));

    assert!(
        matches!(no_scene, Err(EngineError::NotFound(_))),
        "{no_scene:?}"
    );
    assert!(
        matches!(no_chapter, Err(EngineError::NotFound(_))),
        "{no_chapter:?}"
    );
}

#[test]
fn a_scene_position_outside_the_chapter_or_the_current_one_is_refused() {
    let dir = TempDir::new().unwrap();
    let project = project_with_three_scenes(&dir);

    let outside = invalid_input_message(plan_structure_edit(&project, &move_scene(1, "s01", 3)));
    let same = invalid_input_message(plan_structure_edit(&project, &move_scene(1, "s02", 1)));

    assert!(outside.contains("4 番目"), "{outside}");
    assert!(outside.contains("3 件"), "{outside}");
    assert!(same.contains("すでに 2 番目"), "{same}");
    assert_eq!(
        read(&project, "plot/chapters/01.md").unwrap(),
        CHAPTER_WITH_THREE_SCENES
    );
}

#[test]
fn a_scene_move_conflicts_when_the_chapter_changed_after_the_plan() {
    let dir = TempDir::new().unwrap();
    let project = project_with_three_scenes(&dir);
    let plan = plan_structure_edit(&project, &move_scene(1, "s03", 0)).unwrap();
    // 計画してから適用するまでの間に、外のエディタで章立てが書き換わった
    let edited = CHAPTER_WITH_THREE_SCENES.replace("雨の匂い", "雨の匂い（改題）");
    put(&project, "plot/chapters/01.md", &edited);

    let error = plan.change_set.apply(&project).unwrap_err();

    assert_conflict(&error);
    assert_eq!(read(&project, "plot/chapters/01.md").unwrap(), edited);
}

#[test]
fn moving_a_scene_says_what_it_does_before_and_after() {
    let dir = TempDir::new().unwrap();
    let project = project_with_three_scenes(&dir);

    let plan = plan_structure_edit(&project, &move_scene(1, "s03", 1)).unwrap();

    assert_eq!(
        plan.change_set.summary,
        "第1章のシーン「嵐の夜」を 2 番目に移します。"
    );
    assert_eq!(
        plan.completed_summary,
        "第1章のシーン「嵐の夜」を 2 番目に移しました。"
    );
    assert_eq!(plan.created, None);
}
