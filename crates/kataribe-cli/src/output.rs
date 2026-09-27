//! 標準出力・標準エラー出力への書き込み先。
//!
//! 生成中のイベントは `EventSink::emit(&self, ..)` から呼ばれるため、
//! 書き込み先は `&self` だけで書けて `Send + Sync` でなければならない。
//! 本番では実際の標準出力・標準エラー出力に、テストでは共有バッファに書く。

use std::io::Write as _;
use std::sync::{Mutex, PoisonError};

use kataribe_engine::{GenerationEvent, NoticeLevel};

/// 標準出力・標準エラー出力への書き込み先。
///
/// `print` は生成中の本文の断片や、コマンドの結果として利用者に見せる出力に使う。
/// `eprint` は進捗・注意・エラーに使う。どちらも改行の有無は呼び出し側が決める。
/// 書き込みのたびに flush し、失敗（ディスクが一杯など）は `Err` で呼び出し側に返す。
pub trait Console: Send + Sync {
    fn print(&self, text: &str) -> std::io::Result<()>;
    fn eprint(&self, text: &str) -> std::io::Result<()>;
}

/// 実際の標準出力・標準エラー出力に書き込む実装。
#[derive(Debug)]
pub struct StdConsole {
    stdout: Mutex<std::io::Stdout>,
    stderr: Mutex<std::io::Stderr>,
}

impl StdConsole {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stdout: Mutex::new(std::io::stdout()),
            stderr: Mutex::new(std::io::stderr()),
        }
    }
}

impl Default for StdConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl Console for StdConsole {
    fn print(&self, text: &str) -> std::io::Result<()> {
        let mut out = self.stdout.lock().unwrap_or_else(PoisonError::into_inner);
        out.write_all(text.as_bytes())?;
        out.flush()
    }

    fn eprint(&self, text: &str) -> std::io::Result<()> {
        let mut out = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        out.write_all(text.as_bytes())?;
        out.flush()
    }
}

/// テスト用に、標準出力・標準エラー出力の内容をメモリに溜める実装。
#[derive(Debug, Default, Clone)]
pub struct BufferConsole {
    stdout: std::sync::Arc<Mutex<Vec<u8>>>,
    stderr: std::sync::Arc<Mutex<Vec<u8>>>,
}

impl BufferConsole {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// これまでに標準出力へ書き込まれた内容（UTF-8 として解釈できる前提）。
    #[must_use]
    pub fn stdout(&self) -> String {
        buffer_to_string(&self.stdout)
    }

    /// これまでに標準エラー出力へ書き込まれた内容。
    #[must_use]
    pub fn stderr(&self) -> String {
        buffer_to_string(&self.stderr)
    }
}

