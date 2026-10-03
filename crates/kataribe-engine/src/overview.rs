//! 画面の目次に出す、作品全体の一覧。
//! 1 つのファイルが壊れていても一覧全体は出せるよう、ファイルごとにエラーを記録する。

use kataribe_project::store::DirEntryKind;
use kataribe_project::{
    Chapter, ChapterId, CharacterId, Project, ProjectError, RelPath, SceneId, layout,
};
use kataribe_text::count::count_chars;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::stages::materials::chapter_label;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum SectionKind {
    Planning,
    World,
    Characters,
    Plot,
    Manuscript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum EntryKind {
    Manifest,
    Concept,
    Style,
    World,
    Character,
    Synopsis,
    Chapter,
    Scene,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct OverviewEntry {
    /// 作品フォルダからの相対パス。ファイルに対応しない見出しは `None`。
    pub path: Option<String>,
    pub label: String,
    pub kind: EntryKind,
    /// 章に属する項目（章立て・本文の章見出し・シーンの本文）の章。本文の章見出しにはパスが無いので、
    /// 画面が章を知るために持つ。
    pub chapter: Option<ChapterId>,
    /// シーンの本文の項目のシーン。
    pub scene: Option<SceneId>,
    pub exists: bool,
    /// 本文の文字数（ルビの読み・空白を除く）。
    pub chars: usize,
    pub target_chars: Option<u32>,
    /// 読み込みや YAML の解析に失敗したときの説明。
    pub error: Option<String>,
    pub children: Vec<OverviewEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct OverviewSection {
    pub kind: SectionKind,
    pub label: String,
    pub entries: Vec<OverviewEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectOverview {
    pub root: String,
    pub title: String,
    pub target_length: u32,
    pub total_chars: usize,
    pub sections: Vec<OverviewSection>,
}

pub fn overview(project: &Project) -> Result<ProjectOverview> {
    let manifest = project.manifest()?;
    let chapters = chapter_entries(project)?;
    let manuscript = manuscript_section(project, &chapters)?;
    let total_chars = manuscript
        .entries
        .iter()
        .flat_map(|chapter| &chapter.children)
        .map(|scene| scene.chars)
        .sum();
    Ok(ProjectOverview {
        root: project.root().display().to_string(),
        title: manifest.title,
        target_length: manifest.target_length,
        total_chars,
        sections: vec![
            section(
                SectionKind::Planning,
                "企画・文体",
                vec![
                    file_entry(project, layout::MANIFEST, "作品情報", EntryKind::Manifest)?,
                    file_entry(project, layout::CONCEPT, "企画", EntryKind::Concept)?,
                    file_entry(project, layout::STYLE, "文体ガイド", EntryKind::Style)?,
                ],
            ),
            section(SectionKind::World, "世界観", world_entries(project)?),
            section(
                SectionKind::Characters,
                "登場人物",
                character_entries(project)?,
            ),
            section(
                SectionKind::Plot,
                "プロット",
                plot_entries(project, &chapters)?,
            ),
            manuscript,
        ],
    })
}

fn section(kind: SectionKind, label: &str, entries: Vec<OverviewEntry>) -> OverviewSection {
    OverviewSection {
        kind,
        label: label.to_owned(),
        entries,
    }
}

fn entry(path: &RelPath, label: String, kind: EntryKind) -> OverviewEntry {
    OverviewEntry {
        path: Some(path.to_string()),
        label,
        kind,
        chapter: None,
        scene: None,
        exists: false,
        chars: 0,
        target_chars: None,
        error: None,
        children: Vec::new(),
    }
}

/// 1 ファイル分の項目。ファイルが無くても（未生成でも）項目は出す。
fn file_entry(
    project: &Project,
    path: &str,
    label: &str,
    kind: EntryKind,
) -> Result<OverviewEntry> {
    let path = RelPath::new(path)?;
    let mut result = entry(&path, label.to_owned(), kind);
    match project.store().read_text_opt(&path) {
        Ok(Some(file)) => {
            result.exists = true;
            result.chars = count_chars(&file.content);
        }
        Ok(None) => {}
        Err(error) => result.error = Some(error.to_string()),
    }
    Ok(result)
}

fn world_entries(project: &Project) -> Result<Vec<OverviewEntry>> {
    let mut entries = vec![file_entry(
        project,
        layout::WORLD_OVERVIEW,
        "世界観",
        EntryKind::World,
    )?];
    for path in markdown_files(project, layout::WORLD_DIR)? {
        if path.as_str() == layout::WORLD_OVERVIEW {
            continue;
        }
        let label = path.file_stem().to_owned();
        let mut item = file_entry(project, path.as_str(), &label, EntryKind::Other)?;
        if let Ok(Some(document)) = project.markdown(&path) {
            item.label = document.display_title(&label);
        }
        entries.push(item);
    }
    Ok(entries)
}

fn character_entries(project: &Project) -> Result<Vec<OverviewEntry>> {
    let mut characters: Vec<(Option<u32>, OverviewEntry)> = Vec::new();
    for path in markdown_files(project, layout::CHARACTERS_DIR)? {
        let stem = path.file_stem().to_owned();
        let mut item = file_entry(project, path.as_str(), &stem, EntryKind::Character)?;
        let order = match CharacterId::new(&stem)
            .map_err(ProjectError::from)
            .and_then(|id| project.character(&id))
        {
            Ok(Some(character)) => {
                item.label.clone_from(&character.meta.name);
                item.chars = count_chars(&character.body);
                character.meta.order
            }
            Ok(None) => None,
            Err(error) => {
                item.error = Some(error.to_string());
                None
            }
        };
        characters.push((order, item));
    }
    sort_for_display(&mut characters, |(order, _)| *order);
    Ok(characters.into_iter().map(|(_, item)| item).collect())
}

/// 章ファイルを読んだ結果。壊れた章も目次には出す。
struct ChapterEntry {
    id: ChapterId,
    path: RelPath,
    parsed: std::result::Result<Chapter, String>,
}

fn chapter_entries(project: &Project) -> Result<Vec<ChapterEntry>> {
    let mut chapters = Vec::new();
    for path in markdown_files(project, layout::CHAPTERS_DIR)? {
        let Ok(id) = ChapterId::new(path.file_stem()) else {
            continue;
        };
        let parsed = match project.chapter(&id) {
            Ok(Some(chapter)) => Ok(chapter),
            Ok(None) => Err(format!("{path} を読み込めません。")),
            Err(error) => Err(error.to_string()),
        };
        chapters.push(ChapterEntry { id, path, parsed });
    }
    Ok(chapters)
}

fn plot_entries(project: &Project, chapters: &[ChapterEntry]) -> Result<Vec<OverviewEntry>> {
    let mut entries = vec![file_entry(
        project,
        layout::SYNOPSIS,
        "あらすじ",
        EntryKind::Synopsis,
    )?];
    for chapter in chapters {
        let mut item = entry(
            &chapter.path,
            format!("第{}章", chapter.id.number()),
            EntryKind::Chapter,
        );
        item.chapter = Some(chapter.id);
        item.exists = true;
        match &chapter.parsed {
            Ok(parsed) => {
                item.label = chapter_label(parsed);
                item.chars = count_chars(&parsed.storyline);
            }
            Err(error) => item.error = Some(error.clone()),
        }
        entries.push(item);
    }
    Ok(entries)
}

fn manuscript_section(project: &Project, chapters: &[ChapterEntry]) -> Result<OverviewSection> {
    let mut entries = Vec::new();
    for chapter in chapters {
        let Ok(parsed) = &chapter.parsed else {
            continue;
        };
        let mut heading = OverviewEntry {
            path: None,
            label: chapter_label(parsed),
            kind: EntryKind::Chapter,
            chapter: Some(parsed.id),
            scene: None,
            exists: true,
            chars: 0,
            target_chars: None,
            error: None,
            children: Vec::new(),
        };
        for (index, scene) in parsed.meta.scenes.iter().enumerate() {
            let path = layout::scene_text_path(&parsed.id, &scene.id);
            let label = format!("{}. {}", index + 1, scene.title);
            let mut item = file_entry(project, path.as_str(), &label, EntryKind::Scene)?;
            item.chapter = Some(parsed.id);
            item.scene = Some(scene.id);
            item.target_chars = scene.target_chars.filter(|&target| target > 0);
            heading.chars += item.chars;
            heading.children.push(item);
        }
        heading.target_chars = chapter_target(&heading.children);
        entries.push(heading);
    }
    Ok(section(SectionKind::Manuscript, "本文", entries))
}

/// 章の目標文字数。すべてのシーンに目標があるときだけ合計する
/// （一部のシーンだけの合計を、章全体の目標のように見せないため）。
fn chapter_target(scenes: &[OverviewEntry]) -> Option<u32> {
    if scenes.is_empty() {
        return None;
    }
    scenes
        .iter()
        .map(|scene| scene.target_chars)
        .sum::<Option<u32>>()
}

/// 登場人物を目次に出す順に並べる。`order` の昇順で、`order` の無い人物は最後。同じ値どうしは元の並び
/// （ファイル名の順）のまま。目次と、人物の並べ替え（`structure`）が同じ並びを使うための唯一の定義。
pub(crate) fn sort_for_display<T>(items: &mut [T], order_of: impl Fn(&T) -> Option<u32>) {
    items.sort_by_key(|item| order_of(item).unwrap_or(u32::MAX));
}

/// フォルダ直下の Markdown ファイル（名前順）。フォルダが無ければ空。
pub(crate) fn markdown_files(project: &Project, directory: &str) -> Result<Vec<RelPath>> {
    let directory = RelPath::new(directory)?;
    if !project.store().exists(&directory) {
        return Ok(Vec::new());
    }
    Ok(project
        .store()
        .list_dir(&directory)?
        .into_iter()
        .filter(|entry| entry.kind == DirEntryKind::File && entry.path.extension() == Some("md"))
        .map(|entry| entry.path)
        .collect())
}
