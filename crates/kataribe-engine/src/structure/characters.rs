//! 人物の追加と削除。

use kataribe_project::{
    Character, CharacterId, CharacterMeta, Project, RelPath, frontmatter, layout,
};
use kataribe_text::romaji::to_romaji;

use super::plan::{StructurePlan, Wording};
use super::references;
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::stages::materials::non_empty;

/// 人物を足す変更案。`characters/<id>.md` を新規に書く。
pub(super) fn add(
    project: &Project,
    requested_id: Option<&str>,
    meta: &CharacterMeta,
    body: &str,
) -> Result<StructurePlan> {
    let name = meta.name.trim();
    if name.is_empty() {
        return Err(EngineError::InvalidInput(
            "人物の名前を入力してください。".into(),
        ));
    }
    let id = match requested_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(requested) => ensure_unused(project, parse_requested_id(requested)?)?,
        None => suggest_id(project, meta.reading.as_deref().unwrap_or_default(), name)?,
    };
    let meta = CharacterMeta {
        name: name.to_owned(),
        reading: meta.reading.as_deref().and_then(non_empty),
        role: meta.role.trim().to_owned(),
        summary: meta.summary.trim().to_owned(),
        order: Some(match meta.order {
            Some(order) => order,
            None => next_order(project)?,
        }),
        extra: meta.extra.clone(),
    };
    let path = layout::character_path(&id);
    let character = Character {
        id,
        meta,
        body: body.to_owned(),
    };
    let wording = Wording::new(format!("人物「{name}」を追加し"));
    let mut changes = ChangeSet::new(wording.planned());
    changes.put(path.clone(), character.render()?, None);
    Ok(StructurePlan::new(changes, &wording).opening(path))
}

/// 人物を消す（ゴミ箱へ移す）変更案。
///
/// `characters/` 直下の Markdown なら、ファイル名が人物 ID の規則に合わないもの（`Rin.md`・`凛.md`）も消せる。
/// YAML が壊れた人物資料も消せる。その場合は名前を読めないので、シーンでの参照は調べられない。
pub(super) fn remove(project: &Project, path: &RelPath) -> Result<StructurePlan> {
    if !layout::is_character_document(path) {
        return Err(EngineError::InvalidInput(format!(
            "{path} は消せる人物資料ではありません（消せるのは、characters/ 直下の Markdown です）。"
        )));
    }
    let file = project
        .store()
        .read_text_opt(path)?
        .ok_or_else(|| EngineError::NotFound(format!("人物資料 {path} がありません。")))?;

    let mut plan_references = Vec::new();
    let mut notices = Vec::new();
    let label = match frontmatter::parse::<CharacterMeta>(&file.content) {
        Ok(document) => {
            let report = references::scenes_mentioning(project, &document.meta.name)?;
            plan_references = report.references;
            notices = report.notices;
            document.meta.name
        }
        Err(error) => {
            notices.push(format!(
                "{path} を読めないため、人物の名前を確かめられず、シーンでの参照を調べられませんでした（{error}）。"
            ));
            path.file_stem().to_owned()
        }
    };
    let wording = Wording::new(format!("人物「{label}」をゴミ箱へ移し"));
    let mut changes = ChangeSet::new(wording.planned());
    changes.trash_file(path.clone(), &file);
    Ok(StructurePlan {
        references: plan_references,
        notices,
        ..StructurePlan::new(changes, &wording)
    })
}

/// 人物の ID の案。読み、無ければ名前をローマ字にして、使用済みの ID を避ける。
pub(super) fn suggest_id(project: &Project, reading: &str, name: &str) -> Result<CharacterId> {
    let hint = [reading, name]
        .into_iter()
        .map(to_romaji)
        .find(|romaji| !romaji.is_empty())
        .unwrap_or_default();
    let taken = project.character_ids()?;
    Ok(CharacterId::from_hint(&hint, &taken))
}

/// 利用者が指定した ID を検証する。
///
/// 画面から戻ってくる値なので、規則に合わなければ、直せる入力の誤りとして知らせる。
fn parse_requested_id(requested: &str) -> Result<CharacterId> {
    CharacterId::new(requested).map_err(|_| {
        EngineError::InvalidInput(format!(
            "ID「{requested}」は使えません。小文字の英数字とハイフンで、48 文字以内にしてください\
             （ハイフンは先頭・末尾・連続に置けません。con など Windows の予約名も使えません）。"
        ))
    })
}

/// 指定された ID がまだ使われていなければ、そのまま返す。
///
/// ID の一覧ではなくファイルの有無で調べる。大文字小文字の違うファイルや壊れたファイルでも、
/// 同じパスに書こうとすれば競合するので、先に利用者へ分かる言葉で知らせる。
fn ensure_unused(project: &Project, id: CharacterId) -> Result<CharacterId> {
    if project.store().exists(&layout::character_path(&id)) {
        return Err(EngineError::InvalidInput(format!(
            "ID「{id}」はもう使われています。"
        )));
    }
    Ok(id)
}

/// 今の最大の表示順の次の番号。
fn next_order(project: &Project) -> Result<u32> {
    let mut highest = 0;
    for id in project.character_ids()? {
        // 壊れた人物資料は表示順を読めない。それで人物の追加を止めず、読めたものの最大を使う
        if let Ok(Some(character)) = project.character(&id)
            && let Some(order) = character.meta.order
        {
            highest = highest.max(order);
        }
    }
    Ok(highest.saturating_add(1))
}
