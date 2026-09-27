//! 偽の LLM を使って、作品の作成から本文の生成までを通しで確かめる。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;
use std::sync::Arc;

use kataribe_engine::{
    DraftUnit, Engine, EventSink, GenerationEvent, GenerationSettings, IgnoreEvents, NewProject,
    StepState, Task, create_project, pipeline,
};
use kataribe_llm::testing::{Script, ScriptedChatModel};
use kataribe_project::{ChapterId, Project, Rating, RelPath, SceneId};
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;

const CONCEPT: &str = "承知しました。\n\n# 企画\n## ログライン\n嵐の洋館で起きた密室殺人を、盲目の少女探偵が音で解く。\n";
const STYLE: &str = "# 文体ガイド\n## 人称と視点\n三人称、凛の視点に寄り添う。\n## 文体見本\n　雨の音が、屋敷の壁を叩いていた。\n";
const WORLD: &str = "# 世界観\n## 時代と舞台\n昭和初期、岬に建つ洋館。\n";
const CAST: &str = r#"{"characters": [
  {"id": "kirishima-rin", "name": "霧島 凛", "reading": "きりしま りん", "role": "主人公", "summary": "盲目の少女探偵。"},
  {"id": "sato-kenji", "name": "佐藤 健二", "reading": "さとう けんじ", "role": "相棒", "summary": "凛の助手を務める青年。"}
]}"#;
const PROFILE_RIN: &str = "## 口調\n一人称は「わたし」。静かに話す。\n";
const PROFILE_KENJI: &str = "## 口調\n一人称は「僕」。丁寧に話す。\n";
const SYNOPSIS: &str =
    "# あらすじ\n## 起\n嵐の夜、洋館の主が密室で死ぬ。\n## 結\n凛が犯人を言い当てる。\n";
const OUTLINE: &str = r#"{"chapters": [{"title": "雨の匂い", "storyline": "凛と健二が洋館を訪れ、主の死体が見つかる。"}]}"#;
const SCENES: &str = r#"{"scenes": [
  {"title": "洋館への道", "summary": "嵐の中、二人が洋館に着く。", "pov": "霧島 凛", "characters": ["霧島 凛", "佐藤 健二"], "place": "岬の道", "time": "夕刻"},
  {"title": "閉ざされた書斎", "summary": "書斎で主の死体が見つかる。", "pov": "霧島 凛", "characters": ["霧島 凛"], "place": "書斎", "time": "夜"}
]}"#;
const SCENE1_PART1: &str =
    "雨が、傘の布を激しく叩いていた。\n凛は健二の腕に手を添え、坂道を登った。\n";
const SCENE1_PART2: &str =
    "やがて、潮の匂いに混じって古い木の香りがした。\n「着きましたよ」と健二が言った。\n";
const DIGEST1: &str = "凛と健二は嵐の中、岬の洋館にたどり着いた。";
const SCENE2_PART1: &str =
    "書斎の扉は、内側から鍵がかかっていた。\n凛は扉に耳を当て、息をひそめた。\n";
const SCENE2_PART2: &str =
    "扉を破ると、主は机に伏して動かなかった。\n凛は静かに、部屋の音を聞いた。\n";

/// 目印の文に、描写の文を足して、1 回分の目標（1500 字）の半分ほどの本文にする。
/// エンジンは目標の 3 割に満たない出力を「書けなかった」とみなすため。
/// 描写の文は目印ごとに変える（前の本文と同じ段落は、書き写しとして取り除かれるため）。
fn prose(marker: &str) -> String {
    let scene_tag: String = marker.chars().take(4).collect();
    let mut text = marker.to_owned();
    for index in 1..=40 {
        writeln!(
            text,
            "{scene_tag}のあいだ、遠くで波が{index}度目の音を立てた。"
        )
        .unwrap();
    }
    text
}

