//! 1 回の生成で `claude -p` を動かし、その出力を [`ChatEvent`] のストリームにする。

use std::collections::VecDeque;

use super::events::{LineEvent, ResultLine, parse_line};
use super::{ClaudeCodeConfig, TOOL, command_args, schema_argument};
use crate::chat_model::ChatStream;
use crate::cli_process::{
    CliProcess, OutputDecoder, SplitPrompt, command_for, output_stream, prepare_workdir,
    write_temporary_file,
};
use crate::error::LlmError;
use crate::event::{ChatEvent, Finish};
use crate::request::{ChatRequest, ResponseFormat};

/// `claude` を動かす空の作業フォルダ（一時フォルダの下）。
const WORKDIR_NAME: &str = "kataribe-claude";

/// `request` を `claude -p` で生成するストリーム。初めてポーリングされたときに起動する。
pub(super) fn stream_chat(config: ClaudeCodeConfig, request: ChatRequest) -> ChatStream {
    output_stream(Box::pin(start(config, request)))
}

async fn start(
    config: ClaudeCodeConfig,
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
    let schema = request
        .response_format
        .as_ref()
        .map(|ResponseFormat::JsonSchema { schema, .. }| schema_argument(schema));

    let mut command = command_for(&config.program);
    command
        .args(command_args(
            &config.model,
            &system_prompt,
            schema.as_deref(),
        ))
        .current_dir(&workdir);
    let process = CliProcess::spawn(
        TOOL,
        &config.program,
        command,
        prompt.user,
        config.idle_timeout,
        vec![system_prompt],
    )?;
    Ok((
        process,
        Decoder {
            wants_json: schema.is_some(),
            streamed: String::new(),
        },
    ))
}

/// `claude -p --output-format stream-json` の読み方。
struct Decoder {
    /// JSON Schema を指定したか。このときは前置きの文章を捨て、最後の JSON だけを本文にする。
    wants_json: bool,
    /// 差分として渡した本文。最後の結果の全文と突き合わせる。
    streamed: String,
}

impl OutputDecoder for Decoder {
    fn decode_line(
        &mut self,
        line: &str,
        events: &mut VecDeque<ChatEvent>,
    ) -> Result<bool, LlmError> {
        match parse_line(line)? {
            LineEvent::Text(text) if !self.wants_json => {
                self.streamed.push_str(&text);
                events.push_back(ChatEvent::Content(text));
            }
            LineEvent::Thinking(thinking) => events.push_back(ChatEvent::Reasoning(thinking)),
            LineEvent::Result(result) => {
                self.finish(result, events)?;
                return Ok(true);
            }
            LineEvent::Text(_) | LineEvent::Ignored => {}
        }
        Ok(false)
    }
}

impl Decoder {
    fn finish(&self, result: ResultLine, events: &mut VecDeque<ChatEvent>) -> Result<(), LlmError> {
        if result.is_error {
            return Err(LlmError::ClaudeCode(result.text));
        }
        if let Some(content) = self.content_not_yet_sent(result.text)? {
            events.push_back(ChatEvent::Content(content));
        }
        events.push_back(ChatEvent::Finished(Finish {
            reason: result.reason,
            usage: result.usage,
        }));
        Ok(())
    }

    /// 結果の全文のうち、まだ本文として渡していないもの。
    ///
    /// JSON Schema を指定したときは JSON を、差分が 1 つも届かなかったときは全文を渡す。
    /// 差分をつなげたものが全文と食い違うとき（Claude Code が途中でやり直したなど）は、
    /// 重複や欠けのある本文を原稿に入れないようエラーにする。
    fn content_not_yet_sent(&self, full_text: String) -> Result<Option<String>, LlmError> {
        if self.wants_json || self.streamed.is_empty() {
            return Ok(Some(full_text).filter(|text| !text.is_empty()));
        }
        if self.streamed == full_text {
            return Ok(None);
        }
        Err(LlmError::ClaudeCode(
            "途中で出力されたものと最終結果が一致しないため、本文を確定できませんでした。もう一度生成してください。"
                .to_owned(),
        ))
    }
}
