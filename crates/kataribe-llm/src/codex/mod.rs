//! Codex CLI の `codex exec` を LLM として使う。
//!
//! API キーを使わず、利用者が Codex でログインしている権限のまま生成できる。呼び出しごとに空の作業フォルダで
//! `codex exec` を起動し、利用者の設定は読み込ませず、会話の履歴も残さない（小説の文章を書かせるだけで、
//! ファイルを読み書きさせないため）。システムプロンプトはファイルで、本文は標準入力で渡し、`--json` の出力を
//! [`ChatEvent`](crate::ChatEvent) に変換する。ストリームを drop すると `codex` のプロセスも止める（中止）。

mod auth;
mod events;
mod run;

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use crate::chat_model::{ChatModel, ChatStream};
use crate::cli_process::CliTool;
use crate::error::LlmError;
use crate::model_info::ModelInfo;
use crate::request::ChatRequest;

/// 本文の差分が流れず、応答は最後にまとめて届くので、書いている間は何も出力されない。
/// 2,500 字で 2 分かかった（gpt-6.1-sol・推論 high）ので、長い章でも収まるように長くする。
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_mins(30);
const TOOL: CliTool = CliTool {
    command: "codex",
    install_hint: "Codex CLI をインストールしてログインするか、設定で codex コマンドの場所を指定してください（npm でインストールした場合は codex.cmd）。",
    wrap_error: LlmError::Codex,
};

/// [`CodexModel`] の設定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexConfig {
    /// `codex` コマンド（PATH に無ければ実行ファイルの場所）。
    pub program: String,
    /// モデル（`gpt-6.1-sol` など）。空なら Codex の既定。
    pub model: String,
    /// 推論の強さ（`low`・`medium`・`high` など）。空なら Codex の既定。
    pub reasoning_effort: String,
    /// 生成中、`codex` の次の出力を待つ上限。過ぎたらエラーにする。
    pub idle_timeout: Duration,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            program: "codex".to_owned(),
            model: String::new(),
            reasoning_effort: String::new(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

/// `codex exec` で生成する [`ChatModel`]。
#[derive(Debug, Clone)]
pub struct CodexModel {
    config: CodexConfig,
}

impl CodexModel {
    /// 設定から作る。`codex` を起動するのは生成するときで、ここでは何も確かめない。
    #[must_use]
    pub fn new(config: CodexConfig) -> Self {
        Self { config }
    }

    /// 選べるモデル。接続テストを兼ねて、`codex` コマンドが動くことと、Codex にログインしていることも
    /// 確かめる（利用枠は使わない）。
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        auth::ensure_logged_in(&self.config.program).await?;
        auth::available_models(&self.config.program).await
    }
}

impl ChatModel for CodexModel {
    fn describe(&self) -> String {
        if self.config.model.is_empty() {
            "Codex（既定のモデル）".to_owned()
        } else {
            format!("Codex（{}）", self.config.model)
        }
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream {
        run::stream_chat(self.config.clone(), request)
    }
}

/// `codex exec` に渡す、作業の場所とファイル。
struct InvocationPaths<'a> {
    workdir: &'a Path,
    system_prompt: &'a Path,
    output_schema: Option<&'a Path>,
}

/// `codex` に渡す引数。利用者の設定と実行ポリシーを読み込ませず、セッションも残さず、ファイルも書かせない。
fn command_args(config: &CodexConfig, paths: &InvocationPaths<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "exec",
        "--json",
        "--ephemeral",
        "--ignore-user-config",
        "--ignore-rules",
        "--skip-git-repo-check",
        "--sandbox",
        "read-only",
        "--color",
        "never",
        "-C",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(paths.workdir.as_os_str().to_owned());
    args.push("-c".into());
    args.push(format!("model_instructions_file={}", toml_path(paths.system_prompt)).into());
    if !config.reasoning_effort.is_empty() {
        args.push("-c".into());
        args.push(format!("model_reasoning_effort=\"{}\"", config.reasoning_effort).into());
    }
    if !config.model.is_empty() {
        args.push("-m".into());
        args.push(config.model.clone().into());
    }
    if let Some(schema) = paths.output_schema {
        args.push("--output-schema".into());
        args.push(schema.as_os_str().to_owned());
    }
    args.push("-".into());
    args
}

/// `-c` の値は TOML として解釈されるので、`/` 区切りの文字列にする（`\` のままだとエスケープとして読まれる）。
fn toml_path(path: &Path) -> String {
    let path = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"");
    format!("\"{path}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn args_for(config: &CodexConfig, output_schema: Option<&Path>) -> Vec<String> {
        command_args(
            config,
            &InvocationPaths {
                workdir: Path::new("C:/tmp/kataribe-codex"),
                system_prompt: Path::new(r"C:\tmp\system prompt.txt"),
                output_schema,
            },
        )
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
    }

    fn value_after(args: &[String], flag: &str) -> String {
        args[args.iter().position(|arg| arg == flag).unwrap() + 1].clone()
    }

    #[test]
    fn arguments_ignore_user_settings_keep_no_history_and_read_the_prompt_from_stdin() {
        let args = args_for(&CodexConfig::default(), None);

        for expected in [
            "exec",
            "--json",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
        ] {
            assert!(args.contains(&expected.to_owned()), "{expected}: {args:?}");
        }
        assert_eq!(value_after(&args, "--sandbox"), "read-only");
        assert_eq!(value_after(&args, "-C"), "C:/tmp/kataribe-codex");
        assert_eq!(args.last().unwrap(), "-");
    }

    #[test]
    fn the_system_prompt_path_is_a_toml_string_with_forward_slashes() {
        let args = args_for(&CodexConfig::default(), None);

        assert_eq!(
            value_after(&args, "-c"),
            r#"model_instructions_file="C:/tmp/system prompt.txt""#
        );
    }

    #[test]
    fn model_and_reasoning_effort_are_passed_only_when_set() {
        let unset = args_for(&CodexConfig::default(), None);
        assert!(!unset.contains(&"-m".to_owned()));
        assert!(
            !unset
                .iter()
                .any(|arg| arg.starts_with("model_reasoning_effort"))
        );

        let config = CodexConfig {
            model: "gpt-6.1-sol".to_owned(),
            reasoning_effort: "high".to_owned(),
            ..CodexConfig::default()
        };
        let args = args_for(&config, None);
        assert_eq!(value_after(&args, "-m"), "gpt-6.1-sol");
        assert!(
            args.contains(&r#"model_reasoning_effort="high""#.to_owned()),
            "{args:?}"
        );
    }

    #[test]
    fn an_output_schema_is_passed_before_the_stdin_marker() {
        let args = args_for(
            &CodexConfig::default(),
            Some(Path::new("C:/tmp/schema.json")),
        );

        assert_eq!(value_after(&args, "--output-schema"), "C:/tmp/schema.json");
        assert_eq!(args.last().unwrap(), "-");
    }

    #[test]
    fn the_description_names_codex_and_the_model() {
        let named = CodexModel::new(CodexConfig {
            model: "gpt-6.1-sol".to_owned(),
            ..CodexConfig::default()
        });
        assert_eq!(named.describe(), "Codex（gpt-6.1-sol）");
        assert_eq!(
            CodexModel::new(CodexConfig::default()).describe(),
            "Codex（既定のモデル）"
        );
    }
}
