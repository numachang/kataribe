//! 本文の生成単位（シーン・ビート・章）ごとの書き進め方。

use kataribe_project::{Chapter, ChapterMeta, TextFile, layout};
use minijinja::context;
use schemars::JsonSchema;
use serde::Deserialize;

use super::context::{Assignment, Beat, DraftMaterial};
use super::prose::{self, ProseJob, inspect};
use super::story::{Position, Story};
use crate::change_set::ChangeSet;
use crate::cleanup::{clean_prose, drop_unfinished_sentence, split_scenes};
use crate::error::{EngineError, Result};
use crate::events::{NoticeLevel, notice};
use crate::prompt::PromptTemplate;
use crate::stages::Stage;
use crate::stages::materials::{chapter_label, round_to_hundreds, snapshot_chapter};

const BEATS_OUTPUT_TOKENS: u32 = 2048;
const MIN_BEATS: u32 = 2;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BeatsOutput {
    beats: Vec<String>,
}

/// 1 シーンずつ書く。`chars_per_call` を超えるシーンは、何回かに分けて書き継ぐ。
pub(super) async fn by_scene(stage: &Stage<'_>, material: &DraftMaterial<'_>) -> Result<ChangeSet> {
    let (story, position) = (material.story, material.position);
    let base = scene_snapshot(stage, story, position)?;
    let target = story.scene_target(position, &stage.manifest);
    let parts = target.div_ceil(stage.settings.chars_per_call).max(1);
    let chars_per_part = round_to_hundreds(target / parts);
    let previous_scene = story.previous_text(stage.project, position)?;
    stage.caller.expect_steps(parts);

    let mut text = String::new();
    for part in 1..=parts {
        let assignment = Assignment {
            target_chars: chars_per_part,
            part_index: part,
            part_total: parts,
            beat: None,
            previous_scene: &previous_scene,
            written_so_far: &text,
        };
        let written = write_part(
            stage,
            material,
            &assignment,
            progress_label(story, position, part, parts),
        )
        .await?;
        text = join_paragraphs(&text, &written);
    }
    let mut changes = ChangeSet::new(summary(story, position));
    put_scene(stage, story, position, text, base, &mut changes);
    Ok(changes)
}

/// シーンを展開（ビート）に分け、1 ビートずつ書く。ビートは章のファイルに保存し、書き直しでも使い回す。
pub(super) async fn by_beat(stage: &Stage<'_>, material: &DraftMaterial<'_>) -> Result<ChangeSet> {
    let (story, position) = (material.story, material.position);
    let base = scene_snapshot(stage, story, position)?;
    let target = story.scene_target(position, &stage.manifest);
    let mut changes = ChangeSet::new(summary(story, position));
    let beats = match story.scene(position).beats.clone() {
        beats if !beats.is_empty() => beats,
        _ => {
            let (chapter, chapter_base) =
                snapshot_chapter(stage.project, story.chapter(position).id)?;
            let beats = plan_beats(stage, story, position, target).await?;
            let updated = with_beats(chapter, position.scene, beats.clone());
            changes.put(
                layout::chapter_path(&updated.id),
                updated.render()?,
                Some(chapter_base),
            );
            beats
        }
    };
    let beat_count = u32::try_from(beats.len()).unwrap_or(u32::MAX);
    let chars_per_beat = round_to_hundreds(target / beat_count.max(1));
    let previous_scene = story.previous_text(stage.project, position)?;
    stage.caller.expect_steps(beat_count);

    let mut text = String::new();
    for (index, beat) in beats.iter().enumerate() {
        let assignment = Assignment {
            target_chars: chars_per_beat,
            part_index: 1,
            part_total: 1,
            beat: Some(Beat {
                index: index + 1,
                total: beats.len(),
                text: beat,
                next: beats.get(index + 1).map(String::as_str),
            }),
            previous_scene: &previous_scene,
            written_so_far: &text,
        };
        let part = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let written = write_part(
            stage,
            material,
            &assignment,
            progress_label(story, position, part, beat_count),
        )
        .await?;
        text = join_paragraphs(&text, &written);
    }
    put_scene(stage, story, position, text, base, &mut changes);
    Ok(changes)
}