fn new_project(folder: &std::path::Path) -> Project {
    create_project(
        folder,
        NewProject {
            title: "みさき館の殺人".into(),
            author: None,
            genre: "mystery".into(),
            genre_note: None,
            rating: Rating::General,
            target_length: 6000,
            idea: "嵐で孤立した洋館で、盲目の少女探偵が密室殺人を解く。".into(),
        },
    )
    .unwrap()
}

fn settings(unit: DraftUnit) -> GenerationSettings {
    GenerationSettings {
        draft_unit: unit,
        chars_per_call: 1500,
        context_tokens: 16_384,
        quality_retries: 0,
        polish: false,
        ..GenerationSettings::default()
    }
}

async fn run(engine: &Engine, project: &Project, task: &Task) {
    let changes = engine
        .generate(project, task, &IgnoreEvents, &CancellationToken::new())
        .await
        .unwrap_or_else(|error| panic!("{task:?} の生成に失敗: {error}"));
    changes.apply(project).unwrap();
}

/// 取りかかれる工程がなくなるまで、先頭から順に生成して適用する（CLI の run と同じ進め方）。
async fn run_until_blocked(engine: &Engine, project: &Project, unit: DraftUnit) -> Vec<Task> {
    let mut executed = Vec::new();
    while let Some(step) = pipeline(project, unit)
        .unwrap()
        .into_iter()
        .find(|step| step.state == StepState::Ready)
    {
        run(engine, project, &step.task).await;
        executed.push(step.task);
    }
    executed
}

/// 届いたイベントを順に記録する。
#[derive(Default)]
struct RecordEvents(std::sync::Mutex<Vec<GenerationEvent>>);

impl EventSink for RecordEvents {
    fn emit(&self, event: GenerationEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn generation_first_tells_which_llm_it_uses_then_reports_each_step() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let model = Arc::new(ScriptedChatModel::new([Script::reply([CONCEPT])]));
    let engine = Engine::new(model, settings(DraftUnit::Beat)).unwrap();
    let events = RecordEvents::default();

    engine
        .generate(&project, &Task::Concept, &events, &CancellationToken::new())
        .await
        .unwrap();

    let events = events.0.into_inner().unwrap();
    assert_eq!(
        events.first(),
        Some(&GenerationEvent::Started {
            model: "台本どおりに応答するテスト用のモデル".to_owned()
        })
    );
    assert!(matches!(
        events.get(1),
        Some(GenerationEvent::StepStarted { index: 1, .. })
    ));
    assert!(matches!(
        events.last(),
        Some(GenerationEvent::StepFinished { .. })
    ));
}

#[tokio::test]
async fn whole_pipeline_writes_a_complete_project() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply([CONCEPT]),
        Script::reply([STYLE]),
        Script::reply([WORLD]),
        Script::reply([CAST]),
        Script::reply([PROFILE_RIN]),
        Script::reply([PROFILE_KENJI]),
        Script::reply([SYNOPSIS]),
        Script::reply([OUTLINE]),
        Script::reply([SCENES]),
        Script::reply([prose(SCENE1_PART1)]),
        Script::reply([prose(SCENE1_PART2)]),
        Script::reply([DIGEST1]),
        Script::reply([prose(SCENE2_PART1)]),
        Script::reply([prose(SCENE2_PART2)]),
    ]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Scene)).unwrap();

    let executed = run_until_blocked(&engine, &project, DraftUnit::Scene).await;

    let chapter = ChapterId::from_number(1);
    assert_eq!(executed.len(), 11, "{executed:?}");
    assert!(
        pipeline(&project, DraftUnit::Scene)
            .unwrap()
            .iter()
            .all(|step| step.state == StepState::Done)
    );

    let concept = project
        .markdown(&"concept.md".parse().unwrap())
        .unwrap()
        .unwrap();
    assert!(
        concept.body.starts_with("# 企画"),
        "前置きが除かれていない: {}",
        concept.body
    );

    let characters = project.characters().unwrap();
    assert_eq!(characters.len(), 2);
    assert_eq!(characters[0].meta.name, "霧島 凛");
    assert!(characters[0].body.contains("わたし"));

    let plan = project.chapter(&chapter).unwrap().unwrap();
    assert_eq!(plan.meta.title, "雨の匂い");
    assert_eq!(plan.meta.scenes.len(), 2);
    assert_eq!(plan.meta.scenes[0].target_chars, Some(2000));

    let first = project
        .scene_text(&chapter, &plan.meta.scenes[0].id)
        .unwrap()
        .unwrap();
    assert!(
        first.starts_with("　雨が、傘の布"),
        "字下げされていない: {first}"
    );
    assert!(first.contains("「着きましたよ」"));
    let second = project
        .scene_text(&chapter, &plan.meta.scenes[1].id)
        .unwrap()
        .unwrap();
    assert!(second.contains("書斎の扉"));

    // 2 つ目のシーンのプロンプトには、前のシーンの要約と直前の本文が入る
    let requests = model.requests();
    let second_scene_prompt = &requests[12].messages[1].content;
    assert!(
        second_scene_prompt.contains(DIGEST1),
        "{second_scene_prompt}"
    );
    assert!(
        second_scene_prompt.contains("着きましたよ"),
        "{second_scene_prompt}"
    );
    assert!(second_scene_prompt.contains("閉ざされた書斎"));
}

