//! 本文の文字数・段落数・会話行数・原稿用紙換算枚数を数える。

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::ruby;

/// 400 字詰め原稿用紙は 1 行 20 字 × 20 行。
const MANUSCRIPT_COLUMNS_PER_ROW: usize = 20;
const MANUSCRIPT_ROWS_PER_PAGE: f64 = 20.0;

/// 本文の計数結果。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TextStats {
    /// 本文の文字数（ルビの読み・空白・改行を除き、書記素クラスタ単位で数える）。
    pub chars: usize,
    /// 空行でない行の数。
    pub paragraphs: usize,
    /// 「または『で始まる行の数。
    pub dialogue_lines: usize,
    /// 400 字詰め原稿用紙換算の枚数。
    pub manuscript_pages: f64,
}

/// 本文の文字数を数える。
///
/// ルビ記法は [`ruby::to_plain`] で読み仮名・記号を取り除いてから数え、
/// さらに空白（半角・全角スペース、タブ）と改行は数えない。
#[must_use]
pub fn count_chars(text: &str) -> usize {
    visible_char_count(&ruby::to_plain(text))
}

/// 段落数・会話行数・原稿用紙換算枚数を含む計数結果を返す。
///
/// 入力が空、または空白だけで構成されている場合はすべて 0 を返す。
#[must_use]
pub fn stats(text: &str) -> TextStats {
    let plain = ruby::to_plain(text);
    if plain.trim().is_empty() {
        return TextStats {
            chars: 0,
            paragraphs: 0,
            dialogue_lines: 0,
            manuscript_pages: 0.0,
        };
    }

    let lines: Vec<&str> = plain.lines().collect();
    let paragraphs = lines.iter().filter(|line| !is_blank_line(line)).count();
    let dialogue_lines = lines.iter().filter(|line| is_dialogue_line(line)).count();
    let total_rows: usize = lines.iter().map(|line| manuscript_rows(line)).sum();

    // 実際の原稿は多くても数万行程度で、f64 の仮数部(52 bit)に対して十分小さい。
    #[allow(clippy::cast_precision_loss)]
    let manuscript_pages = total_rows as f64 / MANUSCRIPT_ROWS_PER_PAGE;

    TextStats {
        chars: visible_char_count(&plain),
        paragraphs,
        dialogue_lines,
        manuscript_pages,
    }
}

/// 書記素クラスタ単位で数え、空白・改行を除いた文字数を返す。
fn visible_char_count(text: &str) -> usize {
    text.graphemes(true)
        .filter(|grapheme| !is_ignored_whitespace(grapheme))
        .count()
}

fn is_ignored_whitespace(grapheme: &str) -> bool {
    matches!(grapheme, " " | "\u{3000}" | "\t" | "\n" | "\r" | "\r\n")
}

/// 空行（空白だけの行を含む）かどうか。
fn is_blank_line(line: &str) -> bool {
    line.chars()
        .all(|c| c == ' ' || c == '\u{3000}' || c == '\t')
}

/// 字下げの全角空白を除いた先頭が「または『で始まるかどうか。
fn is_dialogue_line(line: &str) -> bool {
    let trimmed = line.trim_start_matches('\u{3000}');
    trimmed.starts_with('「') || trimmed.starts_with('『')
}

/// この行が原稿用紙で占める行数。空行は 1 行として数える。
fn manuscript_rows(line: &str) -> usize {
    let chars = visible_char_count(line);
    if chars == 0 {
        1
    } else {
        chars.div_ceil(MANUSCRIPT_COLUMNS_PER_ROW)
    }
}

#[cfg(test)]
// 原稿用紙換算枚数は「行数 / 20」という単純な割り算なので、テストでは
// 同じ式で得られる厳密な値どうしを比較しており、浮動小数の誤差は生じない。
#[allow(clippy::float_cmp)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn counts_graphemes_and_ignores_whitespace_and_newlines() {
        let text = "吾輩は 猫である。\n名前はまだ無い。";
        // 全角スペースと改行を除いた文字数（吾輩は=3, 猫である。=5, 名前はまだ無い。=8）。
        assert_eq!(count_chars(text), 16);
    }

    #[test]
    fn count_chars_expands_ruby_and_drops_reading() {
        let text = "|東京《とうきょう》";
        // 親文字「東京」の 2 文字だけを数え、読み仮名は数えない。
        assert_eq!(count_chars(text), 2);
    }

    #[test]
    fn empty_and_whitespace_only_input_returns_zero() {
        assert_eq!(count_chars(""), 0);
        assert_eq!(count_chars("   \n\u{3000}\t\n"), 0);

        let empty_stats = stats("   \n\n\t");
        assert_eq!(empty_stats.chars, 0);
        assert_eq!(empty_stats.paragraphs, 0);
        assert_eq!(empty_stats.dialogue_lines, 0);
        assert_eq!(empty_stats.manuscript_pages, 0.0);
    }

    #[test]
    fn stats_counts_paragraphs_and_dialogue_lines() {
        let text = "\u{3000}朝、目が覚めた。\n\n「おはよう」\n『やあ』と返した。\n地の文が続く。";
        let result = stats(text);
        assert_eq!(result.paragraphs, 4);
        assert_eq!(result.dialogue_lines, 2);
    }

    #[test]
    fn manuscript_pages_treats_blank_line_as_one_row() {
        // 20 字ちょうどの行 1 つ + 空行 1 つ = 2 行 → 400 字詰め用紙の 2/20 枚。
        let line = "あ".repeat(20);
        let text = format!("{line}\n\n");
        let result = stats(&text);
        assert_eq!(result.manuscript_pages, 2.0 / 20.0);
    }

    #[test]
    fn manuscript_pages_rounds_up_each_line_to_full_row() {
        // 21 字の行は 2 行分として数える。
        let line = "あ".repeat(21);
        let result = stats(&line);
        assert_eq!(result.manuscript_pages, 2.0 / 20.0);
    }
}
