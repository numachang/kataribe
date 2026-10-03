//! 指示から人物・世界観の資料を作って足す生成を、偽の LLM と作品フォルダで確かめる。
#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use kataribe_engine::{
    ChangeSet, DraftUnit, Engine, EngineError, EventSink, FileChange, GenerationEvent,
    GenerationSettings, IgnoreEvents, NewProject, NoticeLevel, StepState, Task, create_project,
    pipeline,
};
use kataribe_llm::testing::{Script, ScriptedChatModel};
use kataribe_project::{CharacterId, Project, Rating};
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;

use common::{new_project, put, read};

const KIRISHIMA: &str = "---\nname: 霧島 凛\nreading: きりしま りん\nrole: 主人公\nsummary: 盲目の少女探偵。\norder: 1\n---\n## 口調\n静かに話す。\n";
const SATO_ENTRY: &str = r#"{"name": "佐藤 健二", "reading": "さとう けんじ", "role": "相棒", "summary": "凛の助手を務める青年。"}"#;
const SATO_PROFILE: &str = "## 口調\n一人称は「僕」。丁寧に話す。\n";
const INSTRUCTION: &str = "凛の助手になる、気の優しい青年を足してください。";
const ADULT_RULE: &str = "18 歳以上の成人";

fn settings(quality_retries: u32) -> GenerationSettings {
    GenerationSettings {
        context_tokens: 16_384,
        quality_retries,
        ..GenerationSettings::default()
    }
}

fn engine_with(
    scripts: impl IntoIterator<Item = Script>,
    quality_retries: u32,
) -> (Engine, Arc<ScriptedChatModel>) {
    let model = Arc::new(ScriptedChatModel::new(scripts));
    let engine = Engine::new(model.clone(), settings(quality_retries)).unwrap();
    (engine, model)
}

fn add_character(instruction: &str) -> Task {
    Task::AddCharacter {
        instruction: instruction.to_owned(),
    }
}

fn add_world_document(name: Option<&str>, instruction: &str) -> Task {
    Task::AddWorldDocument {
        name: name.map(str::to_owned),
        instruction: instruction.to_owned(),
    }
}

/// 届いたイベントを順に記録する。
#[derive(Default)]
struct RecordEvents(std::sync::Mutex<Vec<GenerationEvent>>);

impl EventSink for RecordEvents {
    fn emit(&self, event: GenerationEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl RecordEvents {
    fn notices(&self) -> Vec<(NoticeLevel, String)> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                GenerationEvent::Notice { level, message } => Some((*level, message.clone())),
                _ => None,
            })
            .collect()
    }
}

async fn generate(
    engine: &Engine,
    project: &Project,
    task: &Task,
) -> Result<ChangeSet, EngineError> {
    engine
        .generate(project, task, &IgnoreEvents, &CancellationToken::new())
        .await
}

/// 変更案が、新しいファイル 1 つの書き込みだけであることを確かめ、そのパスと内容を返す。
fn only_new_file(changes: &ChangeSet) -> (&str, &str) {
    let [
        FileChange::Write {
            path,
            content,
            previous: None,
            base_hash: None,
        },
    ] = changes.files.as_slice()
    else {
        panic!(
            "新しいファイル 1 つの書き込みだけのはず: {:?}",
            changes.files
        );
    };
    (path.as_str(), content.as_str())
}

fn sent_text(model: &ScriptedChatModel, request_index: usize) -> (String, String) {
    let messages = &model.requests()[request_index].messages;
    (messages[0].content.clone(), messages[1].content.clone())
}

fn adult_project(folder: &std::path::Path) -> Project {
    create_project(
        folder,
        NewProject {
            title: "夜の館".into(),
            author: None,
            genre: "mystery".into(),
            genre_note: None,
            rating: Rating::R18,
            target_length: 6000,
            idea: "閉ざされた館で起きる出来事。".into(),
        },
    )
    .unwrap()
}

// ---- 人物 ----

