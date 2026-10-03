//! 作品の構成（人物・世界観の資料・シーン）を自分で足したり消したりする操作を、作品フォルダに対して確かめる。
#![allow(clippy::unwrap_used)]

mod common;

use kataribe_engine::{
    DraftUnit, EngineError, EntryKind, FileChange, NewScenePlan, OverviewEntry, SectionKind,
    StepState, StructureEdit, StructurePlan, Task, overview, pipeline, plan_structure_edit,
    suggest_character_id,
};
use kataribe_project::{
    ChapterId, CharacterId, CharacterMeta, Project, ProjectError, RelPath, SceneId,
};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use common::{new_project, put, read};

const CHAPTER_WITH_TWO_SCENES: &str = "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 二人が洋館に着く。\n    pov: 霧島 凛\n    characters:\n      - 霧島 凛\n      - 佐藤 健二\n  - id: s02\n    title: 閉ざされた書斎\n    summary: 書斎で死体が見つかる。\n    pov: 佐藤健二\n    characters:\n      - 凛\n---\n凛と健二が洋館を訪れる。\n";

fn rel(path: &str) -> RelPath {
    RelPath::new(path).unwrap()
}

fn character_id(id: &str) -> CharacterId {
    CharacterId::new(id).unwrap()
}

fn character_meta(name: &str, reading: &str) -> CharacterMeta {
    CharacterMeta {
        name: name.to_owned(),
        reading: Some(reading.to_owned()).filter(|reading| !reading.is_empty()),
        role: "主人公".to_owned(),
        summary: "盲目の少女探偵。".to_owned(),
        ..CharacterMeta::default()
    }
}

fn add_character(id: Option<&str>, name: &str, reading: &str, body: &str) -> StructureEdit {
    StructureEdit::AddCharacter {
        id: id.map(character_id),
        meta: character_meta(name, reading),
        body: body.to_owned(),
    }
}

fn add_world_document(name: Option<&str>, title: &str, body: &str) -> StructureEdit {
    StructureEdit::AddWorldDocument {
        name: name.map(str::to_owned),
        title: title.to_owned(),
        body: body.to_owned(),
    }
}

fn new_scene(title: &str) -> NewScenePlan {
    NewScenePlan {
        title: title.to_owned(),
        summary: "ここで何かが起きる。".to_owned(),
        pov: Some("霧島 凛".to_owned()),
        characters: vec!["霧島 凛".to_owned(), "  ".to_owned()],
        place: None,
        time: Some("夜".to_owned()),
        target_chars: Some(1500),
    }
}

fn add_scene(chapter: u32, before: Option<&str>, title: &str) -> StructureEdit {
    StructureEdit::AddScene {
        chapter: ChapterId::from_number(chapter),
        before: before.map(|id| SceneId::new(id).unwrap()),
        scene: new_scene(title),
    }
}

fn remove_scene(chapter: u32, scene: &str) -> StructureEdit {
    StructureEdit::RemoveScene {
        chapter: ChapterId::from_number(chapter),
        scene: SceneId::new(scene).unwrap(),
    }
}

/// 構成の操作を計画して適用し、計画を返す。
fn plan_and_apply(project: &Project, edit: &StructureEdit) -> StructurePlan {
    let plan = plan_structure_edit(project, edit).unwrap();
    plan.change_set.apply(project).unwrap();
    plan
}

fn scene_ids(project: &Project, chapter: u32) -> Vec<String> {
    let chapter = project
        .chapter(&ChapterId::from_number(chapter))
        .unwrap()
        .unwrap();
    chapter
        .meta
        .scenes
        .iter()
        .map(|scene| scene.id.to_string())
        .collect()
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

fn invalid_input_message(result: Result<StructurePlan, EngineError>) -> String {
    match result {
        Err(EngineError::InvalidInput(message)) => message,
        other => panic!("入力の誤りになるはず: {other:?}"),
    }
}

// ---- 人物を足す ----

#[test]
fn adding_a_character_makes_the_id_from_the_reading_and_writes_a_new_file() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let plan = plan_and_apply(
        &project,
        &add_character(None, "霧島 凛", "きりしま りん", "## 口調\n静かに話す。\n"),
    );

    assert_eq!(plan.created, Some(rel("characters/kirishima-rin.md")));
    let character = project
        .character(&character_id("kirishima-rin"))
        .unwrap()
        .unwrap();
    assert_eq!(character.meta.name, "霧島 凛");
    assert_eq!(character.meta.reading.as_deref(), Some("きりしま りん"));
    assert_eq!(character.body, "## 口調\n静かに話す。\n");
}

