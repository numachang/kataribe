//! 本文を書くときに参照する、作品全体の構成と現在位置。

use std::ops::Range;

use kataribe_project::{
    Chapter, ChapterId, Character, Manifest, Project, SceneId, ScenePlan, layout,
};
use kataribe_text::tokens::estimate_tokens;
use serde::Serialize;

use crate::error::{EngineError, Result};
use crate::names;
use crate::stages::materials::{
    chapter_label, document_body, find_chapter, round_to_hundreds, world_text,
};

/// 一度に登場人物の資料を入れる上限（場面に名前の無いときに主要人物から選ぶ人数）。
const DEFAULT_CAST_SIZE: usize = 4;
const MIN_SCENE_CHARS: u32 = 500;

pub(crate) struct Story {
    pub chapters: Vec<Chapter>,
    pub characters: Vec<Character>,
    pub style: String,
    pub world: String,
}

/// 章とシーンの位置（どちらも 0 始まりの添字）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Position {
    pub chapter: usize,
    pub scene: usize,
}

/// プロンプトにそのまま入れる、書き終えたシーン 1 つ分の本文。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct VerbatimScene {
    pub label: String,
    pub text: String,
}

/// そのまま渡す本文（読む順）と、その先頭のシーンの位置。これより前は要約で渡す。
pub(crate) struct VerbatimProse {
    pub start: Position,
    pub scenes: Vec<VerbatimScene>,
}

impl Story {
    pub fn load(project: &Project) -> Result<Self> {
        Ok(Self {
            chapters: project.chapters()?,
            characters: project.characters()?,
            style: document_body(project, layout::STYLE)?,
            world: world_text(project)?,
        })
    }

    pub fn locate(&self, chapter_id: ChapterId, scene_id: SceneId) -> Result<Position> {
        let (chapter_index, chapter) = find_chapter(&self.chapters, chapter_id)?;
        let scene_index = chapter
            .meta
            .scenes
            .iter()
            .position(|scene| scene.id == scene_id)
            .ok_or_else(|| {
                EngineError::NotFound(format!(
                    "第{}章にシーン {scene_id} がありません。先にシーン構成を生成してください。",
                    chapter.id.number()
                ))
            })?;
        Ok(Position {
            chapter: chapter_index,
            scene: scene_index,
        })
    }

    pub fn chapter(&self, position: Position) -> &Chapter {
        &self.chapters[position.chapter]
    }

    pub fn scene(&self, position: Position) -> &ScenePlan {
        &self.chapter(position).meta.scenes[position.scene]
    }

    /// 読む順で一つ前のシーン（シーンの無い章は飛ばす）。
    pub fn previous(&self, position: Position) -> Option<Position> {
        if position.scene > 0 {
            return Some(Position {
                scene: position.scene - 1,
                ..position
            });
        }
        (0..position.chapter).rev().find_map(|chapter| {
            let count = self.chapters[chapter].meta.scenes.len();
            (count > 0).then(|| Position {
                chapter,
                scene: count - 1,
            })
        })
    }

    /// このシーンより前の本文の末尾、`max_chars` 文字ほど（読む順）。
    pub fn recent_prose(
        &self,
        project: &Project,
        position: Position,
        max_chars: usize,
    ) -> Result<String> {
        let mut pieces = Vec::new();
        let mut total_chars = 0;
        let mut cursor = self.previous(position);
        while let Some(previous) = cursor {
            if total_chars >= max_chars {
                break;
            }
            let text = self.text(project, previous)?;
            total_chars += text.chars().count();
            pieces.push(text);
            cursor = self.previous(previous);
        }
        pieces.reverse();
        Ok(pieces.join("\n"))
    }