/// このシーンから、次に本文のあるシーンの手前までを 1 回で書かせ、区切り行でシーンごとに分ける。
/// 区切りの数がシーンの数と合わない出力は、どこが何のシーンか分からないので書き込まない。
pub(super) async fn by_chapter(
    stage: &Stage<'_>,
    material: &DraftMaterial<'_>,
) -> Result<ChangeSet> {
    let (story, position) = (material.story, material.position);
    let positions: Vec<Position> = material
        .scenes
        .clone()
        .map(|scene| Position { scene, ..position })
        .collect();
    let bases = positions
        .iter()
        .map(|&scene| scene_snapshot(stage, story, scene))
        .collect::<Result<Vec<_>>>()?;
    let total_chars: u32 = positions
        .iter()
        .map(|&scene| story.scene_target(scene, &stage.manifest))
        .sum();
    let previous_scene = story.previous_text(stage.project, position)?;
    let assignment = Assignment {
        target_chars: total_chars,
        part_index: 1,
        part_total: 1,
        beat: None,
        previous_scene: &previous_scene,
        written_so_far: "",
    };
    let wanted_tokens = stage.output_tokens_for_chars(total_chars);
    let prompt = material.render(stage, &assignment, wanted_tokens)?;
    let output_tokens = stage.output_tokens(&prompt, wanted_tokens)?;
    let label = format!("{}の本文", chapter_label(story.chapter(position)));

    let mut retries_left = stage.settings.quality_retries;
    let texts = loop {
        let output = stage.caller.text(&label, &prompt, output_tokens).await?;
        let pieces = split_scenes(&output.text);
        let problem = if pieces.len() == positions.len() {
            let texts = clean_pieces(pieces, &previous_scene, output.truncated);
            match chapter_problems(stage, story, &positions, &texts) {
                None => break texts,
                Some(problem) => problem,
            }
        } else {
            format!(
                "{} シーン分のはずが、区切りで {} 個に分かれました",
                positions.len(),
                pieces.len()
            )
        };
        if retries_left == 0 {
            return Err(EngineError::InvalidOutput(format!(
                "{label}を書けませんでした（{problem}）。生成単位を「シーン」にすると確実です。"
            )));
        }
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!("{problem}。生成し直します。"),
        );
        stage.caller.expect_steps(1);
        retries_left -= 1;
    };

    let mut changes = ChangeSet::new(format!("{label}を生成しました。"));
    for ((&scene, text), base) in positions.iter().zip(texts).zip(bases) {
        put_scene(stage, story, scene, text, base, &mut changes);
    }
    Ok(changes)
}

async fn write_part(
    stage: &Stage<'_>,
    material: &DraftMaterial<'_>,
    assignment: &Assignment<'_>,
    label: String,
) -> Result<String> {
    let job = ProseJob {
        label,
        prompt: material.render(
            stage,
            assignment,
            stage.output_tokens_for_chars(assignment.target_chars),
        )?,
        target_chars: assignment.target_chars,
        previous_text: if assignment.written_so_far.is_empty() {
            assignment.previous_scene
        } else {
            assignment.written_so_far
        },
        style: &material.story.style,
    };
    prose::generate(stage, &job).await
}

/// 章単位の出力をシーンごとに整える。途中で切れていたら、最後のシーンの書きかけの文を落とす。
fn clean_pieces(pieces: Vec<String>, previous_scene: &str, truncated: bool) -> Vec<String> {
    let last = pieces.len().saturating_sub(1);
    let mut previous = previous_scene.to_owned();
    pieces
        .into_iter()
        .enumerate()
        .map(|(index, piece)| {
            let text = clean_prose(&piece, &previous);
            let text = if truncated && index == last {
                drop_unfinished_sentence(&text)
            } else {
                text
            };
            previous.clone_from(&text);
            text
        })
        .collect()
}

/// 章単位の出力の、生成し直す理由になる問題（重大な品質の問題か、空のシーン）。
fn chapter_problems(
    stage: &Stage<'_>,
    story: &Story,
    positions: &[Position],
    texts: &[String],
) -> Option<String> {
    positions.iter().zip(texts).find_map(|(&scene, text)| {
        let title = &story.scene(scene).title;
        if text.trim().is_empty() {
            return Some(format!("「{title}」が空でした"));
        }
        let report = inspect(text, story.scene_target(scene, &stage.manifest));
        report.has_errors().then(|| {
            let messages: Vec<&str> = report
                .issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect();
            format!("「{title}」に問題があります（{}）", messages.join("、"))
        })
    })
}

