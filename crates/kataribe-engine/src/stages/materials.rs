//! プロンプトに入れる材料を作品フォルダから集める。

use kataribe_project::{
    Chapter, ChapterId, Character, CharacterId, Manifest, Project, ProjectError, RelPath, TextFile,
    layout,
};
use serde::Serialize;

use crate::error::{EngineError, Result};
use crate::excerpt;
use crate::genre::{GenreCatalog, GenrePreset, rating_label};

/// どのプロンプトにも入る作品情報。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProjectInfo {
    pub title: String,
    pub genre_label: String,
    /// 「〜を得意とする」に入るジャンル名。特定のジャンルに寄せない「その他」では `None`。
    pub specialty: Option<String>,
    pub genre_guidance: String,
    pub genre_note: Option<String>,
    pub rating_label: &'static str,
    pub rating_rule: String,
    pub target_length: u32,
    pub idea: String,
}

impl ProjectInfo {
    pub fn new(manifest: &Manifest, genres: &GenreCatalog) -> Self {
        let genre = genres.resolve(&manifest.genre);
        let genre_label = genre.map_or_else(|| manifest.genre.clone(), |genre| genre.label.clone());
        let is_general = genre.is_none_or(GenrePreset::is_general);
        Self {
            title: manifest.title.clone(),
            specialty: (!is_general).then(|| genre_label.clone()),
            genre_label,
            genre_guidance: genre
                .map(|genre| genre.guidance.trim().to_owned())
                .unwrap_or_default(),
            genre_note: manifest
                .genre_note
                .clone()
                .filter(|note| !note.trim().is_empty()),
            rating_label: rating_label(manifest.rating),
            rating_rule: genres.rating_rule(manifest.rating).trim().to_owned(),
            target_length: manifest.target_length,
            idea: manifest.idea.trim().to_owned(),
        }
    }
}

/// プロンプトに入れる人物の要約。`profile` は資料の本文（切り詰め済み。不要なら空）。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CharacterBrief {
    pub name: String,
    pub reading: Option<String>,
    pub role: String,
    pub summary: String,
    pub profile: String,
}

impl CharacterBrief {
    pub fn new(character: &Character, profile_chars: usize) -> Self {
        Self {
            name: character.meta.name.clone(),
            reading: character.meta.reading.clone(),
            role: character.meta.role.clone(),
            summary: character.meta.summary.clone(),
            profile: excerpt::head(&character.body, profile_chars),
        }
    }
}

pub(crate) fn briefs(characters: &[Character], profile_chars: usize) -> Vec<CharacterBrief> {
    characters
        .iter()
        .map(|character| CharacterBrief::new(character, profile_chars))
        .collect()
}

/// 資料の本文（front matter を除く）。ファイルが無ければ空文字列。
pub(crate) fn document_body(project: &Project, path: &str) -> Result<String> {
    let path = RelPath::new(path)?;
    Ok(project
        .markdown(&path)?
        .map(|document| document.body.trim().to_owned())
        .unwrap_or_default())
}

/// 世界観の資料すべて（概要を先頭に）。
pub(crate) fn world_text(project: &Project) -> Result<String> {
    let documents = project.world_docs()?;
    Ok(documents
        .iter()
        .map(|(_, document)| document.body.trim())
        .filter(|body| !body.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n"))
}

/// 生成を始める前の人物ファイルと、その内容を解析した人物。変更案の基準にする。
pub(crate) fn snapshot_character(
    project: &Project,
    id: &CharacterId,
) -> Result<(Character, TextFile)> {
    let path = layout::character_path(id);
    let file = project
        .store()
        .read_text_opt(&path)?
        .ok_or_else(|| EngineError::NotFound(format!("人物ファイル {path} がありません。")))?;
    let character = Character::parse(id.clone(), &file.content)
        .map_err(|source| ProjectError::Frontmatter { path, source })?;
    Ok((character, file))
}

/// 生成を始める前の章ファイルと、その内容を解析した章。変更案の基準にする。
pub(crate) fn snapshot_chapter(project: &Project, id: ChapterId) -> Result<(Chapter, TextFile)> {
    let path = layout::chapter_path(&id);
    let file = project.store().read_text_opt(&path)?.ok_or_else(|| {
        EngineError::NotFound(format!("第{}章（{path}）がありません。", id.number()))
    })?;
    let chapter = Chapter::parse(id, &file.content)
        .map_err(|source| ProjectError::Frontmatter { path, source })?;
    Ok((chapter, file))
}

/// 前後の空白を除き、空なら `None` にする（LLM が空文字列で「なし」を表すことがあるため）。
pub(crate) fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// 前提となる資料がそろっているか確かめる。
pub(crate) fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(EngineError::MissingPrerequisite(message.to_owned()))
    }
}

