//! 原稿の取り込みと、取り込んだ本文を資料の生成に渡すことを、偽の LLM で確かめる。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use kataribe_engine::{
    DEFAULT_MIN_SCENE_CHARS, DraftUnit, Engine, EngineError, GenerationSettings, IgnoreEvents,
    ManuscriptImport, NewProject, Task, create_project, overview, plan_manuscript_import,
};
use kataribe_llm::testing::{Script, ScriptedChatModel};
use kataribe_project::{ChapterId, CharacterId, Project, Rating};
use tokio_util::sync::CancellationToken;

const ROSTER: &str = r#"{"characters": [{"id": "rin", "name": "凛", "reading": "りん", "role": "主人公", "summary": "探偵。"}]}"#;
const OUTLINE: &str = r#"{"chapters": [{"title": "続き", "storyline": "続きの話。"}]}"#;
const SCENES: &str = r#"{"scenes": [{"title": "続きの場面", "summary": "続く。", "pov": "凛", "characters": ["凛"], "place": "港", "time": "朝"}]}"#;
const DOCUMENT: &str = "# 資料\n## 見出し\n内容。\n";

fn new_project(folder: &std::path::Path, target_length: u32) -> Project {
    create_project(
        folder,
        NewProject {
            title: "みさき館".into(),
            author: None,
            genre: "mystery".into(),
            genre_note: None,
            rating: Rating::General,
            target_length,
            idea: "嵐で孤立した洋館。".into(),
        },
    )
    .unwrap()
}

fn write_file(folder: &std::path::Path, path: &str, content: &str) {
    let path = folder.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn import_of(text: &str) -> ManuscriptImport {
    ManuscriptImport {
        chapter_title: "第一章".to_owned(),
        text: text.to_owned(),
        min_scene_chars: DEFAULT_MIN_SCENE_CHARS,
    }
}

fn long_paragraph(marker: &str) -> String {
    format!("　{marker}{}。\n", "雨".repeat(300))
}

fn settings() -> GenerationSettings {
    GenerationSettings {
        draft_unit: DraftUnit::Scene,
        context_tokens: 100_000,
        quality_retries: 0,
        ..GenerationSettings::default()
    }
}

async fn generate(model: &Arc<ScriptedChatModel>, project: &Project, task: &Task) {
    Engine::new(model.clone(), settings())
        .unwrap()
        .generate(project, task, &IgnoreEvents, &CancellationToken::new())
        .await
        .unwrap();
}

fn model_replying(reply: &str) -> Arc<ScriptedChatModel> {
    Arc::new(ScriptedChatModel::new([Script::reply([reply.to_owned()])]))
}

fn last_prompt(model: &ScriptedChatModel) -> String {
    model.requests().last().unwrap().messages[1].content.clone()
}

/// 資料の生成に、本文が入るかを確かめるための、前の工程の資料。
fn prepare_documents(folder: &std::path::Path) {
    write_file(folder, "concept.md", "# 企画\n洋館の話。\n");
    write_file(folder, "world/overview.md", "# 世界観\n岬の洋館。\n");
    write_file(
        folder,
        "characters/rin.md",
        "---\nname: 凛\nreading: りん\nrole: 主人公\nsummary: 探偵。\n---\n## 口調\n静か。\n",
    );
    write_file(folder, "plot/synopsis.md", "# あらすじ\n起承転結。\n");
}

fn import_marker(project: &Project) {
    let plan = plan_manuscript_import(project, &import_of(&long_paragraph("原稿の目印"))).unwrap();
    plan.change_set.apply(project).unwrap();
}

#[test]
fn blank_lines_make_scenes_and_the_status_counts_the_whole_text() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    let text = format!(
        "{}\n{}\n\n\n{}",
        long_paragraph("一"),
        long_paragraph("二"),
        long_paragraph("三")
    );

    let plan = plan_manuscript_import(&project, &import_of(&text)).unwrap();
    plan.change_set.apply(&project).unwrap();

    let chapter = project.chapters().unwrap().remove(0);
    assert_eq!(chapter.meta.title, "第一章");
    let titles: Vec<&str> = chapter
        .meta
        .scenes
        .iter()
        .map(|scene| scene.title.as_str())
        .collect();
    assert_eq!(titles, vec!["シーン 1", "シーン 2", "シーン 3"]);
    assert!(
        chapter
            .meta
            .scenes
            .iter()
            .all(|scene| scene.summary.is_empty())
    );
    let first = project
        .scene_text(&chapter.id, &chapter.meta.scenes[0].id)
        .unwrap()
        .unwrap();
    assert_eq!(first, long_paragraph("一"));
    let total: usize = [
        long_paragraph("一"),
        long_paragraph("二"),
        long_paragraph("三"),
    ]
    .iter()
    .map(|text| kataribe_text::count::count_chars(text))
    .sum();
    assert_eq!(overview(&project).unwrap().total_chars, total);
}

#[test]
fn a_single_line_block_joins_the_next_scene() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    let text = format!(
        "{}\n一行だけの書き出し。\n\n{}",
        long_paragraph("一"),
        long_paragraph("二")
    );

    let plan = plan_manuscript_import(&project, &import_of(&text)).unwrap();
    plan.change_set.apply(&project).unwrap();

    let chapter = project.chapters().unwrap().remove(0);
    assert_eq!(chapter.meta.scenes.len(), 2);
    let second = project
        .scene_text(&chapter.id, &chapter.meta.scenes[1].id)
        .unwrap()
        .unwrap();
    assert!(
        second.starts_with("一行だけの書き出し。\n\n　二"),
        "{second}"
    );
}