async fn plan_beats(
    stage: &Stage<'_>,
    story: &Story,
    position: Position,
    target: u32,
) -> Result<Vec<String>> {
    let chapter = story.chapter(position);
    let scene = story.scene(position);
    let beat_count = target
        .div_ceil(stage.settings.chars_per_call)
        .max(MIN_BEATS);
    // ほかのシーンの予定も見せて、後のシーンで明かす出来事をこのシーンのビートに取り込ませない
    let other_scenes: Vec<_> = chapter
        .meta
        .scenes
        .iter()
        .enumerate()
        .filter(|&(index, _)| index != position.scene)
        .map(|(index, other)| {
            context! {
                title => &other.title,
                summary => &other.summary,
                is_before => index < position.scene,
            }
        })
        .collect();
    let prompt = stage.prompts.render(
        PromptTemplate::Beats,
        &context! {
            project => &stage.info,
            chapter => context! { title => &chapter.meta.title, storyline => chapter.storyline.trim() },
            scene => context! {
                title => &scene.title,
                summary => &scene.summary,
                pov => &scene.pov,
                place => &scene.place,
                time => &scene.time,
                characters => &scene.characters,
            },
            other_scenes => other_scenes,
            beat_count => beat_count,
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, BEATS_OUTPUT_TOKENS)?;
    stage.caller.expect_steps(1);
    let label = format!("「{}」を展開に分ける", scene.title);
    let output: BeatsOutput = stage.caller.json(&label, &prompt, output_tokens).await?;
    let beats: Vec<String> = output
        .beats
        .into_iter()
        .map(|beat| beat.trim().to_owned())
        .filter(|beat| !beat.is_empty())
        .collect();
    if beats.is_empty() {
        return Err(EngineError::InvalidOutput(
            "シーンの展開が一つもありませんでした。".into(),
        ));
    }
    Ok(beats)
}

fn with_beats(chapter: Chapter, scene_index: usize, beats: Vec<String>) -> Chapter {
    let mut scenes = chapter.meta.scenes.clone();
    if let Some(scene) = scenes.get_mut(scene_index) {
        scene.beats = beats;
    }
    Chapter {
        meta: ChapterMeta {
            scenes,
            ..chapter.meta
        },
        ..chapter
    }
}

/// 生成を始める前のシーン本文（変更案の基準）。
fn scene_snapshot(
    stage: &Stage<'_>,
    story: &Story,
    position: Position,
) -> Result<Option<TextFile>> {
    stage.snapshot(&scene_path(story, position))
}

fn scene_path(story: &Story, position: Position) -> kataribe_project::RelPath {
    layout::scene_text_path(&story.chapter(position).id, &story.scene(position).id)
}

fn put_scene(
    stage: &Stage<'_>,
    story: &Story,
    position: Position,
    text: String,
    base: Option<TextFile>,
    changes: &mut ChangeSet,
) {
    report_quality(stage, story, position, &text);
    changes.put(scene_path(story, position), text, base);
}

/// シーン全体の品質の検査結果のうち、注意点を知らせる。
fn report_quality(stage: &Stage<'_>, story: &Story, position: Position, text: &str) {
    let target = story.scene_target(position, &stage.manifest);
    let report = inspect(text, target);
    let messages: Vec<&str> = report
        .issues
        .iter()
        .map(|issue| issue.message.as_str())
        .collect();
    if !messages.is_empty() {
        notice(
            stage.caller.sink(),
            NoticeLevel::Info,
            format!(
                "「{}」の品質チェック: {}",
                story.scene(position).title,
                messages.join("、")
            ),
        );
    }
}

fn join_paragraphs(text: &str, addition: &str) -> String {
    match (text.trim_end(), addition.trim_end()) {
        ("", addition) => format!("{addition}\n"),
        (text, "") => format!("{text}\n"),
        (text, addition) => format!("{text}\n{addition}\n"),
    }
}

fn progress_label(story: &Story, position: Position, part: u32, total: u32) -> String {
    let base = format!(
        "{} シーン{}「{}」の本文",
        chapter_label(story.chapter(position)),
        position.scene + 1,
        story.scene(position).title
    );
    if total > 1 {
        format!("{base}（{part}/{total}）")
    } else {
        base
    }
}

fn summary(story: &Story, position: Position) -> String {
    format!(
        "{} シーン{}「{}」の本文を生成しました。",
        chapter_label(story.chapter(position)),
        position.scene + 1,
        story.scene(position).title
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn paragraphs_are_joined_with_a_single_newline() {
        assert_eq!(join_paragraphs("", "　一。\n"), "　一。\n");
        assert_eq!(join_paragraphs("　一。\n", "　二。\n"), "　一。\n　二。\n");
    }

    #[test]
    fn truncated_chapter_output_loses_only_the_last_unfinished_sentence() {
        let pieces = vec![
            "雨が降った。\n".to_owned(),
            "書斎は静かだった。扉が".to_owned(),
        ];
        assert_eq!(
            clean_pieces(pieces, "", true),
            vec!["　雨が降った。\n", "　書斎は静かだった。\n"]
        );
    }
}