pub(crate) fn find_chapter(chapters: &[Chapter], id: ChapterId) -> Result<(usize, &Chapter)> {
    chapters
        .iter()
        .enumerate()
        .find(|(_, chapter)| chapter.id == id)
        .ok_or_else(|| {
            EngineError::NotFound(format!(
                "第{}章（{}）が見つかりません。",
                id.number(),
                layout::chapter_path(&id)
            ))
        })
}

/// 章の表示名（例: 第1章「雨の匂い」）。
pub(crate) fn chapter_label(chapter: &Chapter) -> String {
    format!("第{}章「{}」", chapter.id.number(), chapter.meta.title)
}

/// 目標の総文字数から、章の数を決める。1 章はおよそ 8,000 字。
pub(crate) fn chapter_count_for(target_length: u32) -> u32 {
    const CHARS_PER_CHAPTER: f64 = 8000.0;
    let count = (f64::from(target_length) / CHARS_PER_CHAPTER).round();
    // 丸めた値は 1〜40 に収めるので、u32 への変換で値が失われることはない
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = count.clamp(1.0, 40.0) as u32;
    count
}

/// 1 章の文字数から、シーンの数を決める。1 シーンがおよそ 2,500 字を超えないようにする
/// （長いシーンは出来事を詰め込みがちで、ローカル LLM が設計どおりに書き切れないため）。
pub(crate) fn scene_count_for(chapter_chars: u32) -> u32 {
    const MAX_CHARS_PER_SCENE: f64 = 2500.0;
    let count = (f64::from(chapter_chars) / MAX_CHARS_PER_SCENE).ceil();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = count.clamp(2.0, 8.0) as u32;
    count
}

/// 目標の総文字数から、登場人物の数を決める。
pub(crate) fn cast_size_for(target_length: u32) -> u32 {
    match target_length {
        0..=20_000 => 5,
        20_001..=60_000 => 6,
        _ => 8,
    }
}

/// 百の位で丸める（プロンプトに書く目安の文字数を読みやすくするため）。
pub(crate) fn round_to_hundreds(chars: u32) -> u32 {
    ((chars + 50) / 100 * 100).max(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_count_scales_with_length() {
        assert_eq!(chapter_count_for(3000), 1);
        assert_eq!(chapter_count_for(30_000), 4);
        assert_eq!(chapter_count_for(100_000), 13);
        assert_eq!(chapter_count_for(10_000_000), 40);
    }

    #[test]
    fn scene_count_stays_within_bounds() {
        assert_eq!(scene_count_for(1000), 2);
        assert_eq!(scene_count_for(7500), 3);
        assert_eq!(scene_count_for(6000), 3);
        assert_eq!(scene_count_for(7600), 4);
        assert_eq!(scene_count_for(100_000), 8);
    }

    #[test]
    fn cast_grows_with_length() {
        assert_eq!(cast_size_for(10_000), 5);
        assert_eq!(cast_size_for(30_000), 6);
        assert_eq!(cast_size_for(200_000), 8);
    }

    #[test]
    fn rounding_to_hundreds() {
        assert_eq!(round_to_hundreds(2449), 2400);
        assert_eq!(round_to_hundreds(2450), 2500);
        assert_eq!(round_to_hundreds(10), 100);
    }
}