fn buffer_to_string(buffer: &Mutex<Vec<u8>>) -> String {
    let bytes = buffer.lock().map(|guard| guard.clone()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

impl Console for BufferConsole {
    fn print(&self, text: &str) -> std::io::Result<()> {
        let mut out = self.stdout.lock().unwrap_or_else(PoisonError::into_inner);
        out.extend_from_slice(text.as_bytes());
        Ok(())
    }

    fn eprint(&self, text: &str) -> std::io::Result<()> {
        let mut out = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        out.extend_from_slice(text.as_bytes());
        Ok(())
    }
}

/// 生成中のイベントを `console` へ振り分ける。
///
/// - `StepStarted`: 標準エラー出力へ進捗を 1 行。2 つ目以降の工程では、直前の工程の本文と
///   混ざらないよう標準出力側にも区切りの空行を入れる（`quiet` なら本文自体を出さないので不要）。
/// - `Content`（本文の断片）: 標準出力へそのまま。`quiet` なら出さない。
/// - `StepFinished` / `Notice`: 標準エラー出力へ 1 行で。
/// - `Reasoning`（推論モデルの思考）: 既定では出さない。`verbose` なら薄く標準エラー出力へ。
///
/// `kataribe_engine::EventSink` は戻り値を持たないため、書き込みに失敗しても生成そのものは
/// 続ける（表示できないことより、生成した内容を最後まで得られないことのほうが損失が大きい）。
/// そのため、ここでの書き込み失敗は最善努力として無視する。
pub fn report_generation_event(
    console: &dyn Console,
    quiet: bool,
    verbose: bool,
    event: &GenerationEvent,
) {
    match event {
        GenerationEvent::Content { text } => {
            if !quiet {
                let _ = console.print(text);
            }
        }
        GenerationEvent::StepStarted {
            label,
            index,
            total,
        } => {
            if !quiet && *index > 1 {
                let _ = console.print("\n");
            }
            let _ = console.eprint(&format!("  ▶ [{index}/{total}] {label}\n"));
        }
        GenerationEvent::StepFinished {
            prompt_tokens,
            completion_tokens,
            elapsed_ms,
        } => {
            let _ = console.eprint(&format!(
                "    完了（{}）\n",
                describe_step_finish(*prompt_tokens, *completion_tokens, *elapsed_ms)
            ));
        }
        GenerationEvent::Notice { level, message } => {
            let mark = match level {
                NoticeLevel::Info => "ℹ",
                NoticeLevel::Warning => "⚠",
            };
            let _ = console.eprint(&format!("  {mark} {message}\n"));
        }
        GenerationEvent::Reasoning { text } => {
            if verbose {
                let _ = console.eprint(&format!("    思考: {text}"));
            }
        }
    }
}

/// LLM 呼び出し 1 回の所要時間はせいぜい数時間程度で、f64 の仮数部（52 bit）に対して
/// 十分小さいミリ秒値なので、秒への変換に精度の実害は無い。
#[allow(clippy::cast_precision_loss)]
fn describe_step_finish(
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    elapsed_ms: u64,
) -> String {
    let seconds = elapsed_ms as f64 / 1000.0;
    match (prompt_tokens, completion_tokens) {
        (Some(prompt), Some(completion)) => {
            format!("{seconds:.1} 秒、入力 {prompt} トークン・出力 {completion} トークン")
        }
        _ => format!("{seconds:.1} 秒"),
    }
}

/// テスト用の道具。同じクレート内の他のモジュールのテストからも使う
/// （書き込み失敗時の挙動を確かめるのに、本物の標準出力を壊す必要がないように）。
#[cfg(test)]
pub(crate) mod testing {
    use super::Console;

    /// 常に書き込みが失敗する [`Console`]。
    #[derive(Debug, Default)]
    pub(crate) struct FailingConsole;

    impl Console for FailingConsole {
        fn print(&self, _text: &str) -> std::io::Result<()> {
            Err(std::io::Error::other("書き込みに失敗しました（テスト用）"))
        }

        fn eprint(&self, _text: &str) -> std::io::Result<()> {
            Err(std::io::Error::other("書き込みに失敗しました（テスト用）"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_console_captures_writes_separately() {
        let console = BufferConsole::new();
        console.print("本文").unwrap();
        console.eprint("進捗").unwrap();

        assert_eq!(console.stdout(), "本文");
        assert_eq!(console.stderr(), "進捗");
    }

    #[test]
    fn quiet_suppresses_content_but_not_progress() {
        let console = BufferConsole::new();
        report_generation_event(
            &console,
            true,
            false,
            &GenerationEvent::Content {
                text: "本文の断片".into(),
            },
        );
        report_generation_event(
            &console,
            true,
            false,
            &GenerationEvent::StepStarted {
                label: "企画を生成".into(),
                index: 1,
                total: 1,
            },
        );

        assert_eq!(console.stdout(), "");
        assert!(console.stderr().contains("企画を生成"));
    }

    #[test]
    fn reasoning_is_hidden_unless_verbose() {
        let console = BufferConsole::new();
        let event = GenerationEvent::Reasoning {
            text: "考え中".into(),
        };

        report_generation_event(&console, false, false, &event);
        assert_eq!(console.stderr(), "");

        report_generation_event(&console, false, true, &event);
        assert!(console.stderr().contains("考え中"));
    }

    #[test]
    fn second_step_started_separates_stdout_from_the_previous_step_with_a_blank_line() {
        let console = BufferConsole::new();
        report_generation_event(
            &console,
            false,
            false,
            &GenerationEvent::Content {
                text: "第一段落。".into(),
            },
        );
        report_generation_event(
            &console,
            false,
            false,
            &GenerationEvent::StepStarted {
                label: "続きを生成".into(),
                index: 2,
                total: 2,
            },
        );
        report_generation_event(
            &console,
            false,
            false,
            &GenerationEvent::Content {
                text: "第二段落。".into(),
            },
        );

        assert_eq!(console.stdout(), "第一段落。\n第二段落。");
    }

    #[test]
    fn first_step_started_does_not_add_a_leading_blank_line() {
        let console = BufferConsole::new();
        report_generation_event(
            &console,
            false,
            false,
            &GenerationEvent::StepStarted {
                label: "企画を生成".into(),
                index: 1,
                total: 1,
            },
        );

        assert_eq!(console.stdout(), "");
    }
}
