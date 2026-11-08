//! LLM の CLI（`claude -p`・`codex exec`）を子プロセスとして動かし、出力を [`ChatEvent`] のストリームにする。
//!
//! 起動のしかた・入出力のつなぎ方・待ち時間・後始末はどの CLI でも同じなので、ここにまとめる。
//! 出力の 1 行をどう読むかだけが CLI ごとに違い、[`OutputDecoder`] で差し替える。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use futures_util::future::BoxFuture;
use futures_util::stream;
use tempfile::TempPath;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::task::JoinHandle;

use crate::chat_model::ChatStream;
use crate::error::{LlmError, truncate_body};
use crate::event::ChatEvent;
use crate::message::{Message, Role};

/// 結果を受け取ったあと、コマンドが自分で終わるのを待つ上限。過ぎたら止める。
const EXIT_GRACE: Duration = Duration::from_secs(10);
/// GUI のアプリから起動したときに、コンソールの窓を出さない（Windows の `CREATE_NO_WINDOW`）。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// CLI ごとに違う、エラーの種類と説明の文言。
#[derive(Debug, Clone, Copy)]
pub(crate) struct CliTool {
    /// エラーの説明に出すコマンドの名前（`claude` など）。
    pub command: &'static str,
    /// コマンドが見つからないときに添える、直し方の案内。
    pub install_hint: &'static str,
    /// 説明を、その CLI の [`LlmError`] にする。
    pub wrap_error: fn(String) -> LlmError,
}

impl CliTool {
    pub fn error(&self, message: impl Into<String>) -> LlmError {
        (self.wrap_error)(message.into())
    }

    pub fn local_error(&self, what: &str, error: &std::io::Error) -> LlmError {
        self.error(format!("{what}: {error}"))
    }

    /// コマンドを起動できなかったときのエラー。
    pub fn spawn_error(&self, program: &str, error: &std::io::Error) -> LlmError {
        if error.kind() == std::io::ErrorKind::NotFound {
            self.error(format!(
                "{} コマンド（{program}）が見つかりません。{}",
                self.command, self.install_hint
            ))
        } else {
            self.local_error(
                &format!("{} コマンド（{program}）を起動できません", self.command),
                error,
            )
        }
    }
}

/// `program` を起動する [`Command`]。待つのをやめたら（drop したら）プロセスも終わらせる。
pub(crate) fn command_for(program: &str) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// CLI を動かす空の作業フォルダ（一時フォルダの下の `folder_name`）。動いている間は消せない（Windows）ため、
/// 呼び出しごとに作って消すのではなく、同じフォルダを使い回す。
pub(crate) async fn prepare_workdir(tool: CliTool, folder_name: &str) -> Result<PathBuf, LlmError> {
    let workdir = std::env::temp_dir().join(folder_name);
    tokio::fs::create_dir_all(&workdir)
        .await
        .map_err(|error| tool.local_error("作業用の一時フォルダを作れません", &error))?;
    Ok(workdir)
}

/// `content` を書いた一時ファイル。drop すると消える。`what` はエラーの説明に使う。
pub(crate) async fn write_temporary_file(
    tool: CliTool,
    what: &str,
    prefix: &str,
    suffix: &str,
    content: &str,
) -> Result<TempPath, LlmError> {
    let path = tempfile::Builder::new()
        .prefix(prefix)
        .suffix(suffix)
        .tempfile()
        .map_err(|error| tool.local_error(&format!("{what}のファイルを作れません"), &error))?
        .into_temp_path();
    tokio::fs::write(&path, content)
        .await
        .map_err(|error| tool.local_error(&format!("{what}を書き出せません"), &error))?;
    Ok(path)
}

/// メッセージを、システムプロンプトと、標準入力で渡す本文に分けたもの。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SplitPrompt {
    pub system: String,
    pub user: String,
}