#[test]
fn adding_a_character_falls_back_to_the_name_and_then_to_a_fixed_id() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let from_name = plan_and_apply(&project, &add_character(None, "Alice", "", ""));
    let from_nothing = plan_and_apply(&project, &add_character(None, "霧島 凛", "", ""));

    assert_eq!(from_name.created, Some(rel("characters/alice.md")));
    assert_eq!(from_nothing.created, Some(rel("characters/character.md")));
}

#[test]
fn adding_characters_with_the_same_reading_numbers_the_ids() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    plan_and_apply(
        &project,
        &add_character(None, "佐藤 健二", "さとう けんじ", ""),
    );
    let second = plan_and_apply(
        &project,
        &add_character(None, "佐藤 賢治", "さとう けんじ", ""),
    );

    assert_eq!(second.created, Some(rel("characters/sato-kenji-2.md")));
}

#[test]
fn a_new_character_without_an_order_goes_to_the_end() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "characters/rin.md",
        "---\nname: 凛\norder: 3\n---\n",
    );
    put(&project, "characters/broken.md", "---\nname: [\n---\n");

    let plan = plan_and_apply(&project, &add_character(Some("kenji"), "健二", "", ""));

    let character = project.character(&character_id("kenji")).unwrap().unwrap();
    assert_eq!(character.meta.order, Some(4));
    assert_eq!(plan.created, Some(rel("characters/kenji.md")));
}

#[test]
fn a_new_character_keeps_an_explicit_order() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "characters/rin.md",
        "---\nname: 凛\norder: 3\n---\n",
    );
    let mut edit = add_character(Some("kenji"), "健二", "", "");
    if let StructureEdit::AddCharacter { meta, .. } = &mut edit {
        meta.order = Some(1);
    }

    plan_and_apply(&project, &edit);

    let character = project.character(&character_id("kenji")).unwrap().unwrap();
    assert_eq!(character.meta.order, Some(1));
}

#[test]
fn adding_a_character_trims_the_entered_fields() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    let mut edit = add_character(Some("rin"), "  霧島 凛  ", "   ", "");
    if let StructureEdit::AddCharacter { meta, .. } = &mut edit {
        meta.role = " 主人公 ".to_owned();
    }

    plan_and_apply(&project, &edit);

    let character = project.character(&character_id("rin")).unwrap().unwrap();
    assert_eq!(character.meta.name, "霧島 凛");
    assert_eq!(character.meta.reading, None);
    assert_eq!(character.meta.role, "主人公");
}

#[test]
fn a_new_character_with_an_empty_body_shows_up_as_a_step_ready_to_start() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    plan_and_apply(&project, &add_character(Some("rin"), "霧島 凛", "", ""));

    let steps = pipeline(&project, DraftUnit::Beat).unwrap();
    let step = steps
        .iter()
        .find(|step| {
            step.task
                == Task::Character {
                    id: character_id("rin"),
                }
        })
        .unwrap();
    assert_eq!(step.label, "人物資料: 霧島 凛");
    assert_eq!(step.state, StepState::Ready);
}

#[test]
fn adding_a_character_shows_up_in_the_overview() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    plan_and_apply(&project, &add_character(Some("rin"), "霧島 凛", "", "本文"));

    let entries = section_entries(&project, SectionKind::Characters);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "霧島 凛");
    assert_eq!(entries[0].kind, EntryKind::Character);
    assert_eq!(entries[0].path.as_deref(), Some("characters/rin.md"));
}