#[tokio::test]
async fn beat_unit_saves_beats_to_the_chapter_plan() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_one_scene_chapter(&project);
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply([r#"{"beats": ["二人が坂を登る。", "洋館が見える。"]}"#]),
        Script::reply([prose(SCENE1_PART1)]),
        Script::reply([prose(SCENE1_PART2)]),
    ]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Beat)).unwrap();
    let chapter = ChapterId::from_number(1);
    let scene: SceneId = "s01".parse().unwrap();

    run(&engine, &project, &Task::Draft { chapter, scene }).await;

    let plan = project.chapter(&chapter).unwrap().unwrap();
    assert_eq!(
        plan.meta.scenes[0].beats,
        vec!["二人が坂を登る。", "洋館が見える。"]
    );
    let text = project.scene_text(&chapter, &scene).unwrap().unwrap();
    assert!(text.contains("坂道を登った") && text.contains("着きましたよ"));
    let second_beat_prompt = &model.requests()[2].messages[1].content;
    assert!(second_beat_prompt.contains("洋館が見える。"));
    assert!(second_beat_prompt.contains("2 部目"));
}

#[tokio::test]
async fn beat_planning_is_told_which_events_belong_to_later_scenes() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_two_scene_chapter(&project);
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply([r#"{"beats": ["二人が坂を登る。", "洋館が見える。"]}"#]),
        Script::reply([prose(SCENE1_PART1)]),
        Script::reply([prose(SCENE1_PART2)]),
    ]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Beat)).unwrap();
    let chapter = ChapterId::from_number(1);

    run(
        &engine,
        &project,
        &Task::Draft {
            chapter,
            scene: "s01".parse().unwrap(),
        },
    )
    .await;

    let beats_prompt = &model.requests()[0].messages[1].content;
    assert!(
        beats_prompt.contains("閉ざされた書斎（後のシーン）: 死体が見つかる。"),
        "{beats_prompt}"
    );
}

#[tokio::test]
async fn chapter_unit_splits_output_into_scene_files() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_two_scene_chapter(&project);
    let model = Arc::new(ScriptedChatModel::new([Script::reply([
        "雨が降っていた。\n\n◇\n\n書斎は静かだった。\n",
    ])]));
    let engine = Engine::new(model, settings(DraftUnit::Chapter)).unwrap();
    let chapter = ChapterId::from_number(1);

    run(
        &engine,
        &project,
        &Task::Draft {
            chapter,
            scene: "s01".parse().unwrap(),
        },
    )
    .await;

    assert_eq!(
        project
            .scene_text(&chapter, &"s01".parse().unwrap())
            .unwrap()
            .unwrap(),
        "　雨が降っていた。\n"
    );
    assert_eq!(
        project
            .scene_text(&chapter, &"s02".parse().unwrap())
            .unwrap()
            .unwrap(),
        "　書斎は静かだった。\n"
    );
}