#[tokio::test]
async fn adding_a_character_writes_one_new_file_named_after_the_reading() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    put(&project, "characters/kirishima-rin.md", KIRISHIMA);
    let (engine, model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );

    let changes = generate(&engine, &project, &add_character(INSTRUCTION))
        .await
        .unwrap();

    let (path, content) = only_new_file(&changes);
    assert_eq!(path, "characters/sato-kenji.md");
    assert_eq!(
        changes.summary,
        "人物「佐藤 健二」を生成しました（characters/sato-kenji.md）。"
    );
    assert_eq!(model.requests().len(), 2);
    assert!(
        content.starts_with("---\n") && content.contains("name: 佐藤 健二"),
        "{content}"
    );
    assert!(content.contains("order: 2"), "表示順は最大の次: {content}");
    assert!(
        content.ends_with(SATO_PROFILE),
        "本文は 2 回目の出力: {content}"
    );
    assert_eq!(
        read(&project, "characters/sato-kenji.md"),
        None,
        "変更案を作っただけでは書き込まない"
    );
}

#[tokio::test]
async fn an_applied_character_shows_up_as_a_finished_profile_step() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );

    generate(&engine, &project, &add_character(INSTRUCTION))
        .await
        .unwrap()
        .apply(&project)
        .unwrap();

    let steps = pipeline(&project, DraftUnit::Beat).unwrap();
    let profile = steps
        .iter()
        .find(|step| step.label == "人物資料: 佐藤 健二")
        .expect("人物資料の工程が出るはず");
    assert_eq!(profile.state, StepState::Done);
}

#[tokio::test]
async fn the_character_prompts_carry_the_instruction_the_cast_and_the_adult_rule() {
    let folder = tempfile::tempdir().unwrap();
    let project = adult_project(folder.path());
    put(&project, "characters/kirishima-rin.md", KIRISHIMA);
    put(
        &project,
        "plot/synopsis.md",
        "# あらすじ\n嵐の夜、館の主が死ぬ。\n",
    );
    let (engine, model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );

    generate(&engine, &project, &add_character(INSTRUCTION))
        .await
        .unwrap();

    let (entry_system, entry_user) = sent_text(&model, 0);
    assert!(entry_system.contains(ADULT_RULE), "{entry_system}");
    assert!(entry_user.contains(INSTRUCTION), "{entry_user}");
    assert!(entry_user.contains("霧島 凛"), "既存の人物: {entry_user}");
    assert!(entry_user.contains("盲目の少女探偵"), "{entry_user}");
    assert!(
        entry_user.contains("館の主が死ぬ"),
        "あらすじ: {entry_user}"
    );
    let (profile_system, profile_user) = sent_text(&model, 1);
    assert!(profile_system.contains(ADULT_RULE), "{profile_system}");
    assert!(profile_user.contains(INSTRUCTION), "{profile_user}");
    assert!(
        profile_user.contains("佐藤 健二"),
        "今回の人物: {profile_user}"
    );
    assert!(
        profile_user.contains("霧島 凛"),
        "一覧の人物: {profile_user}"
    );
}

#[tokio::test]
async fn a_character_id_that_is_taken_gets_a_number() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    put(&project, "characters/sato-kenji.md", KIRISHIMA);
    let (engine, _model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );

    let changes = generate(&engine, &project, &add_character(INSTRUCTION))
        .await
        .unwrap();

    assert_eq!(only_new_file(&changes).0, "characters/sato-kenji-2.md");
}

#[tokio::test]
async fn a_character_saved_while_generating_is_not_overwritten() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );
    let finished_steps = AtomicU32::new(0);
    // 人物資料の本文を作り終えたところで、利用者が同じ ID の人物を保存する
    let sink = |event: GenerationEvent| {
        if matches!(event, GenerationEvent::StepFinished { .. })
            && finished_steps.fetch_add(1, Ordering::SeqCst) == 1
        {
            put(&project, "characters/sato-kenji.md", KIRISHIMA);
        }
    };

    let changes = engine
        .generate(
            &project,
            &add_character(INSTRUCTION),
            &sink,
            &CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(only_new_file(&changes).0, "characters/sato-kenji-2.md");
    changes.apply(&project).unwrap();
    assert_eq!(
        read(&project, "characters/sato-kenji.md").as_deref(),
        Some(KIRISHIMA)
    );
}

