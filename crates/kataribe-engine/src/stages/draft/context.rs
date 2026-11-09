//! 本文生成のプロンプトを、モデルの文脈に収まるように組み立てる。

use kataribe_project::{Character, ScenePlan};
use kataribe_text::repetition::{OverusedOptions, overused_phrases};
use minijinja::{Value, context};
use serde::Serialize;

use super::digest::StoryEntry;
use super::story::{Position, Story, VerbatimScene};
use crate::error::Result;
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::prompt::{Prompt, PromptTemplate};
use crate::settings::DraftUnit;
use crate::stages::Stage;
use crate::stages::materials::CharacterBrief;
use crate::stages::references;

/// 文体ガイドのうち、切り詰めても残したい節（見出しに含まれる語）。
const STYLE_ESSENTIALS: &[&str] = &["文体見本", "人称", "視点", "避けること"];
/// 人物資料のうち、切り詰めても残したい節。
const PROFILE_ESSENTIALS: &[&str] = &["口調", "性格"];
/// 指示の直前で繰り返す、文体ガイドの人称の節（見出しに含まれる語）とその上限。
/// 小さなモデルは長い文脈の途中にある人称の指定を見落とし、視点人物の一人称で書きがちなため。
const NARRATION_HEADING: &str = "人称";
const NARRATION_CHARS: usize = 400;
/// 「使いすぎ」とみなす回数と、プロンプトで避けさせる表現の数。
/// ローカル LLM は前のシーンの決まり文句を場面をまたいで繰り返しがちなので、機械的に拾って伝える。
const OVERUSED_MIN_COUNT: usize = 3;
const MAX_OVERUSED_PHRASES: usize = 8;
/// この長さ以上の文脈を持つモデルには、資料を広く渡し、書き終えた本文をそのまま渡す。
/// ローカル LLM の 16,384 トークンで実測して決めた `LEVELS` の動きを変えないよう、それより十分に大きい値にしている。
const WIDE_CONTEXT_TOKENS: u32 = 32_000;
/// そのまま渡す本文を決めるときに、資料（文体ガイド・人物・シーンの設計など）のために残すトークン数。
/// `LEVELS` の上のほうの段が入る大きさにして、本文を増やしたせいで資料が今より減らないようにする。
const MATERIAL_RESERVE_TOKENS: usize = 8_000;

/// 資料ごとの文字数の上限。上の段から順に試し、文脈に収まった段を使う。
/// 優先度の低い資料（世界観 → これまでの要約 → 人物の詳細 → 直前の本文 → 文体）から先に減らす。
#[derive(Debug, Clone, Copy)]
struct Caps {
    style: usize,
    world: usize,
    profile: usize,
    story_entries: usize,
    previous_text: usize,
    /// 文体の手本（参考文の抜粋）の合計字数。文体ガイドより先に減らす。
    references: usize,
}

const LEVELS: [Caps; 6] = [
    Caps {
        style: 2500,
        world: 1500,
        profile: 900,
        story_entries: 12,
        previous_text: 1500,
        references: 0,
    },
    Caps {
        style: 2500,
        world: 600,
        profile: 900,
        story_entries: 8,
        previous_text: 1500,
        references: 0,
    },
    Caps {
        style: 2500,
        world: 0,
        profile: 600,
        story_entries: 5,
        previous_text: 1500,
        references: 0,
    },
    Caps {
        style: 2000,
        world: 0,
        profile: 400,
        story_entries: 3,
        previous_text: 1000,
        references: 0,
    },
    Caps {
        style: 1500,
        world: 0,
        profile: 250,
        story_entries: 2,
        previous_text: 800,
        references: 0,
    },
    Caps {
        style: 1000,
        world: 0,
        profile: 150,
        story_entries: 1,
        previous_text: 500,
        references: 0,
    },
];

/// 長い文脈を持つモデルで、`LEVELS` より先に試す段。段を下げるときは、資料より先に文体の手本を減らす。
const WIDE_LEVELS: [Caps; 4] = [
    Caps {
        references: 20_000,
        ..WIDEST_MATERIALS
    },
    Caps {
        references: 6_000,
        ..WIDEST_MATERIALS
    },
    WIDEST_MATERIALS,
    Caps {
        style: 8_000,
        world: 8_000,
        profile: 3_000,
        story_entries: 30,
        previous_text: 8_000,
        references: 0,
    },
];

/// 長い文脈のモデルに、文体の手本を含めずに渡す資料の上限。
const WIDEST_MATERIALS: Caps = Caps {
    style: 30_000,
    world: 30_000,
    profile: 8_000,
    story_entries: usize::MAX,
    previous_text: 30_000,
    references: 0,
};

const NARROWEST_LEVEL: Caps = LEVELS[LEVELS.len() - 1];

/// 文脈の長さに応じて、上から順に試す段。
fn levels_for(context_tokens: u32) -> impl Iterator<Item = Caps> {
    let wide: &[Caps] = if context_tokens >= WIDE_CONTEXT_TOKENS {
        &WIDE_LEVELS
    } else {
        &[]
    };
    wide.iter().chain(&LEVELS).copied()
}