#[tokio::test]
async fn revise_rewrites_a_document_following_the_instruction() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply([CONCEPT]),
        Script::reply(["了解しました。\n# 企画\n## ログライン\n吹雪の山荘で起きる事件。\n"]),
    ]));
    let engine = Engine::new(model, settings(DraftUnit::Scene)).unwrap();
    run(&engine, &project, &Task::Concept).await;

    let path = "concept.md".parse().unwrap();
    let task = Task::Revise {
        path,
        instruction: "舞台を雪山にして".into(),
    };
    let changes = engine
        .generate(&project, &task, &IgnoreEvents, &CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(changes.files.len(), 1);
    assert!(
        changes.files[0]
            .previous
            .as_deref()
            .unwrap()
            .contains("嵐の洋館")
    );
    assert_eq!(
        changes.files[0].content,
        "# 企画\n## ログライン\n吹雪の山荘で起きる事件。\n"
    );
}

#[tokio::test]
async fn applying_a_stale_change_set_is_rejected() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply([CONCEPT]),
        Script::reply([CONCEPT]),
    ]));
    let engine = Engine::new(model, settings(DraftUnit::Scene)).unwrap();
    let changes = engine
        .generate(
            &project,
            &Task::Concept,
            &IgnoreEvents,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    // 変更案を確認している間に、利用者が同じファイルを作った
    run(&engine, &project, &Task::Concept).await;

    assert!(changes.apply(&project).is_err());
}

#[tokio::test]
async fn cancelled_generation_changes_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    let model = Arc::new(ScriptedChatModel::new([Script::reply([CONCEPT])]));
    let engine = Engine::new(model, settings(DraftUnit::Scene)).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();

    let result = engine
        .generate(&project, &Task::Concept, &IgnoreEvents, &cancel)
        .await;

    assert!(matches!(
        result,
        Err(kataribe_engine::EngineError::Cancelled)
    ));
    assert!(
        project
            .markdown(&"concept.md".parse().unwrap())
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn edits_saved_during_generation_block_the_change_set() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    write(
        &project,
        "concept.md",
        "# 企画
嵐の洋館。
",
    );
    let model = Arc::new(ScriptedChatModel::new([Script::reply(["# 企画
雪の山荘。
"])]));
    let engine = Engine::new(model, settings(DraftUnit::Scene)).unwrap();
    let task = Task::Revise {
        path: RelPath::new("concept.md").unwrap(),
        instruction: "舞台を雪山に".into(),
    };
    let changes = engine
        .generate(&project, &task, &IgnoreEvents, &CancellationToken::new())
        .await
        .unwrap();

    // 変更案を確認している間に、利用者が同じファイルに書き足して保存した
    write(
        &project,
        "concept.md",
        "# 企画
嵐の洋館。
追記した段落。
",
    );

    assert!(changes.apply(&project).is_err());
    let concept = project
        .markdown(&RelPath::new("concept.md").unwrap())
        .unwrap()
        .unwrap();
    assert!(concept.body.contains("追記した段落"));
}

#[tokio::test]
async fn chapter_unit_from_a_middle_scene_asks_for_the_right_scenes() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_two_scene_chapter(&project);
    write(
        &project,
        "manuscript/01/s01.txt",
        "　雨が降っていた。
",
    );
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply(["二人は嵐の中、洋館に着いた。"]),
        Script::reply(["書斎は静かだった。
"]),
    ]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Chapter)).unwrap();
    let chapter = ChapterId::from_number(1);

    run(
        &engine,
        &project,
        &Task::Draft {
            chapter,
            scene: SceneId::new("s02").unwrap(),
        },
    )
    .await;

    // 1 回目は書き終えたシーン 1 の要約、2 回目が本文
    let prompt = &model.requests()[1].messages[1].content;
    assert!(prompt.contains("シーン 2 からシーン 2 まで"), "{prompt}");
    assert_eq!(
        project
            .scene_text(&chapter, &SceneId::new("s01").unwrap())
            .unwrap()
            .unwrap(),
        "　雨が降っていた。
"
    );
    assert_eq!(
        project
            .scene_text(&chapter, &SceneId::new("s02").unwrap())
            .unwrap()
            .unwrap(),
        "　書斎は静かだった。
"
    );
}

#[tokio::test]
async fn empty_output_never_replaces_existing_text() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_one_scene_chapter(&project);
    write(
        &project,
        "manuscript/01/s01.txt",
        "　書き上げた本文。
",
    );
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply(["以下が本文です。"]),
        Script::reply([""]),
        Script::reply(["承知しました。"]),
        Script::reply([""]),
    ]));
    let mut settings = settings(DraftUnit::Scene);
    settings.quality_retries = 1;
    let engine = Engine::new(model, settings).unwrap();
    let task = Task::Draft {
        chapter: ChapterId::from_number(1),
        scene: SceneId::new("s01").unwrap(),
    };

    let result = engine
        .generate(&project, &task, &IgnoreEvents, &CancellationToken::new())
        .await;

    assert!(matches!(
        result,
        Err(kataribe_engine::EngineError::InvalidOutput(_))
    ));
    let text = project
        .scene_text(&ChapterId::from_number(1), &SceneId::new("s01").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        text,
        "　書き上げた本文。
"
    );
}

