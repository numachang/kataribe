//! 本文生成のプロンプトを、モデルの文脈に収まるように組み立てる。

use kataribe_project::{Character, ScenePlan};
use minijinja::{Value, context};
use serde::Serialize;

use super::digest::StoryEntry;
use super::story::{Position, Story};
use crate::error::Result;
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::prompt::{Prompt, PromptTemplate};
use crate::settings::DraftUnit;
use crate::stages::Stage;
use crate::stages::materials::CharacterBrief;

/// 文体ガイドのうち、切り詰めても残したい節（見出しに含まれる語）。
const STYLE_ESSENTIALS: &[&str] = &["文体見本", "人称", "視点", "避けること"];
/// 人物資料のうち、切り詰めても残したい節。
const PROFILE_ESSENTIALS: &[&str] = &["口調", "性格"];
/// 指示の直前で繰り返す、文体ガイドの人称の節（見出しに含まれる語）とその上限。
/// 小さなモデルは長い文脈の途中にある人称の指定を見落とし、視点人物の一人称で書きがちなため。
const NARRATION_HEADING: &str = "人称";
const NARRATION_CHARS: usize = 400;

/// 資料ごとの文字数の上限。上の段から順に試し、文脈に収まった段を使う。
/// 優先度の低い資料（世界観 → これまでの要約 → 人物の詳細 → 直前の本文 → 文体）から先に減らす。
#[derive(Debug, Clone, Copy)]
struct Caps {
    style: usize,
    world: usize,
    profile: usize,
    story_entries: usize,
    previous_text: usize,
}

const LEVELS: [Caps; 6] = [
    Caps {
        style: 2500,
        world: 1500,
        profile: 900,
        story_entries: 12,
        previous_text: 1500,
    },
    Caps {
        style: 2500,
        world: 600,
        profile: 900,
        story_entries: 8,
        previous_text: 1500,
    },
    Caps {
        style: 2500,
        world: 0,
        profile: 600,
        story_entries: 5,
        previous_text: 1500,
    },
    Caps {
        style: 2000,
        world: 0,
        profile: 400,
        story_entries: 3,
        previous_text: 1000,
    },
    Caps {
        style: 1500,
        world: 0,
        profile: 250,
        story_entries: 2,
        previous_text: 800,
    },
    Caps {
        style: 1000,
        world: 0,
        profile: 150,
        story_entries: 1,
        previous_text: 500,
    },
];

/// 1 回の生成で書かせる範囲。
#[derive(Debug, Clone)]
pub(crate) struct Assignment<'a> {
    pub target_chars: u32,
    /// 1 シーンを何回に分けて書くか（1 始まり）。
    pub part_index: u32,
    pub part_total: u32,
    pub beat: Option<Beat<'a>>,
    /// 前のシーンの本文（参考として末尾だけを渡す）。
    pub previous_scene: &'a str,
    /// このシーンの書きかけの本文（この続きから書かせる）。
    pub written_so_far: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Beat<'a> {
    pub index: usize,
    pub total: usize,
    pub text: &'a str,
    pub next: Option<&'a str>,
}

/// 本文生成のプロンプトの材料（切り詰める前）。
pub(crate) struct DraftMaterial<'a> {
    pub story: &'a Story,
    pub position: Position,
    pub unit: DraftUnit,
    /// 今回書くシーンの範囲（章の中の添字）。章単位なら複数のシーンになる。
    pub scenes: std::ops::Range<usize>,
    pub story_so_far: &'a [StoryEntry],
}

#[derive(Debug, Serialize)]
struct PlannedScene<'a> {
    title: &'a str,
    summary: &'a str,
    status: &'static str,
    pov: &'a Option<String>,
    place: &'a Option<String>,
    time: &'a Option<String>,
    characters: &'a [String],
}

