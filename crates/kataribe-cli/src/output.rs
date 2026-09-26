//! 標準出力・標準エラー出力への書き込み先。
//!
//! 生成中のイベントは `EventSink::emit(&self, ..)` から呼ばれるため、
//! 書き込み先は `&self` だけで書けて `Send + Sync` でなければならない。
//! 本番では実際の標準出力・標準エラー出力に、テストでは共有バッファに書く。

use std::io::Write as _;
use std::sync::Mutex;

use kataribe_engine::{GenerationEvent, NoticeLevel};

/// 標準出力・標準エラー出力への書き込み先。
///
/// `print` は生成中の本文の断片など、そのまま利用者に見せる出力に使う。
/// `eprint` は進捗・注意・エラーに使う。どちらも改行の有無は呼び出し側が決める。
pub trait Console: Send + Sync {
    fn print(&self, text: &str);
    fn eprint(&self, text: &str);
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
    fn print(&self, text: &str) {
        if let Ok(mut out) = self.stdout.lock() {
            let _ = out.write_all(text.as_bytes());
        }
    }

    fn eprint(&self, text: &str) {
        if let Ok(mut out) = self.stderr.lock() {
            let _ = out.write_all(text.as_bytes());
        }
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
    fn print(&self, text: &str) {
        if let Ok(mut out) = self.stdout.lock() {
            out.extend_from_slice(text.as_bytes());
        }
    }

    fn eprint(&self, text: &str) {
        if let Ok(mut out) = self.stderr.lock() {
            out.extend_from_slice(text.as_bytes());
        }
    }
}

/// 生成中のイベントを `console` へ振り分ける。
///
/// - `Content`（本文の断片）: 標準出力へそのまま。`quiet` なら出さない。
/// - `StepStarted` / `StepFinished` / `Notice`: 標準エラー出力へ 1 行で。
/// - `Reasoning`（推論モデルの思考）: 既定では出さない。`verbose` なら薄く標準エラー出力へ。
pub fn report_generation_event(
    console: &dyn Console,
    quiet: bool,
    verbose: bool,
    event: &GenerationEvent,
) {
    match event {
        GenerationEvent::Content { text } => {
            if !quiet {
                console.print(text);
            }
        }
        GenerationEvent::StepStarted {
            label,
            index,
            total,
        } => {
            console.eprint(&format!("  ▶ [{index}/{total}] {label}\n"));
        }
        GenerationEvent::StepFinished {
            prompt_tokens,
            completion_tokens,
            elapsed_ms,
        } => {
            console.eprint(&format!(
                "    完了（{}）\n",
                describe_step_finish(*prompt_tokens, *completion_tokens, *elapsed_ms)
            ));
        }
        GenerationEvent::Notice { level, message } => {
            let mark = match level {
                NoticeLevel::Info => "ℹ",
                NoticeLevel::Warning => "⚠",
            };
            console.eprint(&format!("  {mark} {message}\n"));
        }
        GenerationEvent::Reasoning { text } => {
            if verbose {
                console.eprint(&format!("    思考: {text}"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_console_captures_writes_separately() {
        let console = BufferConsole::new();
        console.print("本文");
        console.eprint("進捗");

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
}
