//! 章の追加と削除。章の順序はファイル名（`plot/chapters/<NN>.md`・`manuscript/<NN>/`）の番号なので、
//! 途中に足す・消すときは、後ろの章の番号を振り直す（ファイルとフォルダを改名する）。
//!
//! 番号を振り直す規則:
//! - `before = X` で足すなら、X 以上の章を 1 つ後ろへずらし、新しい章を X にする。末尾に足すときは、
//!   最大の番号の次（無ければ 01）で、ほかの章は改名しない。
//! - 削除すると、それより後ろの章を 1 つ前へずらす。
//! - 途中が抜けた番号は抜けたまま残す（手で作った番号を勝手に詰めない）。ずらすのは、操作した位置より後ろの章だけ。
//! - 章の本文のフォルダ（`manuscript/<NN>/`）は、章立てと一緒に、フォルダごと 1 回の改名で動かす
//!   （中のファイルを 1 つずつ動かすと、章立てに載っていない本文が古い番号のフォルダに残り、別の章の本文と混ざる）。
//!   本文がまだ無い章（フォルダの無い章）は、改名もゴミ箱への移動もせず、適用のときに移動元にまだ無いこと、
//!   移動先が空いたままであることだけを確かめる（移動先は、同じ変更案の改名・ゴミ箱で空くものを除く）。
//!
//! ほかの章の YAML は読まない（壊れた章があっても操作できる）。章題を読むのは、利用者に見せる文言のためだけで、
//! 読めなければ番号だけにする。

use std::collections::{BTreeMap, HashSet};

use kataribe_project::{Chapter, ChapterId, ChapterMeta, Project, RelPath, frontmatter, layout};

use super::plan::{RenumberedChapter, StructurePlan, Wording};
use crate::change_set::{ChangeSet, FileChange};
use crate::error::{EngineError, Result};

/// 章を足す変更案。後ろの章の番号を振り直し、新しい章の章立て（シーン構成なし）を新規に書く。
pub(super) fn add(
    project: &Project,
    before: Option<ChapterId>,
    title: &str,
    storyline: &str,
) -> Result<StructurePlan> {
    let title = title.trim();
    if title.is_empty() {
        return Err(EngineError::InvalidInput("章題を入力してください。".into()));
    }
    if title.contains(['\n', '\r']) {
        return Err(EngineError::InvalidInput(
            "章題は 1 行で入力してください。".into(),
        ));
    }
    let existing = project.chapter_ids()?;
    let (new_id, shifted) = match before {
        Some(before) => {
            ensure_chapter_exists(&existing, before)?;
            (
                before,
                chapters_where(&existing, |number| number >= before.number()),
            )
        }
        None => (next_chapter_id(&existing)?, Vec::new()),
    };

    let wording = Wording::new(format!("{}を追加し", chapter_label(new_id, Some(title))));
    let mut changes = ChangeSet::new(wording.planned());
    let renumbered = renumber(project, &mut changes, &shifted, 1)?;
    if before.is_none() {
        // 末尾に足す章の本文のフォルダは、まだ無いはず。外で作られたら（章立ての無い本文が増えたら）適用を止める
        changes.expect(layout::manuscript_chapter_dir(&new_id), None);
    }
    let path = layout::chapter_path(&new_id);
    changes.put(
        path.clone(),
        render_new_chapter(new_id, title, storyline)?,
        None,
    );
    let occupied_numbers = renumbered.iter().map(|chapter| chapter.to).chain([new_id]);
    ensure_slots_free(project, &changes, occupied_numbers)?;

    Ok(StructurePlan {
        renumbered,
        ..StructurePlan::new(changes, &wording).opening(path)
    })
}

