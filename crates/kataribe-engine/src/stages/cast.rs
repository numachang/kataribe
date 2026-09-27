//! 登場人物の一覧と、一人ずつの人物資料の生成。

use std::collections::BTreeMap;

use kataribe_project::{Character, CharacterId, CharacterMeta, layout};
use minijinja::context;
use schemars::JsonSchema;
use serde::Deserialize;

use super::Stage;
use super::documents::write_document;
use super::materials::{
    CharacterBrief, briefs, cast_size_for, document_body, non_empty, require, snapshot_character,
    world_text,
};
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::excerpt;
use crate::prompt::PromptTemplate;

const ROSTER_OUTPUT_TOKENS: u32 = 3072;
const PROFILE_OUTPUT_TOKENS: u32 = 3072;
const CONCEPT_CHARS: usize = 2000;
const WORLD_CHARS: usize = 2000;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Roster {
    characters: Vec<RosterEntry>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RosterEntry {
    id: String,
    name: String,
    reading: String,
    role: String,
    summary: String,
}

/// 登場人物の一覧を作り、1 人 1 ファイルの骨組み（front matter だけ）を書く。
/// すでにいる人物（名前が一致する人物）は、資料の本文を残して項目だけを更新する。
pub(super) async fn roster(stage: &Stage<'_>) -> Result<ChangeSet> {
    let concept = document_body(stage.project, layout::CONCEPT)?;
    require(!concept.is_empty(), "先に企画を生成してください。")?;
    let existing = stage
        .project
        .characters()?
        .iter()
        .map(|character| snapshot_character(stage.project, &character.id))
        .collect::<Result<Vec<_>>>()?;
    let prompt = stage.prompts.render(
        PromptTemplate::Cast,
        &context! {
            project => &stage.info,
            concept => excerpt::head(&concept, CONCEPT_CHARS),
            world => excerpt::head(&world_text(stage.project)?, WORLD_CHARS),
            cast_size => cast_size_for(stage.manifest.target_length),
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, ROSTER_OUTPUT_TOKENS)?;
    let roster: Roster = stage
        .caller
        .json("登場人物の一覧を生成", &prompt, output_tokens)
        .await?;
    if roster.characters.is_empty() {
        return Err(EngineError::InvalidOutput(
            "登場人物が一人もいませんでした。".into(),
        ));
    }

    let mut taken: Vec<CharacterId> = existing
        .iter()
        .map(|(character, _)| character.id.clone())
        .collect();
    let mut changes = ChangeSet::new(format!(
        "登場人物 {} 人を生成しました。",
        roster.characters.len()
    ));
    for (index, entry) in roster.characters.into_iter().enumerate() {
        let order = u32::try_from(index + 1).ok();
        let current = existing
            .iter()
            .find(|(character, _)| same_person(&character.meta.name, &entry.name));
        let (character, base) = if let Some((character, base)) = current {
            (
                merge_entry(character.clone(), entry, order),
                Some(base.clone()),
            )
        } else {
            let id = CharacterId::from_hint(&entry.id, &taken);
            taken.push(id.clone());
            (new_character(id, entry, order), None)
        };
        changes.put(
            layout::character_path(&character.id),
            character.render()?,
            base,
        );
    }
    Ok(changes)
}

/// 1 人の人物資料（本文）を書く。
pub(super) async fn profile(stage: &Stage<'_>, id: &CharacterId) -> Result<ChangeSet> {
    let (character, base) = snapshot_character(stage.project, id)?;
    let cast = stage.project.characters()?;
    let concept = document_body(stage.project, layout::CONCEPT)?;
    let prompt = stage.prompts.render(
        PromptTemplate::Character,
        &context! {
            project => &stage.info,
            concept => excerpt::head(&concept, CONCEPT_CHARS),
            world => excerpt::head(&world_text(stage.project)?, WORLD_CHARS),
            cast => briefs(&cast, 0),
            character => CharacterBrief::new(&character, 0),
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, PROFILE_OUTPUT_TOKENS)?;
    let label = format!("{}の人物資料を生成", character.meta.name);
    let body = write_document(stage, &label, &prompt, output_tokens).await?;
    let name = character.meta.name.clone();
    let updated = Character { body, ..character };
    let mut changes = ChangeSet::new(format!("{name}の人物資料を生成しました。"));
    changes.put(
        layout::character_path(&updated.id),
        updated.render()?,
        Some(base),
    );
    Ok(changes)
}

fn new_character(id: CharacterId, entry: RosterEntry, order: Option<u32>) -> Character {
    Character {
        id,
        meta: CharacterMeta {
            name: entry.name,
            reading: non_empty(&entry.reading),
            role: entry.role,
            summary: entry.summary,
            order,
            extra: BTreeMap::default(),
        },
        body: String::new(),
    }
}

fn merge_entry(mut character: Character, entry: RosterEntry, order: Option<u32>) -> Character {
    character.meta.reading = non_empty(&entry.reading).or(character.meta.reading);
    character.meta.role = entry.role;
    character.meta.summary = entry.summary;
    character.meta.order = order;
    character
}

/// 空白の有無を無視して名前が一致するか（「霧島 凛」と「霧島凛」を同一人物とみなす）。
pub(crate) fn same_person(left: &str, right: &str) -> bool {
    let squeeze = |name: &str| {
        name.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
    };
    squeeze(left) == squeeze(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_regardless_of_spacing() {
        assert!(same_person("霧島 凛", "霧島凛"));
        assert!(same_person("霧島　凛", "霧島 凛"));
        assert!(!same_person("霧島 凛", "霧島 蓮"));
    }
}
