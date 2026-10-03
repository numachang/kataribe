//! 作家の指示による資料・本文の書き直し。

use kataribe_project::{ChapterMeta, CharacterMeta, RelPath, frontmatter, layout};
use minijinja::context;

use super::Stage;
use super::materials::document_body;
use crate::change_set::ChangeSet;
use crate::cleanup::{clean_front_matter_document, clean_markdown, clean_revision};
use crate::error::{EngineError, Result};
use crate::excerpt;
use crate::prompt::PromptTemplate;

const CONCEPT_CHARS: usize = 1500;
const STYLE_CHARS: usize = 1500;

/// 書き直す文書の種類。種類によって後処理と検証が変わる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocumentKind {
    Manuscript,
    Character,
    Chapter,
    Markdown,
}

impl DocumentKind {
    fn of(path: &RelPath) -> Result<Self> {
        let is_manuscript = path
            .as_str()
            .strip_prefix(layout::MANUSCRIPT_DIR)
            .is_some_and(|rest| rest.starts_with('/'));
        match path.extension() {
            Some("txt") if is_manuscript => Ok(Self::Manuscript),
            // 人物資料・章立ては、画面や一覧が扱うファイルと同じ判定（layout）に従う
            Some("md") => Ok(match layout::document_kind(path) {
                layout::DocumentKind::Character(_) => Self::Character,
                layout::DocumentKind::Chapter(_) => Self::Chapter,
                layout::DocumentKind::Other => Self::Markdown,
            }),
            _ => Err(EngineError::InvalidInput(format!(
                "{path} は書き直しの対象にできません（Markdown の資料と本文だけが対象です）。"
            ))),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Manuscript => "小説の本文",
            Self::Character => "人物資料",
            Self::Chapter => "章のストーリーラインとシーン構成",
            Self::Markdown => "設定資料",
        }
    }
}

pub(super) async fn document(
    stage: &Stage<'_>,
    path: &RelPath,
    instruction: &str,
) -> Result<ChangeSet> {
    if instruction.trim().is_empty() {
        return Err(EngineError::InvalidInput(
            "書き直しの指示を入力してください。".into(),
        ));
    }
    let kind = DocumentKind::of(path)?;
    let current = stage
        .project
        .store()
        .read_text_opt(path)?
        .ok_or_else(|| EngineError::NotFound(format!("{path} がありません。")))?;
    let has_front_matter = current.content.starts_with("---");
    let prompt = stage.prompts.render(
        PromptTemplate::Revise,
        &context! {
            project => &stage.info,
            concept => excerpt::head(&document_body(stage.project, layout::CONCEPT)?, CONCEPT_CHARS),
            style => excerpt::head(&document_body(stage.project, layout::STYLE)?, STYLE_CHARS),
            document_label => format!("{}（{path}）", kind.label()),
            content => &current.content,
            instruction => instruction.trim(),
            has_front_matter => has_front_matter,
            is_manuscript => kind == DocumentKind::Manuscript,
        },
    )?;
    let length =
        u32::try_from(kataribe_text::count::count_chars(&current.content)).unwrap_or(u32::MAX);
    let output_tokens = stage.output_tokens(&prompt, stage.output_tokens_for_chars(length))?;
    let output = stage
        .caller
        .text(&format!("{path} を書き直し"), &prompt, output_tokens)
        .await?;
    if output.truncated {
        // 途中で切れた書き直しを適用すると、文書の後半が失われる
        return Err(EngineError::InvalidOutput(format!(
            "{path} の書き直しが途中で切れました。文脈の長さを増やすか、指示を分けて試してください。"
        )));
    }
    let revised = match (kind, has_front_matter) {
        (DocumentKind::Manuscript, _) => clean_revision(&output.text),
        (_, true) => clean_front_matter_document(&output.text),
        (_, false) => clean_markdown(&output.text),
    };
    validate(kind, &revised)?;

    let mut changes = ChangeSet::new(format!("{path} を指示に従って書き直しました。"));
    changes.put(path.clone(), revised, Some(current));
    Ok(changes)
}

/// 書き直した結果が、その種類の文書として読めるか確かめる（YAML が壊れていないか）。
fn validate(kind: DocumentKind, revised: &str) -> Result<()> {
    if revised.trim().is_empty() {
        return Err(EngineError::InvalidOutput(
            "書き直した結果が空でした。".into(),
        ));
    }
    let broken = |error: kataribe_project::YamlError| {
        EngineError::InvalidOutput(format!(
            "書き直した結果の形式が崩れています（{error}）。もう一度試してください。"
        ))
    };
    match kind {
        DocumentKind::Character => frontmatter::parse::<CharacterMeta>(revised)
            .map(drop)
            .map_err(broken),
        DocumentKind::Chapter => frontmatter::parse::<ChapterMeta>(revised)
            .map(drop)
            .map_err(broken),
        DocumentKind::Manuscript | DocumentKind::Markdown => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(path: &str) -> Result<DocumentKind> {
        DocumentKind::of(&RelPath::new(path).unwrap())
    }

    #[test]
    fn document_kinds_follow_the_project_layout() {
        assert_eq!(
            kind("manuscript/01/s01.txt").unwrap(),
            DocumentKind::Manuscript
        );
        assert_eq!(kind("characters/rin.md").unwrap(), DocumentKind::Character);
        assert_eq!(kind("plot/chapters/01.md").unwrap(), DocumentKind::Chapter);
        assert_eq!(kind("world/overview.md").unwrap(), DocumentKind::Markdown);
        assert!(kind("kataribe.yaml").is_err());
    }

    #[test]
    fn files_that_the_project_does_not_treat_as_characters_or_chapters_are_plain_markdown() {
        for path in [
            "characters/old/rin.md",
            "characters/霧島.md",
            "plot/chapters/draft/01.md",
            "plot/chapters/1.md",
        ] {
            assert_eq!(kind(path).unwrap(), DocumentKind::Markdown, "path: {path}");
        }
    }

    #[test]
    fn broken_front_matter_is_rejected() {
        let broken = "---\nname: [\n---\n本文\n";
        assert!(validate(DocumentKind::Character, broken).is_err());
        let valid = "---\nname: 凛\nrole: 主人公\nsummary: 探偵\n---\n本文\n";
        assert!(validate(DocumentKind::Character, valid).is_ok());
    }
}
