//! 1 回の生成で `claude -p` を動かし、その出力を [`ChatEvent`] のストリームにする。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use futures_util::stream;
use tempfile::TempPath;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout};
use tokio::task::JoinHandle;

use super::events::{LineEvent, ResultLine, parse_line};
use super::{
    ClaudeCodeConfig, SplitPrompt, claude_command, command_args, local_error, spawn_error,
};
use crate::chat_model::ChatStream;
use crate::error::LlmError;
use crate::event::{ChatEvent, Finish};
use crate::request::{ChatRequest, ResponseFormat};

/// `claude` を動かす空の作業フォルダ（一時フォルダの下）。動いている間は消せない（Windows）ため、
/// 呼び出しごとに作って消すのではなく、同じフォルダを使い回す。
const WORKDIR_NAME: &str = "kataribe-claude";
/// 結果の行を受け取ったあと、`claude` が自分で終わるのを待つ上限。過ぎたら止める。
const EXIT_GRACE: Duration = Duration::from_secs(10);

/// `request` を `claude -p` で生成するストリーム。初めてポーリングされたときに起動する。
pub(super) fn stream_chat(config: ClaudeCodeConfig, request: ChatRequest) -> ChatStream {
    let start = Stage::NotStarted(Box::new(Pending { config, request }));
    Box::pin(stream::unfold(start, advance))
}

/// ストリームの進み具合。
enum Stage {
    NotStarted(Box<Pending>),
    Running(Box<Running>),
    Done,
}

/// まだ起動していない生成の依頼。
struct Pending {
    config: ClaudeCodeConfig,
    request: ChatRequest,
}

/// 動いている `claude` のプロセスと、その出力を読む状態。
///
/// 途中で drop する（中止）と、プロセスも止める（`kill_on_drop`）。`claude.cmd`（npm 版）のように
/// バッチファイルを指定したときは、止まるのは間の `cmd.exe` で、その下の `claude` は、出力先が
/// 閉じたことで数秒のうちに自分で終わる（実測 6 秒以内）。
struct Running {
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
    stderr: JoinHandle<String>,
    /// JSON Schema を指定したか。このときは前置きの文章を捨て、最後の JSON だけを本文にする。
    wants_json: bool,
    /// 差分として渡した本文。最後の結果の全文と突き合わせる。
    streamed: String,
    /// 1 行から 2 つ以上の出来事が出たときの、まだ渡していない分。
    pending: VecDeque<ChatEvent>,
    finished: bool,
    idle_timeout: Duration,
    /// システムプロンプトのファイル。drop すると消える。
    _system_prompt: TempPath,
}

async fn advance(stage: Stage) -> Option<(Result<ChatEvent, LlmError>, Stage)> {
    match stage {
        Stage::Done => None,
        Stage::NotStarted(pending) => match start(pending.config, &pending.request).await {
            Ok(running) => next_event(running).await,
            Err(error) => Some((Err(error), Stage::Done)),
        },
        Stage::Running(running) => next_event(*running).await,
    }
}

async fn start(config: ClaudeCodeConfig, request: &ChatRequest) -> Result<Running, LlmError> {
    let workdir = prepare_workdir().await?;
    let prompt = SplitPrompt::from_messages(&request.messages);
    let system_prompt = write_system_prompt(&prompt.system).await?;
    let schema = request
        .response_format
        .as_ref()
        .map(|ResponseFormat::JsonSchema { schema, .. }| super::schema_argument(schema));

    let mut command = claude_command(&config.program);
    command
        .args(command_args(
            &config.model,
            &system_prompt,
            schema.as_deref(),
        ))
        .current_dir(&workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| spawn_error(&config.program, &error))?;

    let (Some(mut stdin), Some(stdout), Some(mut stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err(LlmError::ClaudeCode(
            "claude コマンドの入出力をつなげませんでした。".to_owned(),
        ));
    };
    // 書き込みの失敗（claude が入力を読まずに終わったなど）は無視する。理由は出力の側に出る
    let user_prompt = prompt.user;
    tokio::spawn(async move {
        let _ = stdin.write_all(user_prompt.as_bytes()).await;
    });
    let stderr = tokio::spawn(async move {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text).await;
        text
    });
    Ok(Running {
        child,
        lines: BufReader::new(stdout).lines(),
        stderr,
        wants_json: schema.is_some(),
        streamed: String::new(),
        pending: VecDeque::new(),
        finished: false,
        idle_timeout: config.idle_timeout,
        _system_prompt: system_prompt,
    })
}