#[tokio::test]
async fn truncated_revision_is_rejected() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    write(
        &project,
        "concept.md",
        "# 企画
嵐の洋館。
## 結末
凛が解く。
",
    );
    let model = Arc::new(ScriptedChatModel::new([Script::reply(["# 企画
雪の山荘。
"])
    .finish_reason(kataribe_llm::FinishReason::Length)]));
    let engine = Engine::new(model, settings(DraftUnit::Scene)).unwrap();
    let task = Task::Revise {
        path: RelPath::new("concept.md").unwrap(),
        instruction: "舞台を雪山に".into(),
    };

    let result = engine
        .generate(&project, &task, &IgnoreEvents, &CancellationToken::new())
        .await;

    assert!(matches!(
        result,
        Err(kataribe_engine::EngineError::InvalidOutput(_))
    ));
}

/// 文体ガイドと、1 シーンの章を用意する。
fn prepare_one_scene_chapter(project: &Project) {
    write(project, "style.md", STYLE);
    write(
        project,
        "plot/chapters/01.md",
        "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 嵐の中、二人が洋館に着く。\n    target_chars: 3000\n---\n凛と健二が洋館を訪れる。\n",
    );
}

/// 文体ガイドと、2 シーンの章を用意する。
fn prepare_two_scene_chapter(project: &Project) {
    write(project, "style.md", STYLE);
    write(
        project,
        "plot/chapters/01.md",
        "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 着く。\n    target_chars: 1000\n  - id: s02\n    title: 閉ざされた書斎\n    summary: 死体が見つかる。\n    target_chars: 1000\n---\n洋館を訪れる。\n",
    );
}

fn write(project: &Project, path: &str, content: &str) {
    project
        .store()
        .write_text(
            &path.parse().unwrap(),
            content,
            kataribe_project::WriteOptions {
                condition: kataribe_project::WriteCondition::Any,
                backup: kataribe_project::BackupMode::Never,
            },
        )
        .unwrap();
}

#[tokio::test]
async fn change_set_is_applied_only_to_the_project_it_was_made_for() {
    let (first_folder, second_folder) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let first = new_project(first_folder.path());
    let second = new_project(second_folder.path());
    let model = Arc::new(ScriptedChatModel::new([Script::reply([CONCEPT])]));
    let engine = Engine::new(model, settings(DraftUnit::Scene)).unwrap();
    let changes = engine
        .generate(
            &first,
            &Task::Concept,
            &IgnoreEvents,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let concept: RelPath = "concept.md".parse().unwrap();

    // 生成中に別の作品へ開き直した場合: 前の作品の変更案は書き込まない
    assert!(changes.apply(&second).is_err());
    assert!(second.store().read_text_opt(&concept).unwrap().is_none());

    changes.apply(&first).unwrap();
    assert!(first.store().read_text_opt(&concept).unwrap().is_some());
}

#[tokio::test]
async fn chapter_unit_stops_before_a_scene_the_author_already_wrote() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_two_scene_chapter(&project);
    write(
        &project,
        "manuscript/01/s02.txt",
        "　作家が手で書いた書斎の場面。\n",
    );
    let model = Arc::new(ScriptedChatModel::new([Script::reply([
        "雨が降っていた。\n",
    ])]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Chapter)).unwrap();
    let chapter = ChapterId::from_number(1);

    run(
        &engine,
        &project,
        &Task::Draft {
            chapter,
            scene: "s01".parse().unwrap(),
        },
    )
    .await;

    let scene_text = |scene: &str| {
        project
            .scene_text(&chapter, &scene.parse().unwrap())
            .unwrap()
            .unwrap()
    };
    assert_eq!(scene_text("s01"), "　雨が降っていた。\n");
    assert_eq!(scene_text("s02"), "　作家が手で書いた書斎の場面。\n");
    let prompt = &model.requests()[0].messages[1].content;
    assert!(prompt.contains("シーン 1 からシーン 1 まで"), "{prompt}");
}

#[test]
fn chapter_target_is_shown_only_when_every_scene_has_one() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    write(
        &project,
        "plot/chapters/01.md",
        "---\ntitle: 全部ある\nscenes:\n  - id: s01\n    title: 一\n    summary: 始まり。\n    target_chars: 1000\n  - id: s02\n    title: 二\n    summary: 続き。\n    target_chars: 1500\n---\n",
    );
    write(
        &project,
        "plot/chapters/02.md",
        "---\ntitle: 一部だけ\nscenes:\n  - id: s01\n    title: 一\n    summary: 始まり。\n    target_chars: 1000\n  - id: s02\n    title: 二\n    summary: 続き。\n    target_chars: 0\n---\n",
    );

    let overview = kataribe_engine::overview(&project).unwrap();
    let manuscript = overview
        .sections
        .iter()
        .find(|section| section.kind == kataribe_engine::SectionKind::Manuscript)
        .unwrap();
    let targets: Vec<(Option<u32>, Vec<Option<u32>>)> = manuscript
        .entries
        .iter()
        .map(|chapter| {
            let scenes = chapter.children.iter().map(|scene| scene.target_chars);
            (chapter.target_chars, scenes.collect())
        })
        .collect();

    assert_eq!(
        targets,
        vec![
            (Some(2500), vec![Some(1000), Some(1500)]),
            (None, vec![Some(1000), None]),
        ]
    );
}

