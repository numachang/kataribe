//! 表記の整形：段落頭の字下げ、省略記号・ダッシュ・感嘆符/疑問符の表記統一、
//! 空行の整理、行末空白の除去。

use serde::{Deserialize, Serialize};

/// 空行の扱い方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum BlankLinePolicy {
    /// そのまま残す。
    #[default]
    Keep,
    /// 連続する空行を 1 行にまとめる。
    Collapse,
    /// 空行をすべて取り除く。
    Remove,
}

/// [`normalize`] の挙動を切り替えるオプション。既定値はすべて有効、空行は保持する。
///
/// 4 つの独立した整形項目を切り替えるための構造体であり、状態機械やビットフラグに
/// 置き換えても可読性は上がらないため、bool が多いという clippy の指摘は抑制する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[allow(clippy::struct_excessive_bools)]
pub struct NormalizeOptions {
    /// 段落頭を全角空白 1 つで字下げする。
    pub paragraph_indent: bool,
    /// `...`・`。。。`・`・・・`・単独の `…` を `……` にそろえる。
    pub ellipsis: bool,
    /// `--`・`——`・単独の `―` を `――` にそろえる。
    pub dash: bool,
    /// 半角の `!`/`?` を全角にし、直後に応じて全角空白を補う。
    pub fullwidth_marks: bool,
    /// 空行の扱い方。
    pub blank_lines: BlankLinePolicy,
}

impl Default for NormalizeOptions {
    fn default() -> Self {
        Self {
            paragraph_indent: true,
            ellipsis: true,
            dash: true,
            fullwidth_marks: true,
            blank_lines: BlankLinePolicy::Keep,
        }
    }
}

/// 本文の表記を整える。
///
/// 各行を正規化した後、行末の空白を除き、ファイル末尾を改行 1 つで終える。
/// 入力が空、または空白だけで構成されている場合は空文字列を返す。
///
/// 2 回適用しても結果が変わらない（冪等）。
#[must_use]
pub fn normalize(text: &str, options: &NormalizeOptions) -> String {
    if text.trim().is_empty() {
        return String::new();
    }

    let lines: Vec<String> = text
        .lines()
        .map(|line| normalize_line(line, *options))
        .collect();
    let mut lines = apply_blank_line_policy(lines, options.blank_lines);
    // 「ファイル末尾は改行 1 つ」は空行の扱い方（Keep/Collapse/Remove）とは独立した
    // 決まりなので、本文の末尾に残った空行はここで取り除く。
    trim_trailing_blank_lines(&mut lines);

    let mut result: String = lines
        .iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    result.push('\n');
    result
}

fn trim_trailing_blank_lines(lines: &mut Vec<String>) {
    while lines.last().is_some_and(|line| is_blank_line(line)) {
        lines.pop();
    }
}

fn normalize_line(line: &str, options: NormalizeOptions) -> String {
    let mut result = line.to_string();
    if options.ellipsis {
        result = normalize_ellipsis(&result);
    }
    if options.dash {
        result = normalize_dash(&result);
    }
    if options.fullwidth_marks {
        result = normalize_fullwidth_marks(&result);
    }
    if options.paragraph_indent {
        result = indent_paragraph(&result);
    }
    result
}

fn is_blank_line(line: &str) -> bool {
    line.trim().is_empty()
}

fn apply_blank_line_policy(lines: Vec<String>, policy: BlankLinePolicy) -> Vec<String> {
    match policy {
        BlankLinePolicy::Keep => lines,
        BlankLinePolicy::Remove => lines
            .into_iter()
            .filter(|line| !is_blank_line(line))
            .collect(),
        BlankLinePolicy::Collapse => collapse_blank_lines(lines),
    }
}

fn collapse_blank_lines(lines: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(lines.len());
    let mut previous_was_blank = false;
    for line in lines {
        let blank = is_blank_line(&line);
        if blank && previous_was_blank {
            continue;
        }
        previous_was_blank = blank;
        result.push(line);
    }
    result
}

/// 段落頭を字下げする。すでに字下げ不要な記号（鉤括弧・ダッシュ・省略記号など）で
/// 始まる行と、空行はそのままにする。
fn indent_paragraph(line: &str) -> String {
    if is_blank_line(line) {
        return String::new();
    }
    let stripped = line.trim_start_matches([' ', '\u{3000}']);
    if starts_without_indent(stripped) {
        stripped.to_string()
    } else {
        format!("\u{3000}{stripped}")
    }
}

fn starts_without_indent(text: &str) -> bool {
    matches!(
        text.chars().next(),
        Some('「' | '『' | '（' | '〈' | '《' | '【' | '〔' | '―' | '…' | '\u{2018}' | '\u{201C}')
    )
}

