//! 人物の追加・削除・並べ替え。
//!
//! 並べ替えは、読める人物資料の `order` を 1, 2, 3… に振り直して書き直す。YAML は書き直すので、人物資料の
//! front matter に手で書いたコメントや項目の順番は残らない（アプリが知らない項目は残る）。`order` の行だけを
//! 行単位で書き換えて YAML をそのまま保つ方法もあるが、YAML の書き方（引用符・ブロック表記・別の位置の `order`）を
//! 文字列として追うことになり壊れやすいので採らない。

use kataribe_project::{
    Character, CharacterId, CharacterMeta, Project, RelPath, TextFile, frontmatter, layout,
};
use kataribe_text::romaji::to_romaji;

use super::plan::{StructurePlan, Wording};
use super::position::{ensure_new_position, move_item};
use super::references;
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::overview::{markdown_files, sort_for_display};
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
    ensure_character_document(path, "消せる")?;
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

/// 人物の表示順を変える変更案。読める人物の `order` を、並べ替えたあとの並びで 1, 2, 3… に振り直し、
/// 値が変わる人物資料だけを書き直す。
///
/// `position` は、並べ替えたあとに、目次の人物の何番目に来るか（0 始まり）。目次と同じ並び
/// （`order` の昇順、`order` の無い人物は最後）の中で数える。YAML が読めない人物資料は動かせず、
/// ほかの人物を動かすときも `order` を変えずに飛ばす（読めないので、書き直すと壊れた中身を失うため）。
/// 読めない資料は目次でも最後に並ぶので、その位置へ動かした人物は、読める人物の最後になる。
pub(super) fn move_to(project: &Project, path: &RelPath, position: usize) -> Result<StructurePlan> {
    ensure_character_document(path, "並べ替えられる")?;
    let mut roster = load_roster(project)?;
    let (current, name) = locate_movable(&roster, path)?;
    let subject = format!("人物「{name}」");
    ensure_new_position(&subject, "人物", current, position, roster.len())?;
    move_item(&mut roster, current, position);

    let wording = Wording::new(format!("{subject}を {} 番目に移し", position + 1));
    let mut changes = ChangeSet::new(wording.planned());
    let mut unreadable = Vec::new();
    let mut next_order = 1;
    for entry in roster {
        match entry.state {
            Ok(mut loaded) => {
                if loaded.character.meta.order != Some(next_order) {
                    loaded.character.meta.order = Some(next_order);
                    changes.put(entry.path, loaded.character.render()?, Some(loaded.file));
                }
                next_order += 1;
            }
            Err(_) => unreadable.push(entry.path),
        }
    }
    if changes.is_empty() {
        return Err(EngineError::InvalidInput(format!(
            "{subject}の順番は変わりません（読めない人物資料は、いちばん後ろに並びます）。"
        )));
    }
    let mut plan = StructurePlan::new(changes, &wording);
    if !unreadable.is_empty() {
        let paths: Vec<String> = unreadable.iter().map(ToString::to_string).collect();
        plan.notices.push(format!(
            "読めない人物資料（{}）は、順番を変えずにいちばん後ろに並べたままにしています。",
            paths.join("、")
        ));
    }
    Ok(plan)
}

/// 動かす人物資料の、目次での位置（0 始まり）と人物の名前。読めない人物資料は動かせない。
fn locate_movable(roster: &[RosterEntry], path: &RelPath) -> Result<(usize, String)> {
    let (position, entry) = roster
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.path == *path)
        .ok_or_else(|| EngineError::NotFound(format!("人物資料 {path} がありません。")))?;
    match &entry.state {
        Ok(loaded) => Ok((position, loaded.character.meta.name.clone())),
        Err(reason) => Err(EngineError::InvalidInput(format!(
            "{path} を読めないため、並べ替えられません。先に直してください（{reason}）。"
        ))),
    }
}

/// `characters/` 直下の Markdown（ファイル名の規則には照らさない）だけを指しているか確かめる。
/// `action` は「消せる」「並べ替えられる」のように、この操作でできることの言い方。
fn ensure_character_document(path: &RelPath, action: &str) -> Result<()> {
    if layout::is_character_document(path) {
        return Ok(());
    }
    Err(EngineError::InvalidInput(format!(
        "{path} は{action}人物資料ではありません（{action}のは、characters/ 直下の Markdown です）。"
    )))
}

/// 目次の「登場人物」の 1 行。
struct RosterEntry {
    path: RelPath,
    /// 読めた人物資料。読めなければ理由（目次の項目の `error` と同じ規則で決まる）。
    state: std::result::Result<LoadedCharacter, String>,
}

struct LoadedCharacter {
    character: Character,
    /// 読んだときのファイル。変更案の基準にする（そのあとの編集との競合を確かめるため）。
    file: TextFile,
}

/// 目次と同じ並び（[`sort_for_display`]）の人物資料。読めないものも含む。
///
/// 目次と同じく、ファイル名が人物 ID の規則に合わない資料（`Rin.md`）は読めないものとして数える。
fn load_roster(project: &Project) -> Result<Vec<RosterEntry>> {
    let mut roster: Vec<RosterEntry> = markdown_files(project, layout::CHARACTERS_DIR)?
        .into_iter()
        .map(|path| {
            let state = load_character(project, &path);
            RosterEntry { path, state }
        })
        .collect();
    sort_for_display(&mut roster, |entry| {
        entry
            .state
            .as_ref()
            .ok()
            .and_then(|loaded| loaded.character.meta.order)
    });
    Ok(roster)
}

fn load_character(
    project: &Project,
    path: &RelPath,
) -> std::result::Result<LoadedCharacter, String> {
    let id = CharacterId::new(path.file_stem()).map_err(|error| error.to_string())?;
    let file = project
        .store()
        .read_text_opt(path)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("{path} がありません。"))?;
    let character = Character::parse(id, &file.content).map_err(|error| error.to_string())?;
    Ok(LoadedCharacter { character, file })
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
