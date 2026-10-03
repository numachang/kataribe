//! 作品の構成（人物・世界観の資料・章・シーン）を、利用者が自分で書いて足したり消したり、並べ替えたりする操作。
//!
//! LLM も設定も使わないので、[`crate::Engine`] の外の関数にしてある（GUI と CLI が同じ関数を使う）。
//! どの操作も作品フォルダを直接書き換えず、変更案（[`crate::ChangeSet`]）を [`StructurePlan`] に入れて返す。
//! 追加と並べ替えは本人が指示した内容をそのまま反映してよいが、削除はゴミ箱へ移るものと参照が切れるものを
//! 見せてから適用する（その材料を [`StructurePlan`] が持つ）。

mod chapters;
mod characters;
mod edit;
mod plan;
mod position;
mod references;
mod scenes;
mod world;

use kataribe_project::{CharacterId, Project};

pub(crate) use world::check_name as check_world_document_name;

pub use edit::{NewScenePlan, StructureEdit};
pub use plan::{RenumberedChapter, SceneReference, StructurePlan};

use crate::error::Result;

/// `edit` の変更案を作る。作品フォルダは書き換えない。
///
/// 返す変更案は `project` のものとして印が付いている。適用は [`crate::ChangeSet::apply`]。
/// 作る前に、入力の誤り（名前が空・使用済みの ID・消せない資料など）と、対象が見つからないことを確かめる。
pub fn plan_structure_edit(project: &Project, edit: &StructureEdit) -> Result<StructurePlan> {
    let plan = match edit {
        StructureEdit::AddCharacter { id, meta, body } => {
            characters::add(project, id.as_deref(), meta, body)?
        }
        StructureEdit::RemoveCharacter { path } => characters::remove(project, path)?,
        StructureEdit::AddWorldDocument { name, title, body } => {
            world::add(project, name.as_deref(), title, body)?
        }
        StructureEdit::RemoveWorldDocument { path } => world::remove(project, path)?,
        StructureEdit::AddChapter {
            before,
            title,
            storyline,
        } => chapters::add(project, *before, title, storyline)?,
        StructureEdit::RemoveChapter { chapter } => chapters::remove(project, *chapter)?,
        StructureEdit::AddScene {
            chapter,
            before,
            scene,
        } => scenes::add(project, *chapter, *before, scene)?,
        StructureEdit::RemoveScene { chapter, scene } => scenes::remove(project, *chapter, *scene)?,
        StructureEdit::MoveCharacter { path, position } => {
            characters::move_to(project, path, *position)?
        }
        StructureEdit::MoveChapter { chapter, position } => {
            chapters::move_to(project, *chapter, *position)?
        }
        StructureEdit::MoveScene {
            chapter,
            scene,
            position,
        } => scenes::move_to(project, *chapter, *scene, *position)?,
    };
    Ok(plan.made_for(project))
}

/// 人物の ID の案を、読み（かな）からローマ字で作る。
///
/// 読みをローマ字にできなければ名前を使い、それでも作れなければ `character`。
/// 使用済みの ID と重なるときは `-2`, `-3`, … を付ける。追加の画面が、入力中の名前から提案を出すのに使う。
pub fn suggest_character_id(project: &Project, reading: &str, name: &str) -> Result<CharacterId> {
    characters::suggest_id(project, reading, name)
}
