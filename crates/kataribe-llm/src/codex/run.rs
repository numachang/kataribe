//! 1 回の生成で `codex exec` を動かし、その出力を [`ChatEvent`] のストリームにする。

use std::collections::VecDeque;

use tempfile::TempPath;

use super::events::{Failure, LineEvent, parse_line};
use super::{CodexConfig, InvocationPaths, TOOL, command_args};
use crate::chat_model::ChatStream;
use crate::cli_process::{
    CliProcess, OutputDecoder, SplitPrompt, command_for, output_stream, prepare_workdir,
    write_temporary_file,
};
use crate::error::LlmError;
use crate::event::{ChatEvent, Finish, FinishReason, Usage};
use crate::request::{ChatRequest, ResponseFormat};

/// `codex` を動かす空の作業フォルダ（一時フォルダの下）。
const WORKDIR_NAME: &str = "kataribe-codex";

/// `request` を `codex exec` で生成するストリーム。初めてポーリングされたときに起動する。
pub(super) fn stream_chat(config: CodexConfig, request: ChatRequest) -> ChatStream {
    output_stream(Box::pin(start(config, request)))
}

async fn start(
    config: CodexConfig,
    request: ChatRequest,
) -> Result<(CliProcess, Decoder), LlmError> {
    let workdir = prepare_workdir(TOOL, WORKDIR_NAME).await?;
    let prompt = SplitPrompt::from_messages(&request.messages);
    let system_prompt = write_temporary_file(
        TOOL,
        "システムプロンプト",
        "kataribe-system-prompt-",
        ".txt",
        &prompt.system,
    )
    .await?;
    let output_schema = write_output_schema(&request).await?;

    let mut command = command_for(&config.program);
    command.args(command_args(
        &config,
        &InvocationPaths {
            workdir: &workdir,
            system_prompt: &system_prompt,
            output_schema: output_schema.as_deref(),
        },
    ));
    let temporary_files = std::iter::once(system_prompt)
        .chain(output_schema)
        .collect();
    let process = CliProcess::spawn(
        TOOL,
        &config.program,
        command,
        prompt.user,
        config.idle_timeout,
        temporary_files,
    )?;
    Ok((process, Decoder::default()))
}

async fn write_output_schema(request: &ChatRequest) -> Result<Option<TempPath>, LlmError> {
    let Some(ResponseFormat::JsonSchema { schema, .. }) = &request.response_format else {
        return Ok(None);
    };
    let path = write_temporary_file(
        TOOL,
        "JSON Schema",
        "kataribe-output-schema-",
        ".json",
        &schema.to_string(),
    )
    .await?;
    Ok(Some(path))
}

/// `codex exec --json` の読み方。
#[derive(Default)]
struct Decoder {
    /// 届いた発言のうち最後のもの。途中の発言（ツールを試す前の「確認します」など）は本文に混ぜない。
    last_message: Option<String>,
    /// 報告された失敗のうち最後のもの。
    last_failure: Option<Failure>,
}

impl OutputDecoder for Decoder {
    fn decode_line(
        &mut self,
        line: &str,
        events: &mut VecDeque<ChatEvent>,
    ) -> Result<bool, LlmError> {
        match parse_line(line)? {
            LineEvent::AgentMessage(text) => self.last_message = Some(text),
            LineEvent::Reasoning(text) => events.push_back(ChatEvent::Reasoning(text)),
            LineEvent::Error(failure) => self.last_failure = Some(failure),
            LineEvent::TurnFailed(failure) => return Err(failed(failure)),
            LineEvent::TurnCompleted(usage) => {
                self.finish(usage, events)?;
                return Ok(true);
            }
            LineEvent::Ignored => {}
        }
        Ok(false)
    }

    fn reported_error(&self) -> Option<String> {
        self.last_failure
            .as_ref()
            .map(|failure| failure.message.clone())
    }
}

impl Decoder {
    /// 応答はここまで届かないので、最後の発言の全文を 1 回で本文として渡す。
    fn finish(
        &mut self,
        usage: Option<Usage>,
        events: &mut VecDeque<ChatEvent>,
    ) -> Result<(), LlmError> {
        let Some(text) = self.last_message.take().filter(|text| !text.is_empty()) else {
            return Err(match self.last_failure.take() {
                Some(failure) => failed(failure),
                None => LlmError::Codex("Codex が本文を返しませんでした。".to_owned()),
            });
        };
        events.push_back(ChatEvent::Content(text));
        events.push_back(ChatEvent::Finished(Finish {
            reason: FinishReason::Stop,
            usage,
        }));
        Ok(())
    }
}

/// 失敗の報告をエラーにする。JSON Schema を断られたときは、サーバーに断られたときと同じ形にして、
/// 執筆エンジンが指示文で JSON を求める方法に切り替えられるようにする。
fn failed(failure: Failure) -> LlmError {
    if failure.schema_rejected {
        LlmError::Status {
            status: 400,
            body: failure.message,
        }
    } else {
        LlmError::Codex(failure.message)
    }
}