    /// このシーンの直前から遡って、`max_tokens` に収まるだけの本文。シーンの途中では切らない。
    /// 本文の無いシーンに当たったらそこで止める（そのシーンは要約の側で「予定」として渡すため）。
    pub fn verbatim_prose(
        &self,
        project: &Project,
        position: Position,
        max_tokens: usize,
    ) -> Result<VerbatimProse> {
        let mut scenes = Vec::new();
        let mut start = position;
        let mut used_tokens = 0;
        let mut cursor = self.previous(position);
        while let Some(previous) = cursor {
            let text = self.text(project, previous)?;
            used_tokens += estimate_tokens(&text);
            if text.trim().is_empty() || used_tokens > max_tokens {
                break;
            }
            let label = format!(
                "{} {}",
                chapter_label(self.chapter(previous)),
                self.scene(previous).title
            );
            scenes.push(VerbatimScene { label, text });
            start = previous;
            cursor = self.previous(previous);
        }
        scenes.reverse();
        Ok(VerbatimProse { start, scenes })
    }

    /// 人物の名前（姓・名に分けたものと、つなげたもの）。本文に繰り返し出てきて当然の語。
    pub fn character_name_words(&self) -> Vec<String> {
        let mut words = Vec::new();
        for character in &self.characters {
            let name = &character.meta.name;
            words.extend(name.split_whitespace().map(str::to_owned));
            words.push(name.split_whitespace().collect());
        }
        words.sort();
        words.dedup();
        words
    }

    /// 章単位で書くシーンの範囲（章の中の添字）。このシーンから、次に本文のあるシーンの手前まで。
    /// 作家が手で書いた後ろのシーンを、LLM の文章で置き換えないようにする。
    pub fn unwritten_run(&self, project: &Project, position: Position) -> Result<Range<usize>> {
        let scene_count = self.chapter(position).meta.scenes.len();
        let mut end = position.scene + 1;
        while end < scene_count
            && self
                .text(
                    project,
                    Position {
                        scene: end,
                        ..position
                    },
                )?
                .trim()
                .is_empty()
        {
            end += 1;
        }
        Ok(position.scene..end)
    }

    /// 前のシーンの本文。無ければ空文字列。
    pub fn previous_text(&self, project: &Project, position: Position) -> Result<String> {
        match self.previous(position) {
            Some(previous) => self.text(project, previous),
            None => Ok(String::new()),
        }
    }

    pub fn text(&self, project: &Project, position: Position) -> Result<String> {
        let chapter = self.chapter(position);
        Ok(project
            .scene_text(&chapter.id, &self.scene(position).id)?
            .unwrap_or_default())
    }

    /// シーンに登場する人物（視点人物を先頭に）。名前の指定が無ければ主要人物から選ぶ。
    pub fn cast_of(&self, scenes: &[&ScenePlan]) -> Vec<&Character> {
        let mut names: Vec<&str> = Vec::new();
        for scene in scenes {
            names.extend(scene.pov.as_deref());
            names.extend(scene.characters.iter().map(String::as_str));
        }
        let mut cast: Vec<&Character> = Vec::new();
        for name in names {
            if let Some(character) = names::find_character(&self.characters, name)
                && !cast.iter().any(|member| member.id == character.id)
            {
                cast.push(character);
            }
        }
        if cast.is_empty() {
            cast = self.characters.iter().take(DEFAULT_CAST_SIZE).collect();
        }
        cast
    }

    /// このシーンの目標文字数。シーン構成に無ければ、作品の目標をシーンの数で割って決める。
    pub fn scene_target(&self, position: Position, manifest: &Manifest) -> u32 {
        if let Some(target) = self.scene(position).target_chars {
            return target.max(MIN_SCENE_CHARS);
        }
        let chapter_count = u32::try_from(self.chapters.len()).unwrap_or(1).max(1);
        let scene_count = u32::try_from(self.chapter(position).meta.scenes.len())
            .unwrap_or(1)
            .max(1);
        round_to_hundreds(manifest.target_length / chapter_count / scene_count).max(MIN_SCENE_CHARS)
    }
}
