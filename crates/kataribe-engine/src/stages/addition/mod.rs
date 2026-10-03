//! 指示から人物・世界観の資料を作って足す生成。
//!
//! 流れはどちらも同じ。指示と足す名前を LLM に頼む前に確かめ、文脈を集めて作らせ、出力を検証してから、
//! 構成の操作（[`StructureEdit`]）の変更案にする。ファイルの書き方・人物の ID・資料のファイル名・表示順は、
//! 利用者が自分で足すときと同じ関数（[`plan_structure_edit`]）に任せる。
//!
//! 変更案の基準（`base_hash`）は、LLM の出力を受け取ったあとの状態になる。構成の操作の関数が最新の状態を読むので、
//! 生成している間に保存された手の編集を、古い状態を前提にして上書きしない。

mod character;
mod world;

use kataribe_project::{Project, layout};

pub(super) use character::add_character;
pub(super) use world::add_world_document;

use super::Stage;
use super::materials::{document_body, world_text};
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::structure::{StructureEdit, plan_structure_edit};

const CONCEPT_CHARS: usize = 2000;
const WORLD_CHARS: usize = 2000;

/// 作品の企画と世界観の抜粋。どちらの生成でも、LLM に渡す背景になる。
struct Background {
    concept: String,
    world: String,
}

impl Background {
    fn gather(project: &Project) -> Result<Self> {
        Ok(Self {
            concept: excerpt::head(&document_body(project, layout::CONCEPT)?, CONCEPT_CHARS),
            world: excerpt::head(&world_text(project)?, WORLD_CHARS),
        })
    }
}

/// 足す内容の指示を取り出す。空なら、LLM を呼ぶ前に入力の誤りとして断る。
fn require_instruction(instruction: &str) -> Result<&str> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err(EngineError::InvalidInput(
            "作らせたい内容の指示を入力してください。".into(),
        ));
    }
    Ok(instruction)
}

/// `edit` を構成の操作として計画し、生成の変更案にする。`subject` は「人物「霧島 凛」」のような、作ったものの呼び名。
///
/// 計画の注意書きは、生成の注意書きとして流す。
fn plan_generated_addition(
    stage: &Stage<'_>,
    edit: &StructureEdit,
    subject: &str,
) -> Result<ChangeSet> {
    let plan = plan_structure_edit(stage.project, edit)?;
    for message in &plan.notices {
        announce(stage, message);
    }
    let summary = match &plan.created {
        Some(path) => format!("{subject}を生成しました（{path}）。"),
        None => format!("{subject}を生成しました。"),
    };
    Ok(ChangeSet {
        summary,
        ..plan.change_set
    })
}

/// 利用者に知らせたい情報を、Info の注意書きとして流す。
fn announce(stage: &Stage<'_>, message: impl Into<String>) {
    notice(stage.caller.sink(), NoticeLevel::Info, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_instruction_is_refused_and_a_real_one_is_trimmed() {
        assert!(matches!(
            require_instruction(" \n　"),
            Err(EngineError::InvalidInput(_))
        ));
        assert_eq!(
            require_instruction("  敵対者を足す \n").unwrap(),
            "敵対者を足す"
        );
    }
}
