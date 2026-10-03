//! `generate` サブコマンドの `TASK` 引数のミニ言語。
//!
//! `concept` のような単独の語、または `character:<id>` のような
//! `種類:引数` の形を解析し、[`kataribe_engine::Task`] に変換する。

use std::str::FromStr;

use kataribe_engine::Task;
use kataribe_project::{ChapterId, CharacterId, RelPath, SceneId};

/// コマンドラインの `TASK` 引数を解析した結果。
///
/// `Task::Revise` の `instruction` はここには含まれない
/// （`--instruction` オプションから別に受け取るため）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSpec {
    Concept,
    Style,
    World,
    Cast,
    Character(CharacterId),
    Synopsis,
    Outline,
    ScenePlan(ChapterId),
    Draft { chapter: ChapterId, scene: SceneId },
    Revise(RelPath),
}

impl TaskSpec {
    /// `--instruction` の値と合わせて [`Task`] に変換する。
    #[must_use]
    pub fn into_task(self, instruction: Option<String>) -> Task {
        match self {
            TaskSpec::Concept => Task::Concept,
            TaskSpec::Style => Task::Style,
            TaskSpec::World => Task::World,
            TaskSpec::Cast => Task::Cast,
            TaskSpec::Character(id) => Task::Character { id },
            TaskSpec::Synopsis => Task::Synopsis,
            TaskSpec::Outline => Task::Outline,
            TaskSpec::ScenePlan(chapter) => Task::ScenePlan { chapter },
            TaskSpec::Draft { chapter, scene } => Task::Draft { chapter, scene },
            TaskSpec::Revise(path) => Task::Revise {
                path,
                instruction: instruction.unwrap_or_default(),
            },
        }
    }
}

impl FromStr for TaskSpec {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "concept" => Ok(TaskSpec::Concept),
            "style" => Ok(TaskSpec::Style),
            "world" => Ok(TaskSpec::World),
            "cast" => Ok(TaskSpec::Cast),
            "synopsis" => Ok(TaskSpec::Synopsis),
            "outline" => Ok(TaskSpec::Outline),
            _ => parse_prefixed(input),
        }
    }
}

fn parse_prefixed(input: &str) -> Result<TaskSpec, String> {
    if let Some(id) = input.strip_prefix("character:") {
        return CharacterId::new(id)
            .map(TaskSpec::Character)
            .map_err(|error| error.to_string());
    }
    if let Some(number) = input.strip_prefix("scenes:") {
        return ChapterId::new(number)
            .map(TaskSpec::ScenePlan)
            .map_err(|error| error.to_string());
    }
    if let Some(rest) = input.strip_prefix("draft:") {
        return parse_draft(rest);
    }
    if let Some(path) = input.strip_prefix("revise:") {
        return RelPath::new(path)
            .map(TaskSpec::Revise)
            .map_err(|error| error.to_string());
    }
    Err(format!(
        "不明な工程です: {input}\n\
         次のいずれかを指定してください: concept | style | world | cast | character:<id> \
         | synopsis | outline | scenes:<NN> | draft:<NN>/<sNN> | revise:<path>"
    ))
}

fn parse_draft(rest: &str) -> Result<TaskSpec, String> {
    let (chapter, scene) = parse_chapter_and_scene(rest, "draft")?;
    Ok(TaskSpec::Draft { chapter, scene })
}

/// `<章番号>/<シーン ID>`（例: `01/s02`）を解析する。`prefix` は、指定の頭に付けた種類の名前
/// （`draft:01/s02` なら `draft`）で、誤りのメッセージに使う。`generate` の `draft:` と `remove` の `scene:` が共有する。
pub(crate) fn parse_chapter_and_scene(
    rest: &str,
    prefix: &str,
) -> Result<(ChapterId, SceneId), String> {
    let (chapter, scene) = rest.split_once('/').ok_or_else(|| {
        format!(
            "{prefix} の指定は <章番号>/<シーン ID> の形にしてください（例: {prefix}:01/s01）: {prefix}:{rest}"
        )
    })?;
    let chapter = ChapterId::new(chapter).map_err(|error| error.to_string())?;
    let scene = SceneId::new(scene).map_err(|error| error.to_string())?;
    Ok((chapter, scene))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_words() {
        assert_eq!("concept".parse(), Ok(TaskSpec::Concept));
        assert_eq!("style".parse(), Ok(TaskSpec::Style));
        assert_eq!("world".parse(), Ok(TaskSpec::World));
        assert_eq!("cast".parse(), Ok(TaskSpec::Cast));
        assert_eq!("synopsis".parse(), Ok(TaskSpec::Synopsis));
        assert_eq!("outline".parse(), Ok(TaskSpec::Outline));
    }

    #[test]
    fn parses_character_with_id() {
        let spec: TaskSpec = "character:kirishima-rin".parse().unwrap();
        assert_eq!(
            spec,
            TaskSpec::Character(CharacterId::new("kirishima-rin").unwrap())
        );
    }

    #[test]
    fn parses_scenes_with_chapter_number() {
        let spec: TaskSpec = "scenes:01".parse().unwrap();
        assert_eq!(spec, TaskSpec::ScenePlan(ChapterId::from_number(1)));
    }

    #[test]
    fn parses_draft_with_chapter_and_scene() {
        let spec: TaskSpec = "draft:01/s02".parse().unwrap();
        assert_eq!(
            spec,
            TaskSpec::Draft {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
            }
        );
    }

    #[test]
    fn parses_revise_with_project_relative_path() {
        let spec: TaskSpec = "revise:concept.md".parse().unwrap();
        assert_eq!(spec, TaskSpec::Revise(RelPath::new("concept.md").unwrap()));
    }

    #[test]
    fn rejects_unknown_task_names() {
        assert!("no-such-task".parse::<TaskSpec>().is_err());
    }

    #[test]
    fn rejects_draft_without_a_slash() {
        assert!("draft:01".parse::<TaskSpec>().is_err());
    }

    #[test]
    fn rejects_draft_with_invalid_chapter_number() {
        assert!("draft:1/s01".parse::<TaskSpec>().is_err());
    }

    #[test]
    fn rejects_draft_with_invalid_scene_id() {
        assert!("draft:01/01".parse::<TaskSpec>().is_err(), "s が要る");
    }

    #[test]
    fn rejects_revise_with_path_outside_the_project() {
        assert!("revise:../escape".parse::<TaskSpec>().is_err());
    }

    #[test]
    fn rejects_character_with_uppercase_id() {
        assert!("character:Rin".parse::<TaskSpec>().is_err());
    }

    #[test]
    fn into_task_carries_the_instruction_only_for_revise() {
        let task = TaskSpec::Revise(RelPath::new("concept.md").unwrap())
            .into_task(Some("もっと短く".to_owned()));
        assert_eq!(
            task,
            Task::Revise {
                path: RelPath::new("concept.md").unwrap(),
                instruction: "もっと短く".to_owned(),
            }
        );

        let task = TaskSpec::Concept.into_task(Some("無視されるはず".to_owned()));
        assert_eq!(task, Task::Concept);
    }
}
