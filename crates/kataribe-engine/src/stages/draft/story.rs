//! 本文を書くときに参照する、作品全体の構成と現在位置。

use kataribe_project::{
    Chapter, ChapterId, Character, Manifest, Project, SceneId, ScenePlan, layout,
};

use crate::error::{EngineError, Result};
use crate::stages::cast::same_person;
use crate::stages::materials::{document_body, find_chapter, round_to_hundreds, world_text};

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

    /// 同じ章の、このシーンから章の終わりまでの位置。
    pub fn rest_of_chapter(&self, position: Position) -> Vec<Position> {
        (position.scene..self.chapter(position).meta.scenes.len())
            .map(|scene| Position { scene, ..position })
            .collect()
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
            if let Some(character) = self.find_character(name)
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

    /// 名前から人物を探す。「凛」のように名だけで書かれていても見つける。
    fn find_character(&self, name: &str) -> Option<&Character> {
        self.characters
            .iter()
            .find(|character| same_person(&character.meta.name, name))
            .or_else(|| {
                let name = name.trim();
                (!name.is_empty())
                    .then(|| {
                        self.characters.iter().find(|character| {
                            character
                                .meta
                                .name
                                .split_whitespace()
                                .any(|part| part == name)
                        })
                    })
                    .flatten()
            })
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