#[tokio::test]
async fn chapter_unit_writes_every_unwritten_scene_up_to_the_next_written_one() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    write(&project, "style.md", STYLE);
    write(
        &project,
        "plot/chapters/01.md",
        "---\ntitle: 雨の匂い\nscenes:\n  - id: s01\n    title: 洋館への道\n    summary: 着く。\n    target_chars: 1000\n  - id: s02\n    title: 閉ざされた書斎\n    summary: 死体が見つかる。\n    target_chars: 1000\n  - id: s03\n    title: 夜明け\n    summary: 嵐がやむ。\n    target_chars: 1000\n---\n洋館を訪れる。\n",
    );
    write(
        &project,
        "manuscript/01/s03.txt",
        "　作家が手で書いた夜明けの場面。\n",
    );
    let model = Arc::new(ScriptedChatModel::new([Script::reply([
        "雨が降っていた。\n\n◇\n\n書斎は静かだった。\n",
    ])]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Chapter)).unwrap();
    let chapter = ChapterId::from_number(1);

    run(
        &engine,
        &project,
        &Task::Draft {
            chapter,
            scene: "s01".parse().unwrap(),
        },
    )
    .await;

    let scene_text = |scene: &str| {
        project
            .scene_text(&chapter, &scene.parse().unwrap())
            .unwrap()
            .unwrap()
    };
    assert_eq!(scene_text("s01"), "　雨が降っていた。\n");
    assert_eq!(scene_text("s02"), "　書斎は静かだった。\n");
    assert_eq!(scene_text("s03"), "　作家が手で書いた夜明けの場面。\n");
    let prompt = &model.requests()[0].messages[1].content;
    assert!(prompt.contains("シーン 1 からシーン 2 まで"), "{prompt}");
}