/// `.`・`。`・`・` の 3 文字以上の連続を `……` に、`…` の連続は奇数個なら
/// 偶数個に切り上げる。
fn normalize_ellipsis(line: &str) -> String {
    normalize_symbol_runs(line, |c| matches!(c, '.' | '。' | '・'), 3, '…', "……")
}

/// `-`・`—` の 2 文字以上の連続を `――` に、`―` の連続は奇数個なら
/// 偶数個に切り上げる（長音記号「ー」は対象外）。
fn normalize_dash(line: &str) -> String {
    normalize_symbol_runs(line, |c| matches!(c, '-' | '—'), 2, '―', "――")
}

/// 記号の連続を正規化する共通処理。
///
/// `is_collapse_trigger` に該当する文字が `collapse_threshold` 個以上続いたら
/// `canonical` にまとめる。`parity_char` はすでに正しい記法（偶数個の連続）を
/// 変えず、奇数個の連続だけを 1 個増やして偶数にする。
fn normalize_symbol_runs(
    line: &str,
    is_collapse_trigger: impl Fn(char) -> bool,
    collapse_threshold: usize,
    parity_char: char,
    canonical: &str,
) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut output = String::with_capacity(line.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let run_len = chars[i..].iter().take_while(|&&x| x == c).count();

        if c == parity_char {
            let target = if run_len % 2 == 1 {
                run_len + 1
            } else {
                run_len
            };
            for _ in 0..target {
                output.push(parity_char);
            }
        } else if is_collapse_trigger(c) && run_len >= collapse_threshold {
            output.push_str(canonical);
        } else {
            for _ in 0..run_len {
                output.push(c);
            }
        }
        i += run_len;
    }

    output
}

/// 半角の `!`/`?` を全角にし、順序を `！？` にそろえ、続く文字が
/// 閉じ括弧・空白・改行・行末でなければ全角空白を補う。
fn normalize_fullwidth_marks(line: &str) -> String {
    let converted = convert_halfwidth_marks(line);
    let unified = unify_mark_order(&converted);
    insert_space_after_marks(&unified)
}

fn convert_halfwidth_marks(line: &str) -> String {
    line.chars()
        .map(|c| match c {
            '!' => '！',
            '?' => '？',
            other => other,
        })
        .collect()
}

/// `！`・`？` が連続する範囲ごとに、`！` をすべて前に、`？` をすべて後ろに
/// 並べ替える（`？！` → `！？`）。
///
/// 単純な `str::replace("？！", "！？")` だと、`？？！` のように 3 文字以上
/// 混ざった連続では 1 回の適用で並べ替えが収まりきらず、2 回目の適用で
/// さらに変化してしまい冪等にならない（`？？！` → `？！？` → `！？？`）。
/// 連続の中の `！`/`？` の個数だけを見て並べ替えることで、1 回の適用で
/// 安定した形になる。
fn unify_mark_order(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut output = String::with_capacity(line.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c == '！' || c == '？' {
            let run = &chars[i..];
            let run_len = run.iter().take_while(|&&x| x == '！' || x == '？').count();
            let bang_count = run[..run_len].iter().filter(|&&x| x == '！').count();
            let question_count = run_len - bang_count;
            output.extend(std::iter::repeat_n('！', bang_count));
            output.extend(std::iter::repeat_n('？', question_count));
            i += run_len;
            continue;
        }
        output.push(c);
        i += 1;
    }

    output
}

fn insert_space_after_marks(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut output = String::with_capacity(line.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c == '！' || c == '？' {
            let run_len = chars[i..]
                .iter()
                .take_while(|&&x| x == '！' || x == '？')
                .count();
            output.extend(&chars[i..i + run_len]);
            i += run_len;
            if let Some(next) = chars.get(i)
                && !is_closing_or_space(*next)
            {
                output.push('\u{3000}');
            }
            continue;
        }
        output.push(c);
        i += 1;
    }

    output
}