#[tokio::test]
async fn a_name_that_an_existing_character_has_is_generated_again() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    put(&project, "characters/kirishima-rin.md", KIRISHIMA);
    let same_name = r#"{"name": "霧島凛", "reading": "きりしま りん", "role": "相棒", "summary": "同じ名前。"}"#;
    let (engine, model) = engine_with(
        [
            Script::reply([same_name]),
            Script::reply([SATO_ENTRY]),
            Script::reply([SATO_PROFILE]),
        ],
        1,
    );
    let events = RecordEvents::default();

    let changes = engine
        .generate(
            &project,
            &add_character(INSTRUCTION),
            &events,
            &CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(only_new_file(&changes).0, "characters/sato-kenji.md");
    assert_eq!(model.requests().len(), 3, "項目を 2 回、本文を 1 回");
    let warnings: Vec<_> = events
        .notices()
        .into_iter()
        .filter(|(level, _)| *level == NoticeLevel::Warning)
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].1.contains("すでにいる人物「霧島 凛」"),
        "{warnings:?}"
    );
}

#[tokio::test]
async fn a_name_with_latin_letters_is_generated_again() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let latin = r#"{"name": "佐藤 Kenji", "reading": "さとう けんじ", "role": "相棒", "summary": "青年。"}"#;
    let (engine, model) = engine_with(
        [
            Script::reply([latin]),
            Script::reply([SATO_ENTRY]),
            Script::reply([SATO_PROFILE]),
        ],
        1,
    );

    let changes = generate(&engine, &project, &add_character(INSTRUCTION))
        .await
        .unwrap();

    assert_eq!(model.requests().len(), 3);
    assert!(only_new_file(&changes).1.contains("name: 佐藤 健二"));
}

#[tokio::test]
async fn a_problem_that_stays_after_the_retries_is_reported_as_a_warning_and_the_character_is_kept()
{
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    put(&project, "characters/kirishima-rin.md", KIRISHIMA);
    let same_name = r#"{"name": "霧島 凛", "reading": "きりしま りん", "role": "相棒", "summary": "同じ名前。"}"#;
    let (engine, _model) = engine_with(
        [Script::reply([same_name]), Script::reply([SATO_PROFILE])],
        0,
    );
    let events = RecordEvents::default();

    let changes = engine
        .generate(
            &project,
            &add_character(INSTRUCTION),
            &events,
            &CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(
        only_new_file(&changes).0,
        "characters/kirishima-rin-2.md",
        "既存の人物は上書きしない"
    );
    assert!(
        events
            .notices()
            .iter()
            .any(|(level, message)| *level == NoticeLevel::Warning
                && message.contains("確認して直してください")),
        "{:?}",
        events.notices()
    );
}

#[tokio::test]
async fn an_empty_name_that_stays_fails_without_writing_a_profile() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let empty_name = r#"{"name": " ", "reading": "さとう", "role": "相棒", "summary": "青年。"}"#;
    let (engine, model) = engine_with([Script::reply([empty_name])], 0);

    let error = generate(&engine, &project, &add_character(INSTRUCTION))
        .await
        .unwrap_err();

    assert!(matches!(error, EngineError::InvalidOutput(_)), "{error:?}");
    assert_eq!(model.requests().len(), 1);
}

#[tokio::test]
async fn a_blank_character_instruction_is_refused_without_calling_the_llm() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, model) = engine_with([], 0);

    let error = generate(&engine, &project, &add_character(" \n　"))
        .await
        .unwrap_err();

    assert!(matches!(error, EngineError::InvalidInput(_)), "{error:?}");
    assert_eq!(model.requests().len(), 0, "LLM は呼ばれないはず");
}

#[tokio::test]
async fn the_generated_documents_that_lack_the_new_character_are_announced() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    put(
        &project,
        "plot/synopsis.md",
        "# あらすじ\n嵐の夜、館の主が死ぬ。\n",
    );
    put(
        &project,
        "plot/chapters/01.md",
        "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 着く。\n---\n本文\n",
    );
    let (engine, _model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );
    let events = RecordEvents::default();

    engine
        .generate(
            &project,
            &add_character(INSTRUCTION),
            &events,
            &CancellationToken::new(),
        )
        .await
        .unwrap();

    let notices = events.notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0].0, NoticeLevel::Info);
    assert!(
        notices[0].1.contains("あらすじ・章立て・シーン構成"),
        "{notices:?}"
    );
}