impl SplitPrompt {
    /// アシスタントの発言（応答の書き出しの指定など）は CLI に渡せないので使わない。
    /// 執筆エンジンが渡すのは、推論モデルの思考を止めるための空の思考ブロックだけで、CLI には要らない。
    pub fn from_messages(messages: &[Message]) -> Self {
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

/// 出力の読み方。CLI ごとに実装する。
pub(crate) trait OutputDecoder: Send + 'static {
    /// 出力の 1 行を解釈し、渡す出来事を `events` に積む。生成が終わった行なら `true` を返す。
    fn decode_line(
        &mut self,
        line: &str,
        events: &mut VecDeque<ChatEvent>,
    ) -> Result<bool, LlmError>;

    /// 終わりの行が来ないまま出力が途切れたときに、説明に使う、コマンド自身が報告したエラー。
    fn reported_error(&self) -> Option<String> {
        None
    }
}

/// 動いているコマンドと、その出力を読む状態。
///
/// 途中で drop する（中止）と、プロセスも、その下で動いているプロセスも止める。
pub(crate) struct CliProcess {
    tool: CliTool,
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
    stderr: JoinHandle<String>,
    idle_timeout: Duration,
    /// システムプロンプトなどの一時ファイル。drop すると消える。
    _temporary_files: Vec<TempPath>,
}

impl CliProcess {
    /// `command` を起動し、`input` を標準入力に書く。
    pub fn spawn(
        tool: CliTool,
        program: &str,
        mut command: Command,
        input: String,
        idle_timeout: Duration,
        temporary_files: Vec<TempPath>,
    ) -> Result<Self, LlmError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| tool.spawn_error(program, &error))?;
        let (Some(mut stdin), Some(stdout), Some(mut stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(tool.error(format!(
                "{} コマンドの入出力をつなげませんでした。",
                tool.command
            )));
        };
        // 書き込みの失敗（コマンドが入力を読まずに終わったなど）は無視する。理由は出力の側に出る
        tokio::spawn(async move {
            let _ = stdin.write_all(input.as_bytes()).await;
        });
        let stderr = tokio::spawn(async move {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text).await;
            text
        });
        Ok(Self {
            tool,
            child,
            lines: BufReader::new(stdout).lines(),
            stderr,
            idle_timeout,
            _temporary_files: temporary_files,
        })
    }

    /// 次の 1 行。出力が終わったら `None`。`idle_timeout` の間なにも出なければエラー。
    async fn read_line(&mut self) -> Result<Option<String>, LlmError> {
        match tokio::time::timeout(self.idle_timeout, self.lines.next_line()).await {
            Ok(result) => result.map_err(|error| {
                self.tool.local_error(
                    &format!("{} コマンドの出力を読めません", self.tool.command),
                    &error,
                )
            }),
            Err(_) => Err(self.tool.error(format!(
                "{} コマンドから {} 秒間、出力がありません。",
                self.tool.command,
                self.idle_timeout.as_secs()
            ))),
        }
    }

    /// 結果を受け取ったあと、コマンドが自分で終わるのを裏で待ってから後始末する（利用者は待たせない）。
    /// 終わらなければ、drop で止める。待っている間にランタイムが終わる（CLI の終了など）ときは、待たずに止める。
    fn exit_in_background(mut self) {
        tokio::spawn(async move {
            let _ = tokio::time::timeout(EXIT_GRACE, self.child.wait()).await;
        });
    }

    /// 終わりの行を出さずに出力が途切れたときのエラー。コマンド自身が報告したエラーがあればそれを、
    /// 無ければエラー出力を、終了コードと一緒に添える。
    async fn ended_without_result(mut self, reported: Option<String>) -> LlmError {
        let status = self.child.wait().await.ok();
        let stderr = (&mut self.stderr).await.unwrap_or_default();
        let detail = reported.unwrap_or(stderr);
        let detail = truncate_body(&detail);
        let code = status
            .and_then(|status| status.code())
            .map_or_else(|| "不明".to_owned(), |code| code.to_string());
        self.tool.error(if detail.is_empty() {
            format!("結果を返さずに終了しました（終了コード {code}）。")
        } else {
            format!("結果を返さずに終了しました（終了コード {code}）: {detail}")
        })
    }
}

impl Drop for CliProcess {
    /// まだ動いていれば、下のプロセスごと止める。`Child` の `kill_on_drop` は起動したプロセスしか止めず、
    /// `codex.cmd`（npm 版）では間の `cmd.exe` だけが止まって、`codex` は書き終えるまで利用枠を使い続ける
    /// （出力先が閉じても止まらないことを実測で確かめた）。
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(pid) = self.child.id() {
            stop_process_tree(pid);
        }
    }
}

/// `pid` とその下のプロセスを止める。`kill_on_drop` より先に行うため（親が先に消えると子をたどれない）、
/// 終わるまで待つ。短い時間なので、`drop` の中で待ってよい。失敗しても `kill_on_drop` が起動したプロセスは止める。
#[cfg(windows)]
fn stop_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt as _;

    let _ = std::process::Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// `start` で起動したコマンドの出力を、`D` で読むストリーム。初めてポーリングされたときに起動する。
pub(crate) fn output_stream<D: OutputDecoder>(
    start: BoxFuture<'static, Result<(CliProcess, D), LlmError>>,
) -> ChatStream {
    Box::pin(stream::unfold(Stage::NotStarted(start), advance))
}

/// ストリームの進み具合。
enum Stage<D> {
    NotStarted(BoxFuture<'static, Result<(CliProcess, D), LlmError>>),
    Running(Box<Running<D>>),
    Done,
}

struct Running<D> {
    process: CliProcess,
    decoder: D,
    /// 1 行から 2 つ以上の出来事が出たときの、まだ渡していない分。
    pending: VecDeque<ChatEvent>,
    finished: bool,
}

async fn advance<D: OutputDecoder>(
    stage: Stage<D>,
) -> Option<(Result<ChatEvent, LlmError>, Stage<D>)> {
    match stage {
        Stage::Done => None,
        Stage::NotStarted(start) => match start.await {
            Ok((process, decoder)) => {
                next_event(Running {
                    process,
                    decoder,
                    pending: VecDeque::new(),
                    finished: false,
                })
                .await
            }
            Err(error) => Some((Err(error), Stage::Done)),
        },
        Stage::Running(running) => next_event(*running).await,
    }
}

async fn next_event<D: OutputDecoder>(
    mut running: Running<D>,
) -> Option<(Result<ChatEvent, LlmError>, Stage<D>)> {
    loop {
        if let Some(event) = running.pending.pop_front() {
            if running.finished && running.pending.is_empty() {
                running.process.exit_in_background();
                return Some((Ok(event), Stage::Done));
            }
            return Some((Ok(event), Stage::Running(Box::new(running))));
        }
        let line = match running.process.read_line().await {
            Ok(Some(line)) => line,
            Ok(None) => {
                let reported = running.decoder.reported_error();
                let error = running.process.ended_without_result(reported).await;
                return Some((Err(error), Stage::Done));
            }
            Err(error) => return Some((Err(error), Stage::Done)),
        };
        if line.trim().is_empty() {
            continue;
        }
        match running.decoder.decode_line(&line, &mut running.pending) {
            Ok(finished) => running.finished = finished,
            Err(error) => return Some((Err(error), Stage::Done)),
        }
    }
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
}