/// 書き終えた本文をそのまま渡すのに使ってよいトークン数。文脈の半分を上限に、出力と資料の分を空けておく。
/// 要約を作るかどうかを先に決めるため、資料の段を選ぶ前に求める。
///
/// `prompt_budget` は、今回の出力を確保したあとにプロンプトへ使えるトークン数。章単位のように出力が長いときに
/// 本文で埋めると、資料を削っても収まらず、出力の上限が切り詰められて章が途中で切れる。
pub(crate) fn verbatim_prose_tokens(context_tokens: u32, prompt_budget: usize) -> usize {
    if context_tokens < WIDE_CONTEXT_TOKENS {
        return 0;
    }
    (context_tokens as usize / 2).min(prompt_budget.saturating_sub(MATERIAL_RESERVE_TOKENS))
}

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
    /// 要約せずそのまま渡す、直前までのシーンの本文（読む順）。
    pub verbatim_scenes: &'a [VerbatimScene],
    /// このシーンより前の本文の末尾（使いすぎの表現を探すのに使う）。
    pub recent_prose: &'a str,
    /// 文体の手本にする参考文（`references/`）。
    pub references: &'a [String],
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
        let overused = self.overused_phrases(assignment.written_so_far);
        for caps in levels_for(stage.settings.context_tokens) {
            let prompt = self.render_with(stage, assignment, &overused, caps)?;
            if prompt.estimated_tokens() <= budget {
                return Ok(prompt);
            }
        }
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            "資料を切り詰めてもモデルの文脈に収まりません。設定の「文脈の長さ」を増やすか、「1 回あたりの文字数」を減らしてください。",
        );
        self.render_with(stage, assignment, &overused, NARROWEST_LEVEL)
    }

    /// 直前までの本文と、このシーンの書きかけの本文で、繰り返し使っている表現。
    fn overused_phrases(&self, written_so_far: &str) -> Vec<String> {
        let text = format!("{}\n{written_so_far}", self.recent_prose);
        let names = self.story.character_name_words();
        overused_phrases(
            &text,
            &OverusedOptions {
                min_count: OVERUSED_MIN_COUNT,
                max_phrases: MAX_OVERUSED_PHRASES,
                ignored_words: &names,
            },
        )
        .into_iter()
        .map(|found| found.phrase)
        .collect()
    }

    fn render_with(
        &self,
        stage: &Stage<'_>,
        assignment: &Assignment<'_>,
        overused: &[String],
        caps: Caps,
    ) -> Result<Prompt> {
        let story_start = self.story_so_far.len().saturating_sub(caps.story_entries);
        // そのまま渡す本文の末尾は前のシーンそのものなので、二重に渡さない
        let previous_scene_ending = if self.verbatim_scenes.is_empty() {
            excerpt::tail(assignment.previous_scene, caps.previous_text)
        } else {
            String::new()
        };
        stage.prompts.render(
            PromptTemplate::Draft,
            &context! {
                project => &stage.info,
                style => excerpt::prioritized(&self.story.style, STYLE_ESSENTIALS, caps.style),
                narration => excerpt::section(&self.story.style, NARRATION_HEADING, NARRATION_CHARS),
                overused => overused,
                world => excerpt::head(&self.story.world, caps.world),
                characters => self.cast(caps.profile),
                story_so_far => &self.story_so_far[story_start..],
                verbatim_scenes => self.verbatim_scenes,
                style_examples => references::excerpts(self.references, caps.references),
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
                previous_scene_ending => previous_scene_ending,
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
    fn short_contexts_use_only_the_measured_levels_and_get_no_verbatim_prose() {
        let below_wide = WIDE_CONTEXT_TOKENS - 1;
        assert_eq!(levels_for(below_wide).count(), LEVELS.len());
        assert!(levels_for(below_wide).all(|caps| caps.style <= LEVELS[0].style));
        assert_eq!(verbatim_prose_tokens(below_wide, 100_000), 0);
    }

    #[test]
    fn long_contexts_try_the_wide_levels_first_and_keep_half_for_other_material() {
        assert_eq!(
            levels_for(WIDE_CONTEXT_TOKENS).count(),
            WIDE_LEVELS.len() + LEVELS.len()
        );
        assert_eq!(verbatim_prose_tokens(100_000, 90_000), 50_000);
    }

    #[test]
    fn verbatim_prose_gives_way_to_a_long_output_and_keeps_room_for_the_materials() {
        assert_eq!(verbatim_prose_tokens(32_000, 15_000), 7_000);
        assert_eq!(verbatim_prose_tokens(32_000, 5_000), 0);
    }

    #[test]
    fn wide_levels_narrow_down_into_the_measured_levels() {
        let all: Vec<Caps> = levels_for(WIDE_CONTEXT_TOKENS).collect();
        for pair in all.windows(2) {
            let (wider, narrower) = (pair[0], pair[1]);
            assert!(narrower.references <= wider.references);
            assert!(narrower.style <= wider.style);
            assert!(narrower.world <= wider.world);
            assert!(narrower.profile <= wider.profile);
            assert!(narrower.story_entries <= wider.story_entries);
            assert!(narrower.previous_text <= wider.previous_text);
        }
    }

    #[test]
    fn the_measured_levels_never_carry_style_examples() {
        assert!(LEVELS.iter().all(|caps| caps.references == 0));
    }

    #[test]
    fn style_examples_are_dropped_before_any_material_is_narrowed() {
        let [widest, reduced, without_examples, ..] = WIDE_LEVELS;
        assert!(widest.references > reduced.references && reduced.references > 0);
        assert_eq!(without_examples.references, 0);
        for level in [widest, reduced] {
            assert_eq!(level.style, without_examples.style);
            assert_eq!(level.profile, without_examples.profile);
            assert_eq!(level.previous_text, without_examples.previous_text);
        }
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
