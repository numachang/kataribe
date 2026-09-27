//! Claude Code の `claude -p`（ヘッドレス実行）を LLM として使う。
//!
//! API キーを使わず、利用者が Claude Code でログインしている権限のまま Claude で生成できる。
//! 呼び出しごとに空の作業フォルダで `claude -p` を起動し、ツール・プロジェクトの設定・MCP サーバーは
//! 読み込ませない（小説の文章を書かせるだけで、ファイルを読み書きさせないため）。
//! システムプロンプトはファイルで、本文は標準入力で渡し、`--output-format stream-json` の出力を
//! [`ChatEvent`](crate::ChatEvent) に変換する。ストリームを drop すると `claude` のプロセスも止める（中止）。

mod auth;
mod events;
mod run;

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use crate::chat_model::{ChatModel, ChatStream};
use crate::error::LlmError;
use crate::message::{Message, Role};
use crate::model_info::ModelInfo;
use crate::request::ChatRequest;

/// Claude Code のモデルの別名（Claude Code が最新の版に読み替える）。
const MODEL_ALIASES: [&str; 3] = ["sonnet", "opus", "haiku"];
/// 考えている間も思考の差分が流れるので、これだけ何も出なければ固まっているとみなす。
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_mins(5);
/// GUI のアプリから起動したときに、コンソールの窓を出さない（Windows の `CREATE_NO_WINDOW`）。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// [`ClaudeCodeModel`] の設定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCodeConfig {
    /// `claude` コマンド（PATH に無ければ実行ファイルの場所）。
    pub program: String,
    /// モデル（`sonnet`・`opus`・`haiku` などの別名か、モデル ID）。
    pub model: String,
    /// 生成中、`claude` の次の出力を待つ上限。過ぎたらエラーにする。
    pub idle_timeout: Duration,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            program: "claude".to_owned(),
            model: MODEL_ALIASES[0].to_owned(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

/// `claude -p` で生成する [`ChatModel`]。
#[derive(Debug, Clone)]
pub struct ClaudeCodeModel {
    config: ClaudeCodeConfig,
}

impl ClaudeCodeModel {
    /// 設定から作る。`claude` を起動するのは生成するときで、ここでは何も確かめない。
    #[must_use]
    pub fn new(config: ClaudeCodeConfig) -> Self {
        Self { config }
    }

    /// 選べるモデル（Claude Code のモデルの別名）。接続テストを兼ねて、`claude` コマンドが
    /// 動くことと、Claude Code にログインしていることも確かめる（利用枠は使わない）。
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        auth::ensure_logged_in(&self.config.program).await?;
        Ok(alias_models())
    }
}

fn alias_models() -> Vec<ModelInfo> {
    MODEL_ALIASES
        .iter()
        .map(|alias| ModelInfo {
            id: (*alias).to_owned(),
            context_length: None,
        })
        .collect()
}

