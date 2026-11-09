//! `references/` の参考文が、文体ガイドと本文の生成に手本として渡ることを、偽の LLM で確かめる。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use kataribe_engine::{
    ChangeSet, DraftUnit, Engine, EventSink, FileChange, GenerationEvent, GenerationSettings,
    IgnoreEvents, NewProject, NoticeLevel, Task, create_project, overview,
};
use kataribe_llm::testing::{Script, ScriptedChatModel};
use kataribe_project::{ChapterId, Project, Rating, SceneId};
use tokio_util::sync::CancellationToken;

const STYLE_GUIDE: &str = "# 文体ガイド\n## 人称と視点\n三人称。\n## 文体見本\n　雨が降る。\n";
const CHAPTER: &str = "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 着く。\n    target_chars: 500\n---\n洋館を訪れる。\n";
const GENERATED_STYLE: &str =
    "# 文体ガイド\n## 人称と視点\n三人称。\n## 語り口\n短い文。\n## 避けること\n比喩。\n";

fn new_project(folder: &std::path::Path) -> Project {
    create_project(
        folder,
        NewProject {
            title: "みさき館".into(),
            author: None,
            genre: "mystery".into(),
            genre_note: None,
            rating: Rating::General,
            target_length: 6000,
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

fn write_reference(folder: &std::path::Path, name: &str, content: &str) {
    write_file(folder, &format!("references/{name}"), content);
}

/// 地の文と会話がそろった参考文。
fn reference_text(marker: &str) -> String {
    format!("　{marker}の地の文が続いた。\n「{marker}の会話です」\n").repeat(100)
}

fn settings(context_tokens: u32) -> GenerationSettings {
    GenerationSettings {
        draft_unit: DraftUnit::Scene,
        chars_per_call: 1500,
        context_tokens,
        quality_retries: 0,
        polish: false,
        ..GenerationSettings::default()
    }
}

#[derive(Default)]
struct RecordNotices(Mutex<Vec<(NoticeLevel, String)>>);

impl EventSink for RecordNotices {
    fn emit(&self, event: GenerationEvent) {
        if let GenerationEvent::Notice { level, message } = event {
            self.0.lock().unwrap().push((level, message));
        }
    }
}

async fn generate(
    model: &Arc<ScriptedChatModel>,
    context_tokens: u32,
    project: &Project,
    task: &Task,
) -> ChangeSet {
    let engine = Engine::new(model.clone(), settings(context_tokens)).unwrap();
    engine
        .generate(project, task, &IgnoreEvents, &CancellationToken::new())
        .await
        .unwrap()
}

fn style_model() -> Arc<ScriptedChatModel> {
    Arc::new(ScriptedChatModel::new([Script::reply([GENERATED_STYLE])]))
}

/// エンジンは目標の 3 割に満たない出力を「書けなかった」とみなすので、十分な長さの本文にする。
fn written_scene() -> String {
    let mut text = String::new();
    for index in 1..=20 {
        writeln!(text, "　書斎の外で、波が{index}度目の音を立てた。").unwrap();
    }
    text
}

fn draft_model() -> Arc<ScriptedChatModel> {
    Arc::new(ScriptedChatModel::new([Script::reply([written_scene()])]))
}

fn prepare_concept(folder: &std::path::Path) {
    write_file(folder, "concept.md", "# 企画\n洋館の話。\n");
}

fn prepare_scene(folder: &std::path::Path, style: &str) {
    write_file(folder, "style.md", style);
    write_file(folder, "plot/chapters/01.md", CHAPTER);
}

fn scene_one() -> Task {
    Task::Draft {
        chapter: ChapterId::from_number(1),
        scene: SceneId::new("s01").unwrap(),
    }
}

fn written_content(changes: &ChangeSet, path: &str) -> String {
    changes
        .files
        .iter()
        .find_map(|change| match change {
            FileChange::Write {
                path: written,
                content,
                ..
            } if written.as_str() == path => Some(content.clone()),
            _ => None,
        })
        .unwrap()
}

#[tokio::test]
async fn the_style_guide_describes_the_references_and_quotes_its_sample_from_them() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_concept(folder.path());
    write_reference(folder.path(), "過去作.txt", &reference_text("手本"));
    let model = style_model();

    let changes = generate(&model, 100_000, &project, &Task::Style).await;

    let request = &model.requests()[0];
    assert!(
        request.messages[0].content.contains("文体を言語化"),
        "{}",
        request.messages[0].content
    );
    let prompt = &request.messages[1].content;
    assert!(prompt.contains("手本の会話です"), "参考文の抜粋が入る");
    assert!(!prompt.contains("400 字程度の見本文"), "見本は作文させない");
    let guide = written_content(&changes, "style.md");
    let sample = guide.split("## 文体見本\n").nth(1).unwrap();
    assert!(sample.contains("手本の会話です"), "{guide}");
    assert!(sample.contains("手本の地の文が続いた"), "{guide}");
}

#[tokio::test]
async fn when_no_reference_fits_the_guide_is_generic_and_the_user_is_told() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_concept(folder.path());
    write_reference(folder.path(), "過去作.txt", &reference_text("手本"));
    let model = style_model();
    let engine = Engine::new(model.clone(), settings(4096)).unwrap();
    let sink = RecordNotices::default();

    let changes = engine
        .generate(&project, &Task::Style, &sink, &CancellationToken::new())
        .await
        .unwrap();

    let request = &model.requests()[0];
    assert!(request.messages[0].content.contains("最もふさわしい文体"));
    let guide = written_content(&changes, "style.md");
    assert!(!guide.contains("手本の会話です"), "{guide}");
    assert!(
        sink.0.lock().unwrap().iter().any(
            |(level, message)| *level == NoticeLevel::Warning && message.contains("文脈の長さ")
        ),
    );
}

#[tokio::test]
async fn a_sample_the_model_wrote_anyway_is_replaced_by_the_references() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_concept(folder.path());
    write_reference(folder.path(), "過去作.txt", &reference_text("手本"));
    let model = Arc::new(ScriptedChatModel::new([Script::reply([
        "# 文体ガイド\n## 語り口\n短い文。\n## 文体見本\nモデルが作文した見本。\n",
    ])]));

    let changes = generate(&model, 100_000, &project, &Task::Style).await;

    let guide = written_content(&changes, "style.md");
    assert!(!guide.contains("モデルが作文した見本"), "{guide}");
    assert_eq!(guide.matches("## 文体見本").count(), 1, "{guide}");
}