#[test]
fn adding_a_character_refuses_an_empty_name_and_a_used_id() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "---\nname: 凛\n---\n");

    let empty_name = plan_structure_edit(&project, &add_character(Some("kenji"), "  ", "", ""));
    let used_id = plan_structure_edit(&project, &add_character(Some("rin"), "別の凛", "", ""));

    assert_eq!(
        invalid_input_message(empty_name),
        "人物の名前を入力してください。"
    );
    assert_eq!(
        invalid_input_message(used_id),
        "ID「rin」はもう使われています。"
    );
    assert_eq!(
        read(&project, "characters/rin.md").unwrap(),
        "---\nname: 凛\n---\n"
    );
}

#[test]
fn a_character_change_conflicts_when_the_id_is_taken_between_plan_and_apply() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    let plan =
        plan_structure_edit(&project, &add_character(Some("rin"), "霧島 凛", "", "")).unwrap();
    put(
        &project,
        "characters/rin.md",
        "---\nname: 先に書かれた凛\n---\n",
    );

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(
        read(&project, "characters/rin.md").unwrap(),
        "---\nname: 先に書かれた凛\n---\n"
    );
}

#[test]
fn suggesting_a_character_id_uses_the_reading_and_avoids_used_ids() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "characters/kirishima-rin.md",
        "---\nname: [\n---\n",
    );

    let first = suggest_character_id(&project, "きりしま りん", "霧島 凛").unwrap();
    let from_name = suggest_character_id(&project, "", "Mr. Smith").unwrap();
    let nothing = suggest_character_id(&project, "", "").unwrap();

    assert_eq!(first.as_str(), "kirishima-rin-2");
    assert_eq!(from_name.as_str(), "mr-smith");
    assert_eq!(nothing.as_str(), "character");
}

// ---- 人物を消す ----

#[test]
fn removing_a_character_moves_the_file_to_the_trash() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "characters/rin.md",
        "---\nname: 霧島 凛\n---\n本文です。\n",
    );

    let plan = plan_structure_edit(
        &project,
        &StructureEdit::RemoveCharacter {
            id: character_id("rin"),
        },
    )
    .unwrap();

    let [FileChange::Trash { path, files }] = plan.change_set.files.as_slice() else {
        panic!("ゴミ箱へ移す変更が 1 つのはず: {:?}", plan.change_set.files);
    };
    assert_eq!(path, &rel("characters/rin.md"));
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, rel("characters/rin.md"));
    assert!(files[0].chars > 0);
    assert_eq!(plan.created, None);
    assert!(
        read(&project, "characters/rin.md").is_some(),
        "計画だけでは消えない"
    );

    plan.change_set.apply(&project).unwrap();

    assert_eq!(read(&project, "characters/rin.md"), None);
    assert_eq!(
        section_entries(&project, SectionKind::Characters),
        Vec::new()
    );
    let trashed = std::fs::read_dir(dir.path().join(".kataribe/trash"))
        .unwrap()
        .count();
    assert_eq!(trashed, 1);
}

#[test]
fn removing_a_character_lists_the_scenes_that_name_them() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "---\nname: 霧島 凛\n---\n");
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    let plan = plan_structure_edit(
        &project,
        &StructureEdit::RemoveCharacter {
            id: character_id("rin"),
        },
    )
    .unwrap();

    // s01: 視点にも登場にも「霧島 凛」。s02: 登場人物に名前だけの「凛」。視点の「佐藤健二」は別人
    assert_eq!(plan.references.len(), 2);
    let first = &plan.references[0];
    assert_eq!(
        (first.scene.to_string(), first.as_pov, first.as_character),
        ("s01".to_owned(), true, true)
    );
    assert_eq!(first.chapter_title, "雨の匂い");
    assert_eq!(first.scene_title, "洋館への道");
    let second = &plan.references[1];
    assert_eq!(
        (second.scene.to_string(), second.as_pov, second.as_character),
        ("s02".to_owned(), false, true)
    );
    assert_eq!(plan.notices, Vec::<String>::new());
}

#[test]
fn name_references_ignore_differences_in_spacing() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "characters/kenji.md",
        "---\nname: 佐藤 健二\n---\n",
    );
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    let plan = plan_structure_edit(
        &project,
        &StructureEdit::RemoveCharacter {
            id: character_id("kenji"),
        },
    )
    .unwrap();

    let scenes: Vec<String> = plan
        .references
        .iter()
        .map(|reference| reference.scene.to_string())
        .collect();
    assert_eq!(
        scenes,
        vec!["s01", "s02"],
        "「佐藤健二」と「佐藤 健二」は同じ人物"
    );
}