#[test]
fn chapters_are_added_after_the_existing_ones() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    write_file(
        folder.path(),
        "plot/chapters/01.md",
        "---\ntitle: 既存\n---\n話。\n",
    );

    let plan = plan_manuscript_import(&project, &import_of(&long_paragraph("一"))).unwrap();
    plan.change_set.apply(&project).unwrap();

    assert_eq!(
        project
            .chapters()
            .unwrap()
            .iter()
            .map(|chapter| chapter.id)
            .collect::<Vec<_>>(),
        vec![ChapterId::from_number(1), ChapterId::from_number(2)]
    );
    assert!(folder.path().join("manuscript/02/s01.txt").exists());
}

#[test]
fn an_existing_manuscript_folder_is_not_overwritten() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    write_file(folder.path(), "manuscript/01/s01.txt", "残す本文。\n");

    let error = plan_manuscript_import(&project, &import_of(&long_paragraph("一"))).unwrap_err();

    assert!(matches!(error, EngineError::InvalidInput(_)), "{error}");
    assert_eq!(
        std::fs::read_to_string(folder.path().join("manuscript/01/s01.txt")).unwrap(),
        "残す本文。\n"
    );
}

#[test]
fn an_empty_manuscript_or_title_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);

    assert!(plan_manuscript_import(&project, &import_of("\n\n")).is_err());
    let untitled = ManuscriptImport {
        chapter_title: "  ".to_owned(),
        ..import_of(&long_paragraph("一"))
    };
    assert!(plan_manuscript_import(&project, &untitled).is_err());
}

#[test]
fn a_manuscript_with_more_scenes_than_a_chapter_can_hold_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    let import = ManuscriptImport {
        min_scene_chars: 1,
        ..import_of(&"　段落。\n\n".repeat(1000))
    };

    let error = plan_manuscript_import(&project, &import).unwrap_err();

    assert!(matches!(error, EngineError::InvalidInput(_)), "{error}");
    assert!(!folder.path().join("plot/chapters/01.md").exists());
}

#[tokio::test]
async fn long_materials_and_manuscript_still_fit_a_context_just_above_the_wide_threshold() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    let long = "長い資料の文章。\n".repeat(1250);
    write_file(folder.path(), "concept.md", &format!("# 企画\n{long}"));
    write_file(
        folder.path(),
        "world/overview.md",
        &format!("# 世界観\n{long}"),
    );
    let plan = plan_manuscript_import(
        &project,
        &import_of(&format!("　{}。\n", "雨".repeat(40_000))),
    )
    .unwrap();
    plan.change_set.apply(&project).unwrap();
    let model = model_replying(ROSTER);
    let settings = GenerationSettings {
        context_tokens: 32_000,
        ..settings()
    };

    Engine::new(model.clone(), settings)
        .unwrap()
        .generate(
            &project,
            &Task::Cast,
            &IgnoreEvents,
            &CancellationToken::new(),
        )
        .await
        .unwrap();

    let prompt = last_prompt(&model);
    assert!(
        prompt.chars().count() < 30_400,
        "{}",
        prompt.chars().count()
    );
}

#[tokio::test]
async fn generating_documents_reads_the_existing_manuscript() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    prepare_documents(folder.path());
    import_marker(&project);
    let tasks = [
        (Task::Concept, DOCUMENT),
        (Task::World, DOCUMENT),
        (Task::Cast, ROSTER),
        (
            Task::Character {
                id: CharacterId::new("rin").unwrap(),
            },
            DOCUMENT,
        ),
        (Task::Synopsis, DOCUMENT),
        (Task::Outline, OUTLINE),
    ];

    for (task, reply) in tasks {
        let model = model_replying(reply);
        generate(&model, &project, &task).await;
        let prompt = last_prompt(&model);
        assert!(prompt.contains("原稿の目印"), "{task:?}: {prompt}");
        assert!(prompt.contains("矛盾させない"), "{task:?}: {prompt}");
    }
}

#[tokio::test]
async fn without_a_manuscript_the_documents_mention_none() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    prepare_documents(folder.path());
    let model = model_replying(DOCUMENT);

    generate(&model, &project, &Task::World).await;

    assert!(!last_prompt(&model).contains("すでに書かれている本文"));
}

#[tokio::test]
async fn a_broken_chapter_does_not_stop_document_generation() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    prepare_documents(folder.path());
    write_file(folder.path(), "plot/chapters/01.md", "---\ntitle: [\n---\n");
    let model = model_replying(DOCUMENT);

    generate(&model, &project, &Task::World).await;

    assert!(!last_prompt(&model).contains("すでに書かれている本文"));
}

#[tokio::test]
async fn the_style_guide_does_not_read_the_manuscript() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 3000);
    prepare_documents(folder.path());
    import_marker(&project);
    let model = model_replying(DOCUMENT);

    generate(&model, &project, &Task::Style).await;

    assert!(!last_prompt(&model).contains("原稿の目印"));
}

#[tokio::test]
async fn the_scene_plan_shares_out_what_the_manuscript_has_not_used() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path(), 8000);
    prepare_documents(folder.path());
    let manuscript = format!("　{}。\n", "雨".repeat(6000));
    let plan = plan_manuscript_import(&project, &import_of(&manuscript)).unwrap();
    plan.change_set.apply(&project).unwrap();
    write_file(
        folder.path(),
        "plot/chapters/02.md",
        "---\ntitle: 続き\n---\n続きの話。\n",
    );
    let model = model_replying(SCENES);

    generate(
        &model,
        &project,
        &Task::ScenePlan {
            chapter: ChapterId::from_number(2),
        },
    )
    .await;

    // 残りは 8000 - 6000 = 2000 字で、2 シーンに分けると 1 シーン 1000 字
    assert!(
        last_prompt(&model).contains("約1000字"),
        "{}",
        last_prompt(&model)
    );
}