fn is_closing_or_space(c: char) -> bool {
    matches!(
        c,
        '」' | '』'
            | '》'
            | '〉'
            | '】'
            | '〕'
            | '）'
            | ')'
            | ']'
            | '”'
            | '’'
            | ' '
            | '\u{3000}'
            | '\t'
            | '\n'
    )
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn options() -> NormalizeOptions {
        NormalizeOptions::default()
    }

    #[test]
    fn empty_and_whitespace_only_input_returns_empty_string() {
        assert_eq!(normalize("", &options()), "");
        assert_eq!(normalize("   \n\t\n", &options()), "");
    }

    #[test]
    fn indents_paragraphs_that_lack_a_leading_symbol() {
        let result = normalize("朝が来た。", &options());
        assert_eq!(result, "\u{3000}朝が来た。\n");
    }

    #[test]
    fn does_not_indent_dialogue_or_leader_symbols() {
        let text = "「おはよう」\n（まだ眠い）\n――そして、静寂。\n……彼は黙っていた。";
        let result = normalize(text, &options());
        assert_eq!(
            result,
            "「おはよう」\n（まだ眠い）\n――そして、静寂。\n……彼は黙っていた。\n"
        );
    }

    #[test]
    fn unifies_ellipsis_variants() {
        let text = "そう思った...\nまさか。。。\n本当に・・・\nそして…";
        let result = normalize(text, &options());
        for line in result.lines() {
            assert!(
                line.contains("……") || line.is_empty(),
                "unexpected line: {line}"
            );
        }
    }

    #[test]
    fn unifies_dash_variants() {
        let text = "これは--本当の話だ\nこれは——本当の話だ\nこれは―本当の話だ";
        let result = normalize(text, &options());
        for line in result.lines() {
            assert!(line.contains("――"), "unexpected line: {line}");
        }
    }

    #[test]
    fn converts_halfwidth_marks_and_unifies_order() {
        let result = normalize("すごい!すごい?すごい!?すごい？！", &options());
        assert!(result.contains('！'));
        assert!(result.contains('？'));
        assert!(!result.contains('!'));
        assert!(!result.contains('?'));
        assert!(result.contains("！？"));
        assert!(!result.contains("？！"));
    }

    #[test]
    fn adds_space_after_marks_unless_followed_by_closing_or_space() {
        let result = normalize("すごい！次の文。\n「すごい！」\nすごい！", &options());
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines[0].contains("！\u{3000}次"));
        assert!(!lines[1].contains("！\u{3000}"));
        assert!(!lines[2].ends_with('\u{3000}'));
    }

    #[test]
    fn blank_line_policy_collapses_or_removes() {
        let text = "一段落目。\n\n\n二段落目。";

        let keep = normalize(
            text,
            &NormalizeOptions {
                blank_lines: BlankLinePolicy::Keep,
                ..options()
            },
        );
        assert_eq!(keep.lines().filter(|l| l.is_empty()).count(), 2);

        let collapse = normalize(
            text,
            &NormalizeOptions {
                blank_lines: BlankLinePolicy::Collapse,
                ..options()
            },
        );
        assert_eq!(collapse.lines().filter(|l| l.is_empty()).count(), 1);

        let remove = normalize(
            text,
            &NormalizeOptions {
                blank_lines: BlankLinePolicy::Remove,
                ..options()
            },
        );
        assert_eq!(remove.lines().filter(|l| l.is_empty()).count(), 0);
    }

    #[test]
    fn trims_trailing_whitespace_and_ends_with_single_newline() {
        let result = normalize("一行目。   \n二行目。\t", &options());
        assert!(result.ends_with("二行目。\n"));
        assert!(!result.ends_with("\n\n"));
    }

    #[test]
    fn is_idempotent_for_mixed_realistic_text() {
        let text = "朝が来た...\n「おはよう!」と彼は言った--そして笑った?\n\n\n『本当?』と彼女は聞き返した。";
        let once = normalize(text, &options());
        let twice = normalize(&once, &options());
        assert_eq!(once, twice);
    }

    #[test]
    fn fullwidth_marks_normalization_is_idempotent_for_all_mark_combinations() {
        // `！`/`？`（半角・全角）が入り混じるあらゆる組み合わせで、1 回目と 2 回目の
        // 適用結果が一致することを確認する（`？？！` → `？！？` → `！？？` のような
        // 収束しない並べ替えが起きないこと）。
        let combinations = [
            "!?",
            "?!",
            "！？",
            "？！",
            "!!",
            "??",
            "！！",
            "？？",
            "!!?",
            "??!",
            "?!?!",
            "!?!?",
            "！！？？",
            "？？！！",
            "!？",
            "？!",
            "!!!?",
            "????!!!!",
            "?",
            "!",
            "！",
            "？",
        ];
        for combo in combinations {
            let once = normalize(combo, &options());
            let twice = normalize(&once, &options());
            assert_eq!(
                once, twice,
                "combo {combo:?} is not idempotent: once={once:?}"
            );
        }
    }

    #[test]
    fn keep_policy_removes_only_trailing_blank_lines_and_keeps_interior_ones() {
        let text = "一段落目。\n\n二段落目。\n\n\n";
        let result = normalize(
            text,
            &NormalizeOptions {
                blank_lines: BlankLinePolicy::Keep,
                ..options()
            },
        );

        assert!(result.ends_with("二段落目。\n"));
        assert!(!result.ends_with("二段落目。\n\n"));
        // 段落の間の空行は Keep なので残る。
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "");
    }
}
