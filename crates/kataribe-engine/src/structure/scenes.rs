//! シーンの追加と削除。章立て（`plot/chapters/<NN>.md`）の `scenes` を書き直す。
//!
//! 章立ての YAML は書き直すので、利用者が手で書いたコメントや項目の順番は残らない
//! （画面から項目を変えて保存したときと同じ。画面が知らない項目は残る）。

use std::collections::BTreeMap;

use kataribe_project::{
    Chapter, ChapterId, ChapterMeta, EditableDocument, Project, SceneId, ScenePlan, TextFile,
    layout,
};

use super::edit::NewScenePlan;
use super::plan::StructurePlan;
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::stages::materials::non_empty;

/// 読み込んだ章立て。
struct LoadedChapter {
    id: ChapterId,
    meta: ChapterMeta,
    storyline: String,
    /// 読んだときのファイル。変更案の基準にする（そのあとの編集との競合を確かめるため）。
    file: TextFile,
}

/// 章にシーンを足す変更案。章立てを書き直す。
///
/// 新しいシーンの id は、章立てにある id に加えて、本文のフォルダに残っている本文の id も避けて決める
/// （消したシーンの本文を、新しいシーンが引き継がないように）。
pub(super) fn add(
    project: &Project,
    chapter_id: ChapterId,
    before: Option<SceneId>,
    scene: &NewScenePlan,
) -> Result<StructurePlan> {
    let title = scene.title.trim();
    if title.is_empty() {
        return Err(EngineError::InvalidInput(
            "シーンの題を入力してください。".into(),
        ));
    }
    let mut chapter = load_chapter(project, chapter_id)?;
    let position = match before {
        Some(before) => scene_position(&chapter, before)?,
        None => chapter.meta.scenes.len(),
    };
    let mut taken: Vec<SceneId> = chapter.meta.scenes.iter().map(|plan| plan.id).collect();
    taken.extend(project.scene_text_ids(&chapter_id)?);
    let planned = ScenePlan {
        id: SceneId::next_available(&taken),
        title: title.to_owned(),
        summary: scene.summary.trim().to_owned(),
        pov: scene.pov.as_deref().and_then(non_empty),
        characters: scene
            .characters
            .iter()
            .filter_map(|name| non_empty(name))
            .collect(),
        place: scene.place.as_deref().and_then(non_empty),
        time: scene.time.as_deref().and_then(non_empty),
        target_chars: scene.target_chars.filter(|chars| *chars > 0),
        beats: Vec::new(),
        extra: BTreeMap::new(),
    };
    chapter.meta.scenes.insert(position, planned);

    let mut changes = ChangeSet::new(format!(
        "第{}章にシーン「{title}」を追加します。",
        chapter_id.number()
    ));
    put_chapter(&mut changes, chapter)?;
    Ok(StructurePlan::new(changes).opening(layout::chapter_path(&chapter_id)))
}

/// 章のシーンを消す変更案。章立てを書き直し、本文があれば本文をゴミ箱へ移す。
pub(super) fn remove(
    project: &Project,
    chapter_id: ChapterId,
    scene_id: SceneId,
) -> Result<StructurePlan> {
    let mut chapter = load_chapter(project, chapter_id)?;
    let position = scene_position(&chapter, scene_id)?;
    let removed = chapter.meta.scenes.remove(position);
    let text_path = layout::scene_text_path(&chapter_id, &scene_id);
    let text = project.store().read_text_opt(&text_path)?;

    let consequence = if text.is_some() {
        "（本文もゴミ箱へ移ります）"
    } else {
        ""
    };
    let mut changes = ChangeSet::new(format!(
        "第{}章のシーン「{}」を削除します{consequence}。",
        chapter_id.number(),
        removed.title
    ));
    put_chapter(&mut changes, chapter)?;
    if let Some(text) = &text {
        changes.trash_file(text_path, text);
    }
    Ok(StructurePlan::new(changes))
}

/// 章立てを、項目に分けて編集できる形で読む。
///
/// 章立てが無いときは見つからないエラー。YAML を解釈できない・シーンの id が重複している章立ては、
/// 書き直すと別のシーンの本文を取り違えるので、直してから操作するよう伝えるエラーにする。
fn load_chapter(project: &Project, id: ChapterId) -> Result<LoadedChapter> {
    let path = layout::chapter_path(&id);
    let file = project.store().read_text_opt(&path)?.ok_or_else(|| {
        EngineError::NotFound(format!("第{}章（{path}）がありません。", id.number()))
    })?;
    let parsed = kataribe_project::parse_document(&path, &file.content);
    match parsed.document {
        EditableDocument::Chapter { meta, body } => Ok(LoadedChapter {
            id,
            meta,
            storyline: body,
            file,
        }),
        EditableDocument::Text { .. } | EditableDocument::Character { .. } => {
            Err(EngineError::InvalidInput(
                parsed
                    .parse_error
                    .unwrap_or_else(|| format!("{path} を章立てとして読めません。")),
            ))
        }
    }
}

fn scene_position(chapter: &LoadedChapter, scene: SceneId) -> Result<usize> {
    chapter
        .meta
        .scenes
        .iter()
        .position(|plan| plan.id == scene)
        .ok_or_else(|| {
            EngineError::NotFound(format!(
                "シーン {scene} が第{}章にありません。",
                chapter.id.number()
            ))
        })
}

/// 書き直した章立てを変更案に入れる。
fn put_chapter(changes: &mut ChangeSet, chapter: LoadedChapter) -> Result<()> {
    let path = layout::chapter_path(&chapter.id);
    let content = Chapter {
        id: chapter.id,
        meta: chapter.meta,
        storyline: chapter.storyline,
    }
    .render()?;
    changes.put(path, content, Some(chapter.file));
    Ok(())
}
