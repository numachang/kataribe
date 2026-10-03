//! 指示から人物を 1 人作って足す。項目（JSON）と人物資料の本文を、LLM を 2 回呼んで一度に作る。
//!
//! 人物の ID は LLM に出させず、読みからローマ字で決める（利用者が自分で足すときと同じ）。

use kataribe_project::{Character, CharacterMeta, layout};
use minijinja::context;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{Background, announce, plan_generated_addition, require_instruction};
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::names::same_person;
use crate::prompt::{Prompt, PromptTemplate};
use crate::stages::Stage;
use crate::stages::documents::write_document;
use crate::stages::materials::{CharacterBrief, briefs, document_body, non_empty};
use crate::structure::StructureEdit;

const ENTRY_OUTPUT_TOKENS: u32 = 1024;
const PROFILE_OUTPUT_TOKENS: u32 = 3072;
const SYNOPSIS_CHARS: usize = 1500;
const ENTRY_LABEL: &str = "人物の項目を生成";

/// LLM が答える、新しい人物の項目。ID は含めない（読みから決める）。
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewCharacter {
    name: String,
    reading: String,
    role: String,
    summary: String,
}

impl NewCharacter {
    fn trimmed(self) -> Self {
        Self {
            name: self.name.trim().to_owned(),
            reading: self.reading.trim().to_owned(),
            role: self.role.trim().to_owned(),
            summary: self.summary.trim().to_owned(),
        }
    }

    /// 人物の項目にする。表示順は決めない（足すときに末尾になる）。
    fn into_meta(self) -> CharacterMeta {
        CharacterMeta {
            name: self.name,
            reading: non_empty(&self.reading),
            role: self.role,
            summary: self.summary,
            ..CharacterMeta::default()
        }
    }
}

/// 指示から人物を 1 人作り、`characters/<id>.md` を新しく書く変更案を返す。
pub(in crate::stages) async fn add_character(
    stage: &Stage<'_>,
    instruction: &str,
) -> Result<ChangeSet> {
    let instruction = require_instruction(instruction)?;
    let cast = stage.project.characters()?;
    let background = Background::gather(stage.project)?;
    let synopsis = document_body(stage.project, layout::SYNOPSIS)?;
    // 項目と本文で、LLM を 2 回呼ぶ
    stage.caller.expect_steps(1);

    let entry_prompt = stage.prompts.render(
        PromptTemplate::AddCharacter,
        &context! {
            project => &stage.info,
            concept => &background.concept,
            world => &background.world,
            synopsis => excerpt::head(&synopsis, SYNOPSIS_CHARS),
            cast => briefs(&cast, 0),
            instruction => instruction,
        },
    )?;
    let meta = generate_entry(stage, &cast, &entry_prompt)
        .await?
        .into_meta();
    let body = write_profile(stage, &cast, &background, &meta, instruction).await?;

    let subject = format!("人物「{}」", meta.name);
    let edit = StructureEdit::AddCharacter {
        id: None,
        meta,
        body,
    };
    let changes = plan_generated_addition(stage, &edit, &subject)?;
    announce_outdated_documents(stage, &synopsis)?;
    Ok(changes)
}

/// 人物の項目を生成する。問題があれば、設定の回数まで生成し直す。
///
/// 直らなくても、名前があれば警告して使う（利用者が確認画面で直せる）。名前が空なら人物にならないので失敗にする。
async fn generate_entry(
    stage: &Stage<'_>,
    cast: &[Character],
    prompt: &Prompt,
) -> Result<NewCharacter> {
    let output_tokens = stage.output_tokens(prompt, ENTRY_OUTPUT_TOKENS)?;
    let mut retries_left = stage.settings.quality_retries;
    loop {
        let entry: NewCharacter = stage
            .caller
            .json(ENTRY_LABEL, prompt, output_tokens)
            .await?;
        let entry = entry.trimmed();
        let problems = entry_problems(&entry, cast);
        if problems.is_empty() {
            return Ok(entry);
        }
        let problems = problems.concat();
        if retries_left == 0 {
            if entry.name.is_empty() {
                return Err(EngineError::InvalidOutput("人物の名前が空でした。".into()));
            }
            notice(
                stage.caller.sink(),
                NoticeLevel::Warning,
                format!("{problems}確認して直してください。"),
            );
            return Ok(entry);
        }
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!("{problems}生成し直します。"),
        );
        stage.caller.expect_steps(1);
        retries_left -= 1;
    }
}

/// 項目の、生成し直す理由になる問題。1 つの問題が 1 文で、文の終わりは「。」。
fn entry_problems(entry: &NewCharacter, cast: &[Character]) -> Vec<String> {
    let mut problems = Vec::new();
    if entry.name.is_empty() {
        problems.push("人物の名前が空です。".to_owned());
    } else {
        if entry.name.chars().any(|c| c.is_ascii_alphabetic()) {
            problems.push(format!(
                "人物の名前にローマ字が混ざっています（{}）。",
                entry.name
            ));
        }
        if let Some(existing) = cast
            .iter()
            .find(|character| same_person(&character.meta.name, &entry.name))
        {
            problems.push(format!(
                "すでにいる人物「{}」と同じ名前です。",
                existing.meta.name
            ));
        }
    }
    // 人物の ID は読みからローマ字で作るので、かな以外が混ざると ID が崩れる
    if !is_kana_reading(&entry.reading) {
        problems.push(format!(
            "読みがひらがな・カタカナだけになっていません（{}）。",
            entry.reading
        ));
    }
    problems
}