#[tokio::test]
async fn no_announcement_is_made_when_nothing_has_been_generated_yet() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );
    let events = RecordEvents::default();

    engine
        .generate(
            &project,
            &add_character(INSTRUCTION),
            &events,
            &CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(events.notices(), []);
}

#[tokio::test]
async fn cancelling_a_character_generation_writes_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with(
        [Script::reply([SATO_ENTRY]), Script::reply([SATO_PROFILE])],
        0,
    );
    let cancel = CancellationToken::new();
    cancel.cancel();

    let error = engine
        .generate(
            &project,
            &add_character(INSTRUCTION),
            &IgnoreEvents,
            &cancel,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, EngineError::Cancelled), "{error:?}");
    assert_eq!(project.character_ids().unwrap(), Vec::<CharacterId>::new());
}

// ---- 世界観の資料 ----

const PORT_TOWN: &str = "承知しました。\n# 港町の歴史\n\n## 成り立ち\n江戸の頃に開かれた漁港。\n";
const WORLD_INSTRUCTION: &str = "舞台になる港町の歴史を足してください。";

#[tokio::test]
async fn a_world_document_with_a_kanji_title_is_named_doc() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, model) = engine_with([Script::reply([PORT_TOWN])], 0);

    let changes = generate(
        &engine,
        &project,
        &add_world_document(None, WORLD_INSTRUCTION),
    )
    .await
    .unwrap();

    let (path, content) = only_new_file(&changes);
    assert_eq!(path, "world/doc.md");
    assert_eq!(
        content,
        "# 港町の歴史\n\n## 成り立ち\n江戸の頃に開かれた漁港。\n"
    );
    assert_eq!(
        changes.summary,
        "世界観の資料「港町の歴史」を生成しました（world/doc.md）。"
    );
    assert_eq!(model.requests().len(), 1);
}

#[tokio::test]
async fn a_world_document_with_a_kana_title_is_named_in_romaji() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with([Script::reply(["# ようご しゅう\n\n霧: 朝に出る。\n"])], 0);

    let changes = generate(
        &engine,
        &project,
        &add_world_document(None, WORLD_INSTRUCTION),
    )
    .await
    .unwrap();

    assert_eq!(only_new_file(&changes).0, "world/yogo-shu.md");
}

#[tokio::test]
async fn a_given_file_name_is_used_for_the_world_document() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with([Script::reply([PORT_TOWN])], 0);

    let changes = generate(
        &engine,
        &project,
        &add_world_document(Some("port-town"), WORLD_INSTRUCTION),
    )
    .await
    .unwrap();

    assert_eq!(only_new_file(&changes).0, "world/port-town.md");
}

#[tokio::test]
async fn a_used_or_invalid_file_name_fails_before_the_llm_is_called() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    put(&project, "world/port-town.md", "# 港町\n");
    let (engine, model) = engine_with([Script::reply([PORT_TOWN])], 0);

    let used = generate(
        &engine,
        &project,
        &add_world_document(Some("port-town"), WORLD_INSTRUCTION),
    )
    .await
    .unwrap_err();
    let invalid = generate(
        &engine,
        &project,
        &add_world_document(Some("Port Town"), WORLD_INSTRUCTION),
    )
    .await
    .unwrap_err();
    let overview = generate(
        &engine,
        &project,
        &add_world_document(Some("overview"), WORLD_INSTRUCTION),
    )
    .await
    .unwrap_err();

    assert!(matches!(used, EngineError::InvalidInput(_)), "{used:?}");
    assert!(matches!(invalid, EngineError::Project(_)), "{invalid:?}");
    assert!(matches!(overview, EngineError::Project(_)), "{overview:?}");
    assert_eq!(model.requests().len(), 0, "LLM は呼ばれないはず");
}

#[tokio::test]
async fn a_world_document_without_a_heading_is_generated_again() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, model) = engine_with(
        [
            Script::reply(["江戸の頃に開かれた漁港。\n"]),
            Script::reply([PORT_TOWN]),
        ],
        1,
    );

    let changes = generate(
        &engine,
        &project,
        &add_world_document(None, WORLD_INSTRUCTION),
    )
    .await
    .unwrap();

    assert_eq!(model.requests().len(), 2);
    assert!(only_new_file(&changes).1.starts_with("# 港町の歴史"));
}