#[test]
fn a_broken_chapter_is_reported_in_the_notices_and_the_other_chapters_are_still_checked() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "---\nname: 霧島 凛\n---\n");
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);
    put(&project, "plot/chapters/02.md", "---\ntitle: [\n---\n");

    let plan = plan_structure_edit(
        &project,
        &StructureEdit::RemoveCharacter {
            id: character_id("rin"),
        },
    )
    .unwrap();

    assert_eq!(plan.references.len(), 2);
    assert_eq!(plan.notices.len(), 1);
    assert!(
        plan.notices[0].contains("第2章は読めないため"),
        "{}",
        plan.notices[0]
    );
}

#[test]
fn a_character_with_broken_yaml_can_still_be_removed_but_references_are_not_checked() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "---\nname: [\n---\n");
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    let plan = plan_structure_edit(
        &project,
        &StructureEdit::RemoveCharacter {
            id: character_id("rin"),
        },
    )
    .unwrap();
    plan.change_set.apply(&project).unwrap();

    assert_eq!(plan.references, Vec::new());
    assert_eq!(plan.notices.len(), 1);
    assert!(
        plan.notices[0].contains("characters/rin.md"),
        "{}",
        plan.notices[0]
    );
    assert_eq!(read(&project, "characters/rin.md"), None);
}

#[test]
fn removing_a_missing_character_is_not_found() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let result = plan_structure_edit(
        &project,
        &StructureEdit::RemoveCharacter {
            id: character_id("rin"),
        },
    );

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "{result:?}"
    );
}

#[test]
fn removing_a_character_conflicts_when_the_file_changed_after_the_plan() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "characters/rin.md", "---\nname: 霧島 凛\n---\n");
    let plan = plan_structure_edit(
        &project,
        &StructureEdit::RemoveCharacter {
            id: character_id("rin"),
        },
    )
    .unwrap();
    put(
        &project,
        "characters/rin.md",
        "---\nname: 霧島 凛\n---\n外で書き足した本文\n",
    );

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert!(
        read(&project, "characters/rin.md")
            .unwrap()
            .contains("外で書き足した本文")
    );
}

// ---- 世界観の資料を足す・消す ----

#[test]
fn adding_a_world_document_writes_a_heading_and_the_body() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let plan = plan_and_apply(
        &project,
        &add_world_document(Some("glossary"), "用語集", "霧：朝に出る。\n"),
    );

    assert_eq!(plan.created, Some(rel("world/glossary.md")));
    assert_eq!(
        read(&project, "world/glossary.md").unwrap(),
        "# 用語集\n\n霧：朝に出る。\n"
    );
    let labels: Vec<String> = section_entries(&project, SectionKind::World)
        .into_iter()
        .map(|entry| entry.label)
        .collect();
    assert_eq!(labels, vec!["世界観", "用語集"]);
}

#[test]
fn a_world_document_title_in_kana_gives_a_romaji_file_name() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let plan = plan_and_apply(&project, &add_world_document(None, "ようご しゅう", ""));

    assert_eq!(plan.created, Some(rel("world/yogo-shu.md")));
    assert_eq!(
        read(&project, "world/yogo-shu.md").unwrap(),
        "# ようご しゅう\n"
    );
}

#[test]
fn world_document_titles_with_kanji_get_numbered_doc_names() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let first = plan_and_apply(&project, &add_world_document(None, "港町の歴史", ""));
    let second = plan_and_apply(&project, &add_world_document(None, "岬の地図", ""));
    let third = plan_and_apply(&project, &add_world_document(None, "洋館の見取り図", ""));

    assert_eq!(first.created, Some(rel("world/doc.md")));
    assert_eq!(second.created, Some(rel("world/doc-2.md")));
    assert_eq!(third.created, Some(rel("world/doc-3.md")));
}

#[test]
fn a_world_document_titled_overview_does_not_replace_the_overview() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "world/overview.md", "# 世界観\n昭和初期。\n");

    let plan = plan_and_apply(&project, &add_world_document(None, "Overview", ""));

    assert_eq!(plan.created, Some(rel("world/overview-2.md")));
    assert_eq!(
        read(&project, "world/overview.md").unwrap(),
        "# 世界観\n昭和初期。\n"
    );
}