/// 読みとして使える文字（かな・長音・「・」・空白）だけで書かれているか。空は使えない。
fn is_kana_reading(reading: &str) -> bool {
    !reading.is_empty()
        && reading.chars().all(|character| {
            matches!(character, 'ぁ'..='ゖ' | 'ァ'..='ヺ' | 'ー' | '・')
                || character.is_whitespace()
        })
}

/// 人物資料の本文を書く。一覧には、いま足す人物も入れる（一覧のほかの人物との関係を書かせるため）。
async fn write_profile(
    stage: &Stage<'_>,
    cast: &[Character],
    background: &Background,
    meta: &CharacterMeta,
    instruction: &str,
) -> Result<String> {
    let newcomer = CharacterBrief::from_meta(meta, "", 0);
    let mut everyone = briefs(cast, 0);
    everyone.push(newcomer.clone());
    let prompt = stage.prompts.render(
        PromptTemplate::Character,
        &context! {
            project => &stage.info,
            concept => &background.concept,
            world => &background.world,
            cast => everyone,
            character => newcomer,
            instruction => instruction,
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, PROFILE_OUTPUT_TOKENS)?;
    let label = format!("{}の人物資料を生成", meta.name);
    write_document(stage, &label, &prompt, output_tokens).await
}

/// 作った人物が出てこない、生成済みの文書があれば知らせる（工程の印は付けず、注意書きだけにする）。
fn announce_outdated_documents(stage: &Stage<'_>, synopsis: &str) -> Result<()> {
    let chapters = stage.project.chapters()?;
    let outdated: Vec<&str> = [
        (!synopsis.is_empty(), "あらすじ"),
        (!chapters.is_empty(), "章立て"),
        (
            chapters
                .iter()
                .any(|chapter| !chapter.meta.scenes.is_empty()),
            "シーン構成",
        ),
    ]
    .into_iter()
    .filter_map(|(exists, label)| exists.then_some(label))
    .collect();
    if !outdated.is_empty() {
        announce(
            stage,
            format!(
                "生成済みの{}には、この人物はまだ出てきません。必要なら、書き直すか作り直してください。",
                outdated.join("・")
            ),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kataribe_project::CharacterId;

    use super::*;

    fn entry(name: &str, reading: &str) -> NewCharacter {
        NewCharacter {
            name: name.to_owned(),
            reading: reading.to_owned(),
            role: "相棒".to_owned(),
            summary: "助手を務める青年。".to_owned(),
        }
    }

    fn existing(name: &str) -> Character {
        Character {
            id: CharacterId::new("rin").unwrap(),
            meta: CharacterMeta {
                name: name.to_owned(),
                ..CharacterMeta::default()
            },
            body: String::new(),
        }
    }

    #[test]
    fn a_japanese_name_with_a_kana_reading_has_no_problem() {
        let problems = entry_problems(&entry("佐藤 健二", "さとう けんじ"), &[existing("霧島 凛")]);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn an_empty_name_is_a_problem() {
        let problems = entry_problems(&entry("", "さとう"), &[]);
        assert_eq!(problems, ["人物の名前が空です。"]);
    }

    #[test]
    fn a_name_with_latin_letters_is_a_problem() {
        let problems = entry_problems(&entry("田中 Shukichi", "たなか しゅうきち"), &[]);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("ローマ字"), "{problems:?}");
    }

    #[test]
    fn a_name_that_an_existing_character_has_is_a_problem_even_without_the_space() {
        let problems = entry_problems(&entry("霧島凛", "きりしま りん"), &[existing("霧島 凛")]);
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].contains("すでにいる人物「霧島 凛」"),
            "{problems:?}"
        );
    }

    #[test]
    fn a_surname_shared_with_an_existing_character_is_not_a_problem() {
        let problems = entry_problems(&entry("霧島 蓮", "きりしま れん"), &[existing("霧島 凛")]);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn a_reading_must_be_made_of_kana() {
        for reading in ["さとう けんじ", "サトウ・ケンジ", "ゆうこ", "らーめん"]
        {
            assert!(is_kana_reading(reading), "{reading}");
        }
        for reading in ["", "佐藤 健二", "sato kenji", "さとう Kenji"] {
            assert!(!is_kana_reading(reading), "{reading}");
        }
    }

    #[test]
    fn an_empty_or_kanji_reading_is_reported_as_a_problem() {
        assert_eq!(entry_problems(&entry("佐藤 健二", ""), &[]).len(), 1);
        assert_eq!(entry_problems(&entry("佐藤 健二", "佐藤"), &[]).len(), 1);
    }
}