#[tokio::test]
async fn phrases_repeated_in_earlier_scenes_are_named_so_the_next_scene_avoids_them() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_two_scene_chapter(&project);
    write(
        &project,
        "manuscript/01/s01.txt",
        "　嵐の咆哮が屋根を叩いた。\n　嵐の咆哮の合間に足音がした。\n　また嵐の咆哮が窓を揺らした。\n",
    );
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply(["二人は嵐の中、洋館に着いた。"]),
        Script::reply(["書斎は静かだった。\n"]),
    ]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Chapter)).unwrap();

    run(
        &engine,
        &project,
        &Task::Draft {
            chapter: ChapterId::from_number(1),
            scene: SceneId::new("s02").unwrap(),
        },
    )
    .await;

    // 1 回目は書き終えたシーン 1 の要約、2 回目が本文
    let prompt = &model.requests()[1].messages[1].content;
    assert!(
        prompt.contains("繰り返し使っている表現") && prompt.contains("「嵐の咆哮」"),
        "{prompt}"
    );
}

#[tokio::test]
async fn nothing_is_named_when_no_phrase_is_overused() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    prepare_one_scene_chapter(&project);
    let model = Arc::new(ScriptedChatModel::new([Script::reply([prose(
        SCENE1_PART1,
    )])]));
    let engine = Engine::new(model.clone(), settings(DraftUnit::Chapter)).unwrap();

    run(
        &engine,
        &project,
        &Task::Draft {
            chapter: ChapterId::from_number(1),
            scene: "s01".parse().unwrap(),
        },
    )
    .await;

    let prompt = &model.requests()[0].messages[1].content;
    assert!(!prompt.contains("繰り返し使っている表現"), "{prompt}");
}

#[tokio::test]
async fn a_cast_with_romanized_names_is_generated_again() {
    let folder = tempfile::tempdir().unwrap();
    let project = new_project(folder.path());
    write(&project, "concept.md", CONCEPT);
    write(&project, "world/overview.md", WORLD);
    let romanized = r#"{"characters": [
  {"id": "kirishima-rin", "name": "霧島 凛", "reading": "きりしま りん", "role": "主人公", "summary": "盲目の少女探偵。"},
  {"id": "tanaka-shukichi", "name": "田中 Shukichi", "reading": "たなか しゅうきち", "role": "執事", "summary": "館の執事。"}
]}"#;
    let model = Arc::new(ScriptedChatModel::new([
        Script::reply([romanized]),
        Script::reply([CAST]),
    ]));
    let settings = GenerationSettings {
        quality_retries: 1,
        ..settings(DraftUnit::Beat)
    };
    let engine = Engine::new(model.clone(), settings).unwrap();

    run(&engine, &project, &Task::Cast).await;

    let names: Vec<String> = project
        .characters()
        .unwrap()
        .into_iter()
        .map(|character| character.meta.name)
        .collect();
    assert_eq!(names, vec!["霧島 凛", "佐藤 健二"]);
    assert_eq!(model.requests().len(), 2);
}