#[test]
fn adding_a_world_document_refuses_bad_titles_and_file_names() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "world/glossary.md", "# 用語集\n");

    let empty_title = plan_structure_edit(&project, &add_world_document(None, " ", ""));
    let two_lines = plan_structure_edit(&project, &add_world_document(None, "一行目\n二行目", ""));
    let used_name = plan_structure_edit(
        &project,
        &add_world_document(Some("glossary"), "別の用語集", ""),
    );
    let overview_name =
        plan_structure_edit(&project, &add_world_document(Some("overview"), "概要", ""));
    let bad_name = plan_structure_edit(&project, &add_world_document(Some("用語集"), "用語集", ""));

    assert_eq!(
        invalid_input_message(empty_title),
        "資料の題を入力してください。"
    );
    assert_eq!(
        invalid_input_message(two_lines),
        "資料の題は 1 行で入力してください。"
    );
    assert_eq!(
        invalid_input_message(used_name),
        "ファイル名「glossary」はもう使われています。"
    );
    assert!(
        matches!(
            overview_name,
            Err(EngineError::Project(ProjectError::InvalidId(_)))
        ),
        "{overview_name:?}"
    );
    assert!(
        matches!(
            bad_name,
            Err(EngineError::Project(ProjectError::InvalidId(_)))
        ),
        "{bad_name:?}"
    );
}

#[test]
fn a_blank_file_name_means_automatic() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let plan = plan_and_apply(&project, &add_world_document(Some("  "), "ようご", ""));

    assert_eq!(plan.created, Some(rel("world/yogo.md")));
}

#[test]
fn removing_a_world_document_moves_it_to_the_trash() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "world/glossary.md", "# 用語集\n霧\n");
    let edit = StructureEdit::RemoveWorldDocument {
        path: rel("world/glossary.md"),
    };

    let plan = plan_and_apply(&project, &edit);

    assert_eq!(
        plan.change_set.summary,
        "世界観の資料「用語集」をゴミ箱へ移します。"
    );
    assert_eq!(
        plan.completed_summary,
        "世界観の資料「用語集」をゴミ箱へ移しました。"
    );
    assert_eq!(read(&project, "world/glossary.md"), None);
}

#[test]
fn the_world_overview_and_files_outside_world_cannot_be_removed_as_world_documents() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "world/overview.md", "# 世界観\n");
    put(&project, "world/notes.txt", "メモ");
    put(&project, "concept.md", "# 企画\n");

    for path in [
        "world/overview.md",
        "world/notes.txt",
        "world/maps/town.md",
        "concept.md",
        "kataribe.yaml",
    ] {
        let result = plan_structure_edit(
            &project,
            &StructureEdit::RemoveWorldDocument { path: rel(path) },
        );
        assert!(
            matches!(result, Err(EngineError::InvalidInput(_))),
            "{path} は消せないはず: {result:?}"
        );
    }
    assert!(read(&project, "world/overview.md").is_some());
}

#[test]
fn removing_a_missing_world_document_is_not_found() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());

    let result = plan_structure_edit(
        &project,
        &StructureEdit::RemoveWorldDocument {
            path: rel("world/glossary.md"),
        },
    );

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "{result:?}"
    );
}

// ---- シーンを足す ----

#[test]
fn adding_a_scene_appends_it_to_the_end_with_the_next_id() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    let plan = plan_and_apply(&project, &add_scene(1, None, "屋根裏の足音"));

    assert_eq!(plan.created, Some(rel("plot/chapters/01.md")));
    assert_eq!(scene_ids(&project, 1), vec!["s01", "s02", "s03"]);
    let chapter = project
        .chapter(&ChapterId::from_number(1))
        .unwrap()
        .unwrap();
    let added = chapter.scene(&SceneId::from_number(3)).unwrap();
    assert_eq!(added.title, "屋根裏の足音");
    assert_eq!(added.pov.as_deref(), Some("霧島 凛"));
    assert_eq!(
        added.characters,
        vec!["霧島 凛"],
        "空白だけの名前は入れない"
    );
    assert_eq!(added.time.as_deref(), Some("夜"));
    assert_eq!(added.place, None);
    assert_eq!(added.target_chars, Some(1500));
    assert_eq!(chapter.storyline, "凛と健二が洋館を訪れる。\n");
}