/// 章を消す変更案。章立てと本文のフォルダをゴミ箱へ移し、後ろの章の番号を 1 つ前へずらす。
pub(super) fn remove(project: &Project, chapter: ChapterId) -> Result<StructurePlan> {
    let path = layout::chapter_path(&chapter);
    let file = project.store().read_text_opt(&path)?.ok_or_else(|| {
        EngineError::NotFound(format!("第{}章（{path}）がありません。", chapter.number()))
    })?;
    let existing = project.chapter_ids()?;
    let shifted = chapters_where(&existing, |number| number > chapter.number());

    let wording = Wording::new(format!(
        "{}をゴミ箱へ移し",
        chapter_label(chapter, title_of(&file.content).as_deref())
    ));
    let mut changes = ChangeSet::new(wording.planned());
    changes.trash_file(path, &file);
    trash_text_folder(project, &mut changes, chapter)?;
    let renumbered = renumber(project, &mut changes, &shifted, -1)?;
    ensure_slots_free(
        project,
        &changes,
        renumbered.iter().map(|chapter| chapter.to),
    )?;

    Ok(StructurePlan {
        renumbered,
        ..StructurePlan::new(changes, &wording)
    })
}

fn ensure_chapter_exists(existing: &[ChapterId], chapter: ChapterId) -> Result<()> {
    if existing.contains(&chapter) {
        return Ok(());
    }
    Err(EngineError::NotFound(format!(
        "第{}章（{}）がありません。",
        chapter.number(),
        layout::chapter_path(&chapter)
    )))
}

/// 番号が条件に合う章。
fn chapters_where(existing: &[ChapterId], keep: impl Fn(u32) -> bool) -> Vec<ChapterId> {
    existing
        .iter()
        .copied()
        .filter(|id| keep(id.number()))
        .collect()
}

/// 末尾に足す章の id。最大の番号の次で、章が無ければ 01。
fn next_chapter_id(existing: &[ChapterId]) -> Result<ChapterId> {
    let Some(last) = existing.iter().max_by_key(|id| id.number()) else {
        return Ok(ChapterId::from_number(1));
    };
    last.shifted(1).ok_or_else(too_many_chapters)
}

fn too_many_chapters() -> EngineError {
    EngineError::InvalidInput("章は 999 までです。これより後ろには足せません。".into())
}

/// `chapters` の章立てと本文のフォルダを、番号を `delta` ずらした場所へ改名する変更を加える。
/// 本文のフォルダがまだ無い章には、適用のときに移動元にまだ無いことと、移動先が空いたままであることの
/// 確認を加える。
fn renumber(
    project: &Project,
    changes: &mut ChangeSet,
    chapters: &[ChapterId],
    delta: i32,
) -> Result<Vec<RenumberedChapter>> {
    let mut renumbered = Vec::with_capacity(chapters.len());
    let mut destinations_without_text = Vec::new();
    for &from in chapters {
        let to = from.shifted(delta).ok_or_else(too_many_chapters)?;
        changes.move_entry(layout::chapter_path(&from), layout::chapter_path(&to));
        let text_dir = layout::manuscript_chapter_dir(&from);
        if project.store().exists(&text_dir) {
            changes.move_entry(text_dir, layout::manuscript_chapter_dir(&to));
        } else {
            changes.expect(text_dir, None);
            destinations_without_text.push(layout::manuscript_chapter_dir(&to));
        }
        renumbered.push(RenumberedChapter {
            from,
            to,
            title: chapter_title(project, from),
        });
    }
    // 全部の改名を加えてから確かめる（行き先を同じ変更案の改名が空けるかどうかは、並べ終えないと分からない）
    for destination in destinations_without_text {
        if !is_already_decided(changes, &destination) {
            changes.expect(destination, None);
        }
    }
    Ok(renumbered)
}

/// 同じ変更案が、`path` の今の状態を既に扱っているか（移動元・ゴミ箱・状態の確認のどれかになっているか）。
///
/// 移動元かゴミ箱なら、中身は適用のときに別の場所へ移るので空く。状態の確認なら、同じ確認を重ねない
/// （同じパスへの確認が重なるのは形の誤りになる）。
fn is_already_decided(changes: &ChangeSet, path: &RelPath) -> bool {
    changes.files.iter().any(|change| match change {
        FileChange::Move { from: decided, .. }
        | FileChange::Trash { path: decided, .. }
        | FileChange::Expect { path: decided, .. } => decided == path,
        FileChange::Write { .. } => false,
    })
}