impl DraftMaterial<'_> {
    /// 文脈の予算に収まる最も詳しいプロンプトを返す。どうしても収まらなければ、最も短いものを返して警告する。
    pub fn render(
        &self,
        stage: &Stage<'_>,
        assignment: &Assignment<'_>,
        output_tokens: u32,
    ) -> Result<Prompt> {
        let budget = stage.prompt_budget(output_tokens);
        let mut prompt = self.render_with(stage, assignment, LEVELS[0])?;
        for caps in &LEVELS[1..] {
            if prompt.estimated_tokens() <= budget {
                return Ok(prompt);
            }
            prompt = self.render_with(stage, assignment, *caps)?;
        }
        if prompt.estimated_tokens() > budget {
            notice(
                stage.caller.sink(),
                NoticeLevel::Warning,
                "資料を切り詰めてもモデルの文脈に収まりません。設定の「文脈の長さ」を増やすか、「1 回あたりの文字数」を減らしてください。",
            );
        }
        Ok(prompt)
    }

    fn render_with(
        &self,
        stage: &Stage<'_>,
        assignment: &Assignment<'_>,
        caps: Caps,
    ) -> Result<Prompt> {
        let story_start = self.story_so_far.len().saturating_sub(caps.story_entries);
        stage.prompts.render(
            PromptTemplate::Draft,
            &context! {
                project => &stage.info,
                style => excerpt::prioritized(&self.story.style, STYLE_ESSENTIALS, caps.style),
                narration => excerpt::section(&self.story.style, NARRATION_HEADING, NARRATION_CHARS),
                world => excerpt::head(&self.story.world, caps.world),
                characters => self.cast(caps.profile),
                story_so_far => &self.story_so_far[story_start..],
                chapter => self.chapter_context(),
                scene => self.scene_context(),
                unit => unit_name(self.unit),
                part => context! {
                    index => assignment.part_index,
                    total => assignment.part_total,
                    label => part_label(assignment.part_index, assignment.part_total),
                },
                beat => &assignment.beat,
                target_chars => assignment.target_chars,
                previous_scene_ending => excerpt::tail(assignment.previous_scene, caps.previous_text),
                written_so_far => excerpt::tail(assignment.written_so_far, caps.previous_text),
            },
        )
    }

    fn current_scenes(&self) -> Vec<&ScenePlan> {
        self.story.chapter(self.position).meta.scenes[self.scenes.clone()]
            .iter()
            .collect()
    }

    fn cast(&self, profile_chars: usize) -> Vec<CharacterBrief> {
        let characters: Vec<&Character> = self.story.cast_of(&self.current_scenes());
        characters
            .into_iter()
            .map(|character| CharacterBrief {
                profile: excerpt::prioritized(&character.body, PROFILE_ESSENTIALS, profile_chars),
                ..CharacterBrief::new(character, 0)
            })
            .collect()
    }

    fn chapter_context(&self) -> Value {
        let chapter = self.story.chapter(self.position);
        let current = self.scenes.clone();
        let scenes: Vec<PlannedScene<'_>> = chapter
            .meta
            .scenes
            .iter()
            .enumerate()
            .map(|(index, scene)| PlannedScene {
                title: &scene.title,
                summary: &scene.summary,
                status: if current.contains(&index) {
                    "current"
                } else if index < current.start {
                    "done"
                } else {
                    "upcoming"
                },
                pov: &scene.pov,
                place: &scene.place,
                time: &scene.time,
                characters: &scene.characters,
            })
            .collect();
        context! {
            title => &chapter.meta.title,
            storyline => chapter.storyline.trim(),
            scenes => scenes,
            first_current => current.start + 1,
            last_current => current.end,
        }
    }

    fn scene_context(&self) -> Value {
        if self.unit == DraftUnit::Chapter {
            return Value::from(());
        }
        let scene = self.story.scene(self.position);
        context! {
            title => &scene.title,
            summary => &scene.summary,
            pov => &scene.pov,
            place => &scene.place,
            time => &scene.time,
            characters => &scene.characters,
        }
    }
}

fn unit_name(unit: DraftUnit) -> &'static str {
    match unit {
        DraftUnit::Chapter => "chapter",
        DraftUnit::Scene => "scene",
        DraftUnit::Beat => "beat",
    }
}

/// 1 シーンを分けて書くときの、今回の範囲の呼び方。小さなモデルにも伝わる言葉にする。
fn part_label(index: u32, total: u32) -> String {
    match (total, index) {
        (2, 1) => "前半".to_owned(),
        (2, _) => "後半（シーンの終わりまで）".to_owned(),
        (3, 1) => "序盤".to_owned(),
        (3, 2) => "中盤".to_owned(),
        (3, _) => "終盤（シーンの終わりまで）".to_owned(),
        _ => format!("{total} 分の {index} の部分"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_labels_use_plain_words() {
        assert_eq!(part_label(1, 2), "前半");
        assert_eq!(part_label(3, 3), "終盤（シーンの終わりまで）");
        assert_eq!(part_label(2, 5), "5 分の 2 の部分");
    }

    #[test]
    fn levels_shrink_lower_priority_material_first() {
        for pair in LEVELS.windows(2) {
            let (wider, narrower) = (pair[0], pair[1]);
            assert!(narrower.world <= wider.world);
            assert!(narrower.story_entries <= wider.story_entries);
            assert!(narrower.profile <= wider.profile);
            assert!(narrower.previous_text <= wider.previous_text);
            assert!(narrower.style <= wider.style);
        }
        // 文体は、世界観が無くなるまで削らない
        let first_style_cut = LEVELS
            .iter()
            .position(|caps| caps.style < LEVELS[0].style)
            .unwrap();
        assert_eq!(LEVELS[first_style_cut - 1].world, 0);
    }
}