impl ChatModel for ClaudeCodeModel {
    fn describe(&self) -> String {
        format!("Claude Code（{}）", self.config.model)
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream {
        run::stream_chat(self.config.clone(), request)
    }
}

/// メッセージを、システムプロンプトと、標準入力で渡す本文に分けたもの。
#[derive(Debug, PartialEq, Eq)]
struct SplitPrompt {
    system: String,
    user: String,
}

impl SplitPrompt {
    /// アシスタントの発言（応答の書き出しの指定など）は `claude -p` に渡せないので使わない。
    /// 執筆エンジンが渡すのは、推論モデルの思考を止めるための空の思考ブロックだけで、Claude には要らない。
    fn from_messages(messages: &[Message]) -> Self {
        let join = |role: Role| {
            messages
                .iter()
                .filter(|message| message.role == role)
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        Self {
            system: join(Role::System),
            user: join(Role::User),
        }
    }
}

/// `--json-schema` に渡すスキーマ。Claude Code の検証器は draft 2020-12 のメタスキーマを知らず、
/// `$schema` でそれを名乗るスキーマ（schemars が既定で作る）を拒むため、`$schema` だけを外す。
fn schema_argument(schema: &serde_json::Value) -> String {
    let mut schema = schema.clone();
    if let Some(object) = schema.as_object_mut() {
        object.remove("$schema");
    }
    schema.to_string()
}

/// `claude` を起動する [`Command`]。待つのをやめたら（drop したら）プロセスも終わらせる。
fn claude_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// `claude` に渡す引数。ツール・設定・MCP サーバーを読み込ませず、セッションも残さない。
fn command_args(
    model: &str,
    system_prompt_path: &Path,
    json_schema: Option<&str>,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--no-session-persistence",
        "--tools",
        "",
        "--strict-mcp-config",
        "--setting-sources",
        "",
        "--model",
        model,
        "--system-prompt-file",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(system_prompt_path.as_os_str().to_owned());
    if let Some(schema) = json_schema {
        args.push("--json-schema".into());
        args.push(schema.into());
    }
    args
}

fn spawn_error(program: &str, error: &std::io::Error) -> LlmError {
    if error.kind() == std::io::ErrorKind::NotFound {
        LlmError::ClaudeCode(format!(
            "claude コマンド（{program}）が見つかりません。Claude Code をインストールしてログインするか、設定で claude コマンドの場所を指定してください（npm でインストールした場合は claude.cmd）。"
        ))
    } else {
        local_error(
            &format!("claude コマンド（{program}）を起動できません"),
            error,
        )
    }
}

fn local_error(what: &str, error: &std::io::Error) -> LlmError {
    LlmError::ClaudeCode(format!("{what}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn messages_are_split_into_system_prompt_and_input_without_the_prefill() {
        let messages = vec![
            Message::system("あなたは小説家です。"),
            Message::user("雨の場面を書いてください。"),
            Message::assistant("<think>\n\n</think>\n\n"),
        ];

        assert_eq!(
            SplitPrompt::from_messages(&messages),
            SplitPrompt {
                system: "あなたは小説家です。".to_owned(),
                user: "雨の場面を書いてください。".to_owned(),
            }
        );
    }

    #[test]
    fn arguments_disable_tools_settings_and_sessions() {
        let args = command_args("sonnet", Path::new("C:/tmp/system-prompt.txt"), None);
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        for expected in [
            "-p",
            "--no-session-persistence",
            "--strict-mcp-config",
            "C:/tmp/system-prompt.txt",
        ] {
            assert!(args.contains(&expected.to_owned()), "{expected}: {args:?}");
        }
        let position = |flag: &str| args.iter().position(|arg| arg == flag).unwrap();
        assert_eq!(args[position("--tools") + 1], "");
        assert_eq!(args[position("--setting-sources") + 1], "");
        assert_eq!(args[position("--model") + 1], "sonnet");
        assert!(!args.contains(&"--json-schema".to_owned()));
    }

    #[test]
    fn a_json_schema_is_passed_when_structured_output_is_requested() {
        let args = command_args("opus", Path::new("s.txt"), Some(r#"{"type":"object"}"#));

        let tail: Vec<String> = args[args.len() - 2..]
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(tail, vec!["--json-schema", r#"{"type":"object"}"#]);
    }

    #[test]
    fn the_schema_is_passed_without_its_meta_schema() {
        let schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": { "name": { "$ref": "#/$defs/Name" } },
            "$defs": { "Name": { "type": "string" } },
        });

        let argument: serde_json::Value = serde_json::from_str(&schema_argument(&schema)).unwrap();

        assert_eq!(argument.get("$schema"), None);
        assert_eq!(argument["$defs"], schema["$defs"]);
        assert_eq!(argument["properties"], schema["properties"]);
    }

    #[test]
    fn the_description_names_claude_code_and_the_model() {
        let model = ClaudeCodeModel::new(ClaudeCodeConfig {
            model: "haiku".to_owned(),
            ..ClaudeCodeConfig::default()
        });

        assert_eq!(model.describe(), "Claude Code（haiku）");
    }

    #[test]
    fn available_models_are_the_claude_code_aliases() {
        let ids: Vec<String> = alias_models().into_iter().map(|model| model.id).collect();
        assert_eq!(ids, vec!["sonnet", "opus", "haiku"]);
    }
}