#[test]
fn adding_a_scene_before_another_inserts_it_at_that_position() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    plan_and_apply(&project, &add_scene(1, Some("s02"), "間のシーン"));

    let chapter = project
        .chapter(&ChapterId::from_number(1))
        .unwrap()
        .unwrap();
    let titles: Vec<&str> = chapter
        .meta
        .scenes
        .iter()
        .map(|scene| scene.title.as_str())
        .collect();
    assert_eq!(titles, vec!["洋館への道", "間のシーン", "閉ざされた書斎"]);
    assert_eq!(scene_ids(&project, 1), vec!["s01", "s03", "s02"]);
}

#[test]
fn a_new_scene_does_not_reuse_the_id_of_a_removed_scene_whose_text_remains() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);
    put(
        &project,
        "manuscript/01/s03.txt",
        "章立てから消えたシーンの本文が残っている。",
    );

    plan_and_apply(&project, &add_scene(1, None, "新しいシーン"));

    assert_eq!(scene_ids(&project, 1), vec!["s01", "s02", "s04"]);
    assert_eq!(
        read(&project, "manuscript/01/s03.txt").unwrap(),
        "章立てから消えたシーンの本文が残っている。"
    );
}

#[test]
fn a_new_scene_shows_up_in_the_manuscript_section_with_its_chapter_and_scene() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    plan_and_apply(&project, &add_scene(1, None, "屋根裏の足音"));

    let chapters = section_entries(&project, SectionKind::Manuscript);
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].chapter, Some(ChapterId::from_number(1)));
    assert_eq!(chapters[0].scene, None);
    let scenes = &chapters[0].children;
    assert_eq!(scenes.len(), 3);
    assert_eq!(scenes[2].label, "3. 屋根裏の足音");
    assert_eq!(scenes[2].scene, Some(SceneId::from_number(3)));
    assert_eq!(scenes[2].chapter, Some(ChapterId::from_number(1)));
    assert_eq!(scenes[2].path.as_deref(), Some("manuscript/01/s03.txt"));
}

#[test]
fn adding_a_scene_keeps_the_fields_the_app_does_not_know() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(
        &project,
        "plot/chapters/01.md",
        "---\ntitle: 雨の匂い\nmemo: 手で足した項目\nscenes:\n  - id: s01\n    title: 一つ目\n    summary: 要約\n    mood: 緊張\n---\n本文\n",
    );

    plan_and_apply(&project, &add_scene(1, None, "二つ目"));

    let text = read(&project, "plot/chapters/01.md").unwrap();
    assert!(text.contains("memo: 手で足した項目"), "{text}");
    assert!(text.contains("mood: 緊張"), "{text}");
}

#[test]
fn adding_a_scene_refuses_an_empty_title_a_missing_chapter_and_an_unknown_position() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    let empty_title = plan_structure_edit(&project, &add_scene(1, None, "  "));
    let no_chapter = plan_structure_edit(&project, &add_scene(2, None, "題"));
    let no_position = plan_structure_edit(&project, &add_scene(1, Some("s09"), "題"));

    assert_eq!(
        invalid_input_message(empty_title),
        "シーンの題を入力してください。"
    );
    assert!(
        matches!(no_chapter, Err(EngineError::NotFound(_))),
        "{no_chapter:?}"
    );
    assert!(
        matches!(no_position, Err(EngineError::NotFound(_))),
        "{no_position:?}"
    );
}

#[test]
fn a_chapter_that_cannot_be_split_into_fields_is_not_rewritten() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    let duplicated = CHAPTER_WITH_TWO_SCENES.replace("id: s02", "id: s01");
    put(&project, "plot/chapters/01.md", &duplicated);
    put(&project, "plot/chapters/02.md", "---\ntitle: [\n---\n");

    let duplicate_id = plan_structure_edit(&project, &add_scene(1, None, "題"));
    let broken_yaml = plan_structure_edit(&project, &remove_scene(2, "s01"));

    let message = invalid_input_message(duplicate_id);
    assert!(message.contains("重複"), "{message}");
    assert!(
        matches!(broken_yaml, Err(EngineError::InvalidInput(_))),
        "{broken_yaml:?}"
    );
    assert_eq!(read(&project, "plot/chapters/01.md").unwrap(), duplicated);
}