async fn prepare_workdir() -> Result<PathBuf, LlmError> {
    let workdir = std::env::temp_dir().join(WORKDIR_NAME);
    tokio::fs::create_dir_all(&workdir)
        .await
        .map_err(|error| local_error("作業用の一時フォルダを作れません", &error))?;
    Ok(workdir)
}

async fn write_system_prompt(system: &str) -> Result<TempPath, LlmError> {
    let path = tempfile::Builder::new()
        .prefix("kataribe-system-prompt-")
        .suffix(".txt")
        .tempfile()
        .map_err(|error| local_error("システムプロンプトのファイルを作れません", &error))?
        .into_temp_path();
    tokio::fs::write(&path, system)
        .await
        .map_err(|error| local_error("システムプロンプトを書き出せません", &error))?;
    Ok(path)
}

async fn next_event(mut running: Running) -> Option<(Result<ChatEvent, LlmError>, Stage)> {
    loop {
        if let Some(event) = running.pending.pop_front() {
            if running.finished && running.pending.is_empty() {
                running.exit_in_background();
                return Some((Ok(event), Stage::Done));
            }
            return Some((Ok(event), Stage::Running(Box::new(running))));
        }
        let line = match running.read_line().await {
            Ok(Some(line)) => line,
            Ok(None) => return Some((Err(running.ended_without_result().await), Stage::Done)),
            Err(error) => return Some((Err(error), Stage::Done)),
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Err(error) = parse_line(&line).and_then(|event| running.accept(event)) {
            return Some((Err(error), Stage::Done));
        }
    }
}

impl Running {
    /// 次の 1 行。出力が終わったら `None`。`idle_timeout` の間なにも出なければエラー。
    async fn read_line(&mut self) -> Result<Option<String>, LlmError> {
        match tokio::time::timeout(self.idle_timeout, self.lines.next_line()).await {
            Ok(result) => {
                result.map_err(|error| local_error("claude コマンドの出力を読めません", &error))
            }
            Err(_) => Err(LlmError::ClaudeCode(format!(
                "claude コマンドから {} 秒間、出力がありません。",
                self.idle_timeout.as_secs()
            ))),
        }
    }

    /// 1 行の出来事を、渡す [`ChatEvent`] に変えて積む。
    fn accept(&mut self, event: LineEvent) -> Result<(), LlmError> {
        match event {
            LineEvent::Text(text) if !self.wants_json => {
                self.streamed.push_str(&text);
                self.pending.push_back(ChatEvent::Content(text));
            }
            LineEvent::Thinking(thinking) => {
                self.pending.push_back(ChatEvent::Reasoning(thinking));
            }
            LineEvent::Result(result) => self.finish(result)?,
            LineEvent::Text(_) | LineEvent::Ignored => {}
        }
        Ok(())
    }

    fn finish(&mut self, result: ResultLine) -> Result<(), LlmError> {
        if result.is_error {
            return Err(LlmError::ClaudeCode(result.text));
        }
        if let Some(content) = self.content_not_yet_sent(result.text)? {
            self.pending.push_back(ChatEvent::Content(content));
        }
        self.pending.push_back(ChatEvent::Finished(Finish {
            reason: result.reason,
            usage: result.usage,
        }));
        self.finished = true;
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

    /// 結果を受け取ったあと、`claude` が自分で終わるのを裏で待ってから後始末する（利用者は待たせない）。
    /// 終わらなければ止める。待っている間にランタイムが終わる（CLI の終了など）ときは、待たずに止める。
    fn exit_in_background(mut self) {
        tokio::spawn(async move {
            if tokio::time::timeout(EXIT_GRACE, self.child.wait())
                .await
                .is_err()
            {
                let _ = self.child.kill().await;
            }
        });
    }

    /// 結果の行を出さずに出力が終わったときのエラー（エラー出力と終了コードを添える）。
    async fn ended_without_result(mut self) -> LlmError {
        let status = self.child.wait().await.ok();
        let stderr = self.stderr.await.unwrap_or_default();
        let detail = stderr.trim();
        let code = status
            .and_then(|status| status.code())
            .map_or_else(|| "不明".to_owned(), |code| code.to_string());
        LlmError::ClaudeCode(if detail.is_empty() {
            format!("結果を返さずに終了しました（終了コード {code}）。")
        } else {
            format!("結果を返さずに終了しました（終了コード {code}）: {detail}")
        })
    }
}
