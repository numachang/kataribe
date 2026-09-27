//! プロンプトのテンプレート（`prompts/*.j2`）の描画。

use kataribe_llm::Message;
use minijinja::Environment;
use serde::Serialize;

use crate::error::{EngineError, Result};

/// テンプレートの種類。1 つのテンプレートは `system` と `user` の 2 つのブロックを持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptTemplate {
    Concept,
    Style,
    World,
    Cast,
    Character,
    Synopsis,
    Outline,
    ScenePlan,
    Beats,
    Draft,
    Digest,
    Revise,
    Polish,
}

impl PromptTemplate {
    const ALL: [PromptTemplate; 13] = [
        Self::Concept,
        Self::Style,
        Self::World,
        Self::Cast,
        Self::Character,
        Self::Synopsis,
        Self::Outline,
        Self::ScenePlan,
        Self::Beats,
        Self::Draft,
        Self::Digest,
        Self::Revise,
        Self::Polish,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Concept => "concept.j2",
            Self::Style => "style.j2",
            Self::World => "world.j2",
            Self::Cast => "cast.j2",
            Self::Character => "character.j2",
            Self::Synopsis => "synopsis.j2",
            Self::Outline => "outline.j2",
            Self::ScenePlan => "scene_plan.j2",
            Self::Beats => "beats.j2",
            Self::Draft => "draft.j2",
            Self::Digest => "digest.j2",
            Self::Revise => "revise.j2",
            Self::Polish => "polish.j2",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::Concept => include_str!("../prompts/concept.j2"),
            Self::Style => include_str!("../prompts/style.j2"),
            Self::World => include_str!("../prompts/world.j2"),
            Self::Cast => include_str!("../prompts/cast.j2"),
            Self::Character => include_str!("../prompts/character.j2"),
            Self::Synopsis => include_str!("../prompts/synopsis.j2"),
            Self::Outline => include_str!("../prompts/outline.j2"),
            Self::ScenePlan => include_str!("../prompts/scene_plan.j2"),
            Self::Beats => include_str!("../prompts/beats.j2"),
            Self::Draft => include_str!("../prompts/draft.j2"),
            Self::Digest => include_str!("../prompts/digest.j2"),
            Self::Revise => include_str!("../prompts/revise.j2"),
            Self::Polish => include_str!("../prompts/polish.j2"),
        }
    }
}

const MACROS_NAME: &str = "_macros.j2";
const MACROS_SOURCE: &str = include_str!("../prompts/_macros.j2");

/// 描画済みのプロンプト。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Prompt {
    pub system: String,
    pub user: String,
}

impl Prompt {
    pub fn to_messages(&self) -> Vec<Message> {
        vec![
            Message::system(self.system.clone()),
            Message::user(self.user.clone()),
        ]
    }

    pub fn estimated_tokens(&self) -> usize {
        // メッセージの区切りなど、チャットテンプレートが足すトークンの分を少し上乗せする
        const TEMPLATE_OVERHEAD: usize = 32;
        kataribe_text::tokens::estimate_tokens(&self.system)
            + kataribe_text::tokens::estimate_tokens(&self.user)
            + TEMPLATE_OVERHEAD
    }
}

/// 組み込みテンプレートの集まり。
#[derive(Debug)]
pub(crate) struct PromptLibrary {
    environment: Environment<'static>,
}

impl PromptLibrary {
    pub fn builtin() -> Result<Self> {
        let mut environment = Environment::new();
        environment.set_trim_blocks(true);
        environment.set_lstrip_blocks(true);
        environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        environment.add_template(MACROS_NAME, MACROS_SOURCE)?;
        for template in PromptTemplate::ALL {
            environment.add_template(template.name(), template.source())?;
        }
        Ok(Self { environment })
    }

    pub fn render(&self, template: PromptTemplate, context: &impl Serialize) -> Result<Prompt> {
        let template = self.environment.get_template(template.name())?;
        let mut captured = template.render_captured(context)?;
        let (system, user) = captured.with_state_mut(|state| {
            Ok::<_, minijinja::Error>((state.render_block("system")?, state.render_block("user")?))
        })?;
        Ok(Prompt {
            system: tidy(&system),
            user: tidy(&user),
        })
    }
}

impl From<minijinja::Error> for EngineError {
    fn from(error: minijinja::Error) -> Self {
        Self::Prompt(format!("{error:#}"))
    }
}

/// テンプレートの条件分岐で生じた余分な空行を詰め、前後の空白を除く。
fn tidy(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.trim().lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        result.push_str(line);
        result.push('\n');
    }
    result.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn tidy_collapses_blank_runs_and_trims() {
        assert_eq!(tidy("\n\nA\n\n\n\nB  \n\n"), "A\n\nB");
    }

    #[test]
    fn every_builtin_template_compiles() {
        PromptLibrary::builtin().unwrap();
    }
}