#[tokio::test]
async fn without_references_the_style_prompt_still_asks_for_a_sample() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_concept(folder.path());
    let model = style_model();

    let changes = generate(&model, 100_000, &project, &Task::Style).await;

    let request = &model.requests()[0];
    assert!(request.messages[0].content.contains("最もふさわしい文体"));
    let prompt = &request.messages[1].content;
    assert!(prompt.contains("400 字程度の見本文"));
    assert!(!prompt.contains("参考文"), "{prompt}");
    assert_eq!(written_content(&changes, "style.md"), GENERATED_STYLE);
}

#[tokio::test]
async fn long_contexts_pass_excerpts_of_every_reference_as_style_examples() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_scene(folder.path(), STYLE_GUIDE);
    write_reference(folder.path(), "a.txt", &reference_text("甲"));
    write_reference(folder.path(), "b.txt", &reference_text("乙"));
    let model = draft_model();

    generate(&model, 100_000, &project, &scene_one()).await;

    let request = &model.requests()[0];
    assert!(
        request.messages[0]
            .content
            .contains("固有名詞・言い回しは使わない")
    );
    let prompt = &request.messages[1].content;
    assert!(prompt.contains("【文体の手本"), "{prompt}");
    assert!(
        prompt.contains("甲の会話です") && prompt.contains("乙の会話です"),
        "{prompt}"
    );
}

#[tokio::test]
async fn without_references_the_draft_prompt_has_no_style_examples() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_scene(folder.path(), STYLE_GUIDE);
    let model = draft_model();

    generate(&model, 100_000, &project, &scene_one()).await;

    let request = &model.requests()[0];
    assert!(!request.messages[0].content.contains("文体の手本"));
    assert!(!request.messages[1].content.contains("文体の手本"));
}

#[tokio::test]
async fn short_contexts_get_no_style_examples() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_scene(folder.path(), STYLE_GUIDE);
    write_reference(folder.path(), "a.txt", &reference_text("甲"));
    let model = draft_model();

    generate(&model, 16_384, &project, &scene_one()).await;

    let request = &model.requests()[0];
    assert!(!request.messages[0].content.contains("文体の手本"));
    assert!(!request.messages[1].content.contains("甲の会話です"));
}

#[tokio::test]
async fn style_examples_are_dropped_first_when_the_budget_is_small() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let long_style = format!(
        "{STYLE_GUIDE}## 語り口\n{}文体ガイドの末尾の目印\n",
        "語".repeat(27_000)
    );
    prepare_scene(folder.path(), &long_style);
    write_reference(folder.path(), "a.txt", &reference_text("甲").repeat(20));
    let model = draft_model();

    // 予算は約 33,000。文体ガイド（約 27,000）と残りの資料に、手本（最小でも 6,000 字）を足すと収まらない
    generate(&model, 36_000, &project, &scene_one()).await;

    let prompt = &model.requests()[0].messages[1].content;
    assert!(
        prompt.contains("文体ガイドの末尾の目印"),
        "文体ガイドは削られない"
    );
    assert!(!prompt.contains("甲の会話です"), "手本が先に省かれる");
}

#[tokio::test]
async fn a_reference_that_is_not_utf8_is_skipped_with_a_warning() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_scene(folder.path(), STYLE_GUIDE);
    write_reference(folder.path(), "ok.txt", &reference_text("甲"));
    std::fs::write(
        folder.path().join("references/sjis.txt"),
        [0x82, 0xa0, 0xff],
    )
    .unwrap();
    let model = draft_model();
    let engine = Engine::new(model.clone(), settings(100_000)).unwrap();
    let sink = RecordNotices::default();

    engine
        .generate(&project, &scene_one(), &sink, &CancellationToken::new())
        .await
        .unwrap();

    let notices = sink.0.lock().unwrap();
    assert!(
        notices
            .iter()
            .any(|(level, message)| *level == NoticeLevel::Warning && message.contains("sjis.txt")),
        "{notices:?}"
    );
    assert!(
        model.requests()[0].messages[1]
            .content
            .contains("甲の会話です")
    );
}

#[test]
fn references_are_not_counted_as_manuscript() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_scene(folder.path(), STYLE_GUIDE);
    write_file(folder.path(), "manuscript/01/s01.txt", "　雨。\n");
    let before = overview(&project).unwrap().total_chars;
    write_reference(folder.path(), "a.txt", &reference_text("甲"));

    assert_eq!(overview(&project).unwrap().total_chars, before);
}
