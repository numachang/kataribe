//! 指示から世界観の資料を 1 つ作って足す。
//!
//! 出力は JSON にせず、文章にして 1 行目を `# 題` にさせる（ローカル LLM は、長い Markdown を JSON の文字列に
//! 入れるのが苦手なため）。

use minijinja::context;

use super::{Background, announce, plan_generated_addition, require_instruction};
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};
use crate::prompt::PromptTemplate;
use crate::stages::Stage;
use crate::stages::documents::write_checked_document;
use crate::stages::materials::briefs;
use crate::structure::{StructureEdit, check_world_document_name};

const DOCUMENT_OUTPUT_TOKENS: u32 = 4096;
const DOCUMENT_LABEL: &str = "世界観の資料を生成";

/// 指示から世界観の資料を 1 つ作り、`world/<name>.md` を新しく書く変更案を返す。
///
/// `name` が `None` なら、題からファイル名を決める。指定があるときは、LLM を待たせないよう先に確かめる。
pub(in crate::stages) async fn add_world_document(
    stage: &Stage<'_>,
    name: Option<&str>,
    instruction: &str,
) -> Result<ChangeSet> {
    let instruction = require_instruction(instruction)?;
    let name = check_world_document_name(stage.project, name)?;
    let background = Background::gather(stage.project)?;
    let existing_titles: Vec<String> = stage
        .project
        .world_docs()?
        .iter()
        .map(|(path, document)| document.display_title(path.file_stem()))
        .collect();
    let cast = stage.project.characters()?;
    let prompt = stage.prompts.render(
        PromptTemplate::AddWorldDocument,
        &context! {
            project => &stage.info,
            concept => &background.concept,
            world => &background.world,
            titles => existing_titles,
            cast => briefs(&cast, 0),
            instruction => instruction,
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, DOCUMENT_OUTPUT_TOKENS)?;
    let written = write_checked_document(stage, DOCUMENT_LABEL, &prompt, output_tokens, |text| {
        split_title(text, &existing_titles).err()
    })
    .await?;
    let document = split_title(&written, &existing_titles).map_err(EngineError::InvalidOutput)?;

    let subject = format!("世界観の資料「{}」", document.title);
    let edit = StructureEdit::AddWorldDocument {
        name: name.map(|name| name.to_string()),
        title: document.title,
        body: document.body,
    };
    let changes = plan_generated_addition(stage, &edit, &subject)?;
    announce(
        stage,
        "この資料は、これからの生成で世界観として使われます（長いと、切り詰められることがあります）。\
         生成済みの文書には反映されません。",
    );
    Ok(changes)
}

/// 先頭の見出しで分けた資料。
#[derive(Debug, PartialEq, Eq)]
struct TitledDocument {
    title: String,
    body: String,
}

/// 資料の先頭の行（`# 題`）を題として取り出し、残りを本文にする。
///
/// 作り直す理由になるときは、その説明を返す。題が無い（先頭が見出しでない・節の見出し `##` だった・題が空）、
/// 本文が空、題が `existing_titles` のどれかと同じ（目次に同じ名前が 2 つ並んでしまう）のどれか。
fn split_title(
    text: &str,
    existing_titles: &[String],
) -> std::result::Result<TitledDocument, String> {
    let text = text.trim_start();
    let (first_line, rest) = text.split_once('\n').unwrap_or((text, ""));
    let title = first_line
        .trim()
        .strip_prefix('#')
        .filter(|after_mark| !after_mark.starts_with('#'))
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .ok_or_else(|| "1 行目が「# 題」の形になっていません。".to_owned())?;
    let body = rest.trim();
    if body.is_empty() {
        return Err("題の下に本文がありません。".to_owned());
    }
    if existing_titles
        .iter()
        .any(|existing| existing.trim() == title)
    {
        return Err(format!("すでにある資料と同じ題「{title}」です。"));
    }
    Ok(TitledDocument {
        title: title.to_owned(),
        body: body.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_heading_becomes_the_title_and_the_rest_the_body() {
        let document =
            split_title("\n# 港町の歴史\n\n## 成り立ち\n江戸の頃に開かれた。\n", &[]).unwrap();
        assert_eq!(
            document,
            TitledDocument {
                title: "港町の歴史".to_owned(),
                body: "## 成り立ち\n江戸の頃に開かれた。".to_owned(),
            }
        );
    }

    #[test]
    fn a_document_with_only_a_title_is_refused_for_lack_of_a_body() {
        for text in ["# 用語集", "# 用語集\n", "# 用語集\n\n  \n　\n"] {
            let reason = split_title(text, &[]).unwrap_err();
            assert!(reason.contains("本文"), "{text:?}: {reason}");
        }
    }

    #[test]
    fn a_title_that_an_existing_document_already_has_is_refused() {
        let existing = ["世界観".to_owned(), "港町の歴史".to_owned()];

        let reason = split_title("# 港町の歴史\n江戸の頃。", &existing).unwrap_err();

        assert!(reason.contains("港町の歴史"), "{reason}");
        assert!(split_title("# 港町の暮らし\n漁が盛ん。", &existing).is_ok());
    }

    #[test]
    fn text_that_does_not_start_with_a_title_heading_is_refused() {
        for text in [
            "港町の歴史\n江戸の頃に開かれた。",
            "## 成り立ち\n江戸の頃。",
            "#\n本文",
            "# \n本文",
            "",
        ] {
            assert!(split_title(text, &[]).is_err(), "{text:?}");
        }
    }
}