#[tokio::test]
async fn a_world_document_that_never_gets_a_heading_fails() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, model) = engine_with(
        [
            Script::reply(["江戸の頃に開かれた漁港。\n"]),
            Script::reply(["## 成り立ち\n江戸の頃。\n"]),
        ],
        1,
    );

    let error = generate(
        &engine,
        &project,
        &add_world_document(None, WORLD_INSTRUCTION),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, EngineError::InvalidOutput(_)), "{error:?}");
    assert_eq!(model.requests().len(), 2);
}

#[tokio::test]
async fn the_world_prompt_carries_the_instruction_the_titles_and_the_adult_rule() {
    let folder = tempfile::tempdir().unwrap();
    let project = adult_project(folder.path());
    put(
        &project,
        "world/overview.md",
        "# 世界観\n霧の深い岬の館。\n",
    );
    put(
        &project,
        "world/glossary.md",
        "# 用語集\n霧笛: 朝を告げる。\n",
    );
    put(&project, "characters/kirishima-rin.md", KIRISHIMA);
    let (engine, model) = engine_with([Script::reply([PORT_TOWN])], 0);

    generate(
        &engine,
        &project,
        &add_world_document(None, WORLD_INSTRUCTION),
    )
    .await
    .unwrap();

    let (system, user) = sent_text(&model, 0);
    assert!(system.contains(ADULT_RULE), "{system}");
    assert!(user.contains(WORLD_INSTRUCTION), "{user}");
    assert!(user.contains("・用語集"), "既存の資料の題: {user}");
    assert!(
        user.contains("霧笛: 朝を告げる"),
        "既存の資料の抜粋: {user}"
    );
    assert!(user.contains("霧島 凛"), "人物の一覧: {user}");
}

#[tokio::test]
async fn a_blank_world_instruction_is_refused_without_calling_the_llm() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, model) = engine_with([], 0);

    let error = generate(&engine, &project, &add_world_document(None, "  "))
        .await
        .unwrap_err();

    assert!(matches!(error, EngineError::InvalidInput(_)), "{error:?}");
    assert_eq!(model.requests().len(), 0, "LLM は呼ばれないはず");
}

#[tokio::test]
async fn adding_a_world_document_tells_that_only_later_generations_use_it() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with([Script::reply([PORT_TOWN])], 0);
    let events = RecordEvents::default();

    engine
        .generate(
            &project,
            &add_world_document(None, WORLD_INSTRUCTION),
            &events,
            &CancellationToken::new(),
        )
        .await
        .unwrap();

    let notices = events.notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0].0, NoticeLevel::Info);
    assert!(notices[0].1.contains("これからの生成"), "{notices:?}");
}

#[tokio::test]
async fn cancelling_a_world_document_generation_writes_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let (engine, _model) = engine_with([Script::reply([PORT_TOWN])], 0);
    let cancel = CancellationToken::new();
    cancel.cancel();

    let error = engine
        .generate(
            &project,
            &add_world_document(None, WORLD_INSTRUCTION),
            &IgnoreEvents,
            &cancel,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, EngineError::Cancelled), "{error:?}");
    assert_eq!(project.world_document_names().unwrap(), []);
}

// ---- 共通 ----

#[test]
fn the_pipeline_never_lists_the_addition_tasks() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    put(&project, "characters/kirishima-rin.md", KIRISHIMA);

    let steps = pipeline(&project, DraftUnit::Beat).unwrap();

    assert!(steps.iter().all(|step| !matches!(
        step.task,
        Task::AddCharacter { .. } | Task::AddWorldDocument { .. }
    )));
}

#[test]
fn the_addition_tasks_are_written_with_the_names_of_the_structure_edits() {
    let character = serde_json::to_value(add_character("x")).unwrap();
    let world = serde_json::to_value(add_world_document(Some("a"), "x")).unwrap();

    assert_eq!(character["kind"], "add_character");
    assert_eq!(world["kind"], "add_world_document");
}