#[test]
fn a_scene_change_conflicts_when_the_chapter_changed_after_the_plan() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);
    let plan = plan_structure_edit(&project, &add_scene(1, None, "題")).unwrap();
    let edited = CHAPTER_WITH_TWO_SCENES.replace("雨の匂い", "雪の匂い");
    put(&project, "plot/chapters/01.md", &edited);

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(read(&project, "plot/chapters/01.md").unwrap(), edited);
}

// ---- シーンを消す ----

#[test]
fn removing_a_scene_with_text_moves_the_text_to_the_trash_too() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);
    put(
        &project,
        "manuscript/01/s02.txt",
        "書斎の扉は、内側から鍵がかかっていた。\n",
    );
    put(&project, "manuscript/01/s01.txt", "雨が降っていた。\n");

    let plan = plan_structure_edit(&project, &remove_scene(1, "s02")).unwrap();

    // 本文がゴミ箱へ移ることは、要約の文ではなく、ゴミ箱へ移るものの一覧（下）で知らせる
    assert!(plan.change_set.summary.ends_with("を削除します。"));
    assert!(plan.completed_summary.ends_with("を削除しました。"));
    let trashed: Vec<&RelPath> = plan
        .change_set
        .files
        .iter()
        .filter_map(|change| match change {
            FileChange::Trash { path, .. } => Some(path),
            FileChange::Write { .. } => None,
        })
        .collect();
    assert_eq!(trashed, vec![&rel("manuscript/01/s02.txt")]);

    plan.change_set.apply(&project).unwrap();

    assert_eq!(scene_ids(&project, 1), vec!["s01"]);
    assert_eq!(read(&project, "manuscript/01/s02.txt"), None);
    assert_eq!(
        read(&project, "manuscript/01/s01.txt").unwrap(),
        "雨が降っていた。\n"
    );
    let trash_batch = std::fs::read_dir(dir.path().join(".kataribe/trash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        std::fs::read_to_string(trash_batch.join("manuscript/01/s02.txt")).unwrap(),
        "書斎の扉は、内側から鍵がかかっていた。\n"
    );
}

#[test]
fn removing_a_scene_without_text_only_rewrites_the_chapter() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    let plan = plan_structure_edit(&project, &remove_scene(1, "s01")).unwrap();

    assert_eq!(plan.change_set.files.len(), 1);
    assert!(matches!(plan.change_set.files[0], FileChange::Write { .. }));
    assert_eq!(plan.created, None);
    assert!(!plan.change_set.summary.contains("ゴミ箱"));

    plan.change_set.apply(&project).unwrap();

    assert_eq!(scene_ids(&project, 1), vec!["s02"]);
}

#[test]
fn removing_an_unknown_scene_is_not_found() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);

    let result = plan_structure_edit(&project, &remove_scene(1, "s09"));

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "{result:?}"
    );
}

#[test]
fn removing_a_scene_conflicts_when_its_text_changed_after_the_plan() {
    let dir = TempDir::new().unwrap();
    let project = new_project(dir.path());
    put(&project, "plot/chapters/01.md", CHAPTER_WITH_TWO_SCENES);
    put(&project, "manuscript/01/s02.txt", "計画したときの本文。\n");
    let plan = plan_structure_edit(&project, &remove_scene(1, "s02")).unwrap();
    put(
        &project,
        "manuscript/01/s02.txt",
        "そのあと書き足した本文。\n",
    );

    let error = plan.change_set.apply(&project).unwrap_err();

    assert!(
        matches!(error, EngineError::Project(ProjectError::Conflict { .. })),
        "{error:?}"
    );
    assert_eq!(
        scene_ids(&project, 1),
        vec!["s01", "s02"],
        "章立ても変わらない"
    );
    assert_eq!(
        read(&project, "manuscript/01/s02.txt").unwrap(),
        "そのあと書き足した本文。\n"
    );
}