/// 消す章の本文のフォルダを、中のファイル全部ごとゴミ箱へ移す変更を加える。フォルダが無ければ、
/// 適用のときにまだ無いことの確認を加える。
fn trash_text_folder(project: &Project, changes: &mut ChangeSet, chapter: ChapterId) -> Result<()> {
    let text_dir = layout::manuscript_chapter_dir(&chapter);
    let Some(files) = project.store().read_folder(&text_dir)? else {
        changes.expect(text_dir, None);
        return Ok(());
    };
    if let Some(unreadable) = files.iter().find(|file| file.text.is_none()) {
        return Err(EngineError::InvalidInput(format!(
            "{text_dir} の中に、テキストとして読めないファイル（{}）があるため、章を消せません。\
             先に取り除くか、別の場所へ移してください。",
            unreadable.path
        )));
    }
    changes.trash_folder(text_dir, &files);
    Ok(())
}

/// 章が置かれる場所（章立てのファイルと本文のフォルダ）が、利用者の気付かないファイルやフォルダで
/// 塞がっていれば、分かりやすく断る。
///
/// 本文のフォルダが無い章をずらすときも確かめる（行き先に章立ての無い本文が残っていると、ずらした章の本文として
/// 読まれてしまうため）。同じ変更案のゴミ箱や改名で空く場所は塞がっていない。適用のときにも同じことを確かめるが、
/// そこでは競合（「外で変更された」）としか言えない。
fn ensure_slots_free(
    project: &Project,
    changes: &ChangeSet,
    chapters: impl IntoIterator<Item = ChapterId>,
) -> Result<()> {
    let vacated: HashSet<String> = changes
        .files
        .iter()
        .filter_map(|change| match change {
            FileChange::Move { from, .. } => Some(from),
            FileChange::Trash { path, .. } => Some(path),
            FileChange::Write { .. } | FileChange::Expect { .. } => None,
        })
        .map(|path| path.as_str().to_lowercase())
        .collect();
    for chapter in chapters {
        let slots = [
            layout::chapter_path(&chapter),
            layout::manuscript_chapter_dir(&chapter),
        ];
        for slot in slots {
            let is_blocked =
                project.store().exists(&slot) && !vacated.contains(&slot.as_str().to_lowercase());
            if is_blocked {
                return Err(EngineError::InvalidInput(format!(
                    "{slot} が既にあるため、第{}章の置き場所にできません。\
                     {slot} を別の場所へ移すか削除してから、もう一度操作してください。",
                    chapter.number()
                )));
            }
        }
    }
    Ok(())
}

/// 新しい章の章立て。章題とストーリーラインだけで、シーン構成は無い。
fn render_new_chapter(id: ChapterId, title: &str, storyline: &str) -> Result<String> {
    let storyline = storyline.trim();
    // ファイルの末尾は改行で終える（ストーリーラインが無ければ、front matter の後は空）
    let storyline = if storyline.is_empty() {
        String::new()
    } else {
        format!("{storyline}\n")
    };
    let chapter = Chapter {
        id,
        meta: ChapterMeta {
            title: title.to_owned(),
            scenes: Vec::new(),
            extra: BTreeMap::new(),
        },
        storyline,
    };
    Ok(chapter.render()?)
}

/// 利用者に見せる章の呼び方。章題が読めなければ番号だけ。
fn chapter_label(id: ChapterId, title: Option<&str>) -> String {
    match title {
        Some(title) => format!("第{}章「{title}」", id.number()),
        None => format!("第{}章", id.number()),
    }
}

/// 章題。章立てが読めなければ（壊れている・無い）`None`。
fn chapter_title(project: &Project, id: ChapterId) -> Option<String> {
    let file = project
        .store()
        .read_text_opt(&layout::chapter_path(&id))
        .ok()??;
    title_of(&file.content)
}

fn title_of(content: &str) -> Option<String> {
    let document = frontmatter::parse::<ChapterMeta>(content).ok()?;
    let title = document.meta.title.trim();
    (!title.is_empty()).then(|| title.to_owned())
}
