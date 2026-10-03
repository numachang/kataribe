//! ルビ（`|親《よみ》`・`｜親《よみ》`・`漢字《よみ》`）と傍点（`《《強調》》`）の解析。
//!
//! カクヨム・小説家になろう互換の記法を扱う。閉じ括弧が無い、読みが空、といった
//! 不正な記法は解析を諦め、その部分をそのまま本文として扱う（panic しない）。

use serde::{Deserialize, Serialize};

/// ルビ・傍点の探索を打ち切る最大文字数。
///
/// 実際の読み仮名や傍点がこれを超えることはまず無い。壊れた入力（閉じ括弧の無い
/// `《` が大量に続く、など）で探索が本文全体に及んで計算量が膨らむのを防ぐ。
const MAX_ANNOTATION_SCAN: usize = 256;

/// 本文を構成する断片。隣り合う [`Segment::Text`] は生成されない（常にまとめられる）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Segment {
    /// ルビ・傍点ではない、通常の本文。
    Text { text: String },
    /// ルビ。`base` が親文字、`reading` が読み仮名。
    Ruby { base: String, reading: String },
    /// 傍点（強調）。
    Emphasis { text: String },
}

/// 本文をルビ・傍点の記法にしたがって断片に分解する。
#[must_use]
pub fn parse(text: &str) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let mut segments = Vec::new();
    let mut plain_buffer = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if is_pipe(c) {
            if chars.get(i + 1) == Some(&'《') {
                // `|《` / `｜《`：記法のエスケープ。縦棒は消え、《 だけが本文になる。
                plain_buffer.push('《');
                i += 2;
                continue;
            }
            if let Some((base, reading, consumed)) = parse_pipe_ruby(&chars, i) {
                flush_text(&mut segments, &mut plain_buffer);
                segments.push(Segment::Ruby { base, reading });
                i += consumed;
                continue;
            }
            // 不正な記法：縦棒 1 文字を本文としてそのまま扱う。
            plain_buffer.push(c);
            i += 1;
            continue;
        }

        if c == '《' {
            if chars.get(i + 1) == Some(&'《') {
                if let Some((emphasis, consumed)) = parse_emphasis(&chars, i) {
                    flush_text(&mut segments, &mut plain_buffer);
                    segments.push(Segment::Emphasis { text: emphasis });
                    i += consumed;
                    continue;
                }
                plain_buffer.push(c);
                i += 1;
                continue;
            }
            if let Some((base, reading, consumed)) = parse_kanji_ruby(&chars, &plain_buffer, i) {
                truncate_trailing_chars(&mut plain_buffer, base.chars().count());
                flush_text(&mut segments, &mut plain_buffer);
                segments.push(Segment::Ruby { base, reading });
                i += consumed;
                continue;
            }
            // 不正な記法（直前に漢字が無い、または閉じ括弧が無い）：《 を本文として扱う。
            plain_buffer.push(c);
            i += 1;
            continue;
        }

        plain_buffer.push(c);
        i += 1;
    }

    flush_text(&mut segments, &mut plain_buffer);
    segments
}

/// 親文字だけを残した本文を返す（読み仮名・記号・傍点の記法は取り除く）。
#[must_use]
pub fn to_plain(text: &str) -> String {
    parse(text)
        .into_iter()
        .map(|segment| match segment {
            Segment::Text { text } | Segment::Emphasis { text } => text,
            Segment::Ruby { base, .. } => base,
        })
        .collect()
}

fn is_pipe(c: char) -> bool {
    matches!(c, '|' | '｜')
}

/// 漢字（CJK 統合漢字とその拡張・CJK 互換漢字）と、々・〆・ヶ・〇 を漢字とみなす。
///
/// 拡張 A（U+3400–U+4DBF）や、`𠮟` のような拡張 B 以降（U+20000–）の漢字、
/// `﨑` のような CJK 互換漢字（U+F900–）も対象にする。
/// `quality` モジュールが漢字率の計算に使うため `pub(crate)` にしている。
pub(crate) fn is_kanji(c: char) -> bool {
    matches!(
        c,
        '\u{3400}'..='\u{4DBF}'     // CJK 統合漢字拡張 A
        | '\u{4E00}'..='\u{9FFF}'   // CJK 統合漢字
        | '\u{F900}'..='\u{FAFF}'   // CJK 互換漢字
        | '\u{20000}'..='\u{2A6DF}' // CJK 統合漢字拡張 B
        | '\u{2A700}'..='\u{2EBEF}' // CJK 統合漢字拡張 C・D・E・F（連続したブロック）
        | '\u{2F800}'..='\u{2FA1F}' // CJK 互換漢字補助
        | '\u{30000}'..='\u{323AF}' // CJK 統合漢字拡張 G・H（連続したブロック）
        | '々' | '〆' | 'ヶ' | '〇'
    )
}

fn flush_text(segments: &mut Vec<Segment>, buffer: &mut String) {
    if buffer.is_empty() {
        return;
    }
    segments.push(Segment::Text {
        text: std::mem::take(buffer),
    });
}

/// `buffer` の末尾から `count` 文字を取り除く。
fn truncate_trailing_chars(buffer: &mut String, count: usize) {
    for _ in 0..count {
        buffer.pop();
    }
}

/// `start` 以降で最初に見つかった `target` の位置を返す。
///
/// 改行、走査上限、入力末尾に達したら諦める（不正な記法として扱う）。
fn find_within_line(chars: &[char], start: usize, target: char) -> Option<usize> {
    let limit = chars.len().min(start + MAX_ANNOTATION_SCAN);
    for (offset, &c) in chars[start..limit].iter().enumerate() {
        match c {
            c if c == target => return Some(start + offset),
            '\n' => return None,
            _ => {}
        }
    }
    None
}

/// `start` 以降で最初に見つかった `》》`（2 文字連続）の開始位置を返す。
fn find_double_close(chars: &[char], start: usize) -> Option<usize> {
    let limit = chars.len().min(start + MAX_ANNOTATION_SCAN);
    for (offset, &c) in chars[start..limit].iter().enumerate() {
        let idx = start + offset;
        if c == '\n' {
            return None;
        }
        if c == '》' && chars.get(idx + 1) == Some(&'》') {
            return Some(idx);
        }
    }
    None
}

/// `|親《よみ》` / `｜親《よみ》` を解析する。`pipe_index` は縦棒の位置。
///
/// 成功したら `(親文字, よみ, 消費した文字数)` を返す。
fn parse_pipe_ruby(chars: &[char], pipe_index: usize) -> Option<(String, String, usize)> {
    let open = find_within_line(chars, pipe_index + 1, '《')?;
    let close = find_within_line(chars, open + 1, '》')?;

    let base: String = chars[pipe_index + 1..open].iter().collect();
    let reading: String = chars[open + 1..close].iter().collect();
    if reading.is_empty() {
        return None;
    }

    Some((base, reading, close + 1 - pipe_index))
}

/// `漢字《よみ》` を解析する。`bracket_index` は 《 の位置。
///
/// `plain_buffer` の末尾にある連続した漢字を親文字の候補とする。
/// 成功したら `(親文字, よみ, 《 から 》 までの消費文字数)` を返す。
fn parse_kanji_ruby(
    chars: &[char],
    plain_buffer: &str,
    bracket_index: usize,
) -> Option<(String, String, usize)> {
    let base: String = plain_buffer
        .chars()
        .rev()
        .take(MAX_ANNOTATION_SCAN)
        .take_while(|&c| is_kanji(c))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if base.is_empty() {
        return None;
    }

    let close = find_within_line(chars, bracket_index + 1, '》')?;
    let reading: String = chars[bracket_index + 1..close].iter().collect();
    if reading.is_empty() {
        return None;
    }

    Some((base, reading, close + 1 - bracket_index))
}

/// `《《傍点》》` を解析する。`start` は最初の 《 の位置。
///
/// 成功したら `(傍点の文字列, 消費した文字数)` を返す。
fn parse_emphasis(chars: &[char], start: usize) -> Option<(String, usize)> {
    let close = find_double_close(chars, start + 2)?;
    let text: String = chars[start + 2..close].iter().collect();
    if text.is_empty() {
        return None;
    }
    Some((text, close + 2 - start))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn parses_pipe_ruby_with_halfwidth_and_fullwidth_bar() {
        assert_eq!(
            parse("|東京《とうきょう》"),
            vec![Segment::Ruby {
                base: "東京".to_string(),
                reading: "とうきょう".to_string()
            }]
        );
        assert_eq!(
            parse("｜東京《とうきょう》"),
            vec![Segment::Ruby {
                base: "東京".to_string(),
                reading: "とうきょう".to_string()
            }]
        );
    }

    #[test]
    fn parses_kanji_ruby_without_bar() {
        assert_eq!(
            parse("漢字《かんじ》"),
            vec![Segment::Ruby {
                base: "漢字".to_string(),
                reading: "かんじ".to_string()
            }]
        );
    }

    #[test]
    fn kanji_ruby_base_is_maximal_trailing_kanji_run() {
        let segments = parse("食べた漢字《かんじ》");
        assert_eq!(
            segments,
            vec![
                Segment::Text {
                    text: "食べた".to_string()
                },
                Segment::Ruby {
                    base: "漢字".to_string(),
                    reading: "かんじ".to_string()
                },
            ]
        );
    }

    #[test]
    fn kanji_ruby_base_recognizes_extension_and_compatibility_kanji() {
        // 𠮟（CJK 統合漢字拡張 B）と 﨑（CJK 互換漢字）。
        let segments = parse("𠮟《しっ》られた");
        assert_eq!(
            segments,
            vec![
                Segment::Ruby {
                    base: "𠮟".to_string(),
                    reading: "しっ".to_string()
                },
                Segment::Text {
                    text: "られた".to_string()
                },
            ]
        );

        let segments = parse("山﨑《やまさき》さん");
        assert_eq!(
            segments,
            vec![
                Segment::Ruby {
                    base: "山﨑".to_string(),
                    reading: "やまさき".to_string()
                },
                Segment::Text {
                    text: "さん".to_string()
                },
            ]
        );
    }

    #[test]
    fn non_kanji_before_bracket_is_not_ruby() {
        // 直前が漢字でなければルビとして扱わず、そのまま本文。
        let segments = parse("ひらがな《よみ》");
        assert_eq!(
            segments,
            vec![Segment::Text {
                text: "ひらがな《よみ》".to_string()
            }]
        );
    }

    #[test]
    fn parses_emphasis() {
        assert_eq!(
            parse("《《強調》》"),
            vec![Segment::Emphasis {
                text: "強調".to_string()
            }]
        );
    }

    #[test]
    fn parses_escape_for_literal_bracket() {
        assert_eq!(
            parse("|《これはただの括弧"),
            vec![Segment::Text {
                text: "《これはただの括弧".to_string()
            }]
        );
        assert_eq!(
            parse("｜《注釈"),
            vec![Segment::Text {
                text: "《注釈".to_string()
            }]
        );
    }

    #[test]
    fn missing_closing_bracket_falls_back_to_plain_text() {
        let segments = parse("|親文字《よみ");
        assert_eq!(
            segments,
            vec![Segment::Text {
                text: "|親文字《よみ".to_string()
            }]
        );

        let segments = parse("漢字《よみ");
        assert_eq!(
            segments,
            vec![Segment::Text {
                text: "漢字《よみ".to_string()
            }]
        );
    }

    #[test]
    fn empty_reading_falls_back_to_plain_text() {
        assert_eq!(
            parse("漢字《》"),
            vec![Segment::Text {
                text: "漢字《》".to_string()
            }]
        );
        assert_eq!(
            parse("|漢字《》"),
            vec![Segment::Text {
                text: "|漢字《》".to_string()
            }]
        );
    }

    #[test]
    fn newline_before_closing_bracket_invalidates_annotation() {
        let segments = parse("漢字《よみ\n続き》");
        assert_eq!(
            segments,
            vec![Segment::Text {
                text: "漢字《よみ\n続き》".to_string()
            }]
        );
    }

    #[test]
    fn to_plain_keeps_base_and_emphasis_but_drops_reading() {
        let text = "|東京《とうきょう》に《《到着》》した。漢字《かんじ》も。";
        assert_eq!(to_plain(text), "東京に到着した。漢字も。");
    }

    #[test]
    fn mixed_paragraph_with_dialogue_and_ruby_parses_without_panicking() {
        let text = "「|明日《あす》は晴れるだろう」と、彼女は《《静かに》》呟いた。\n漢字《かんじ》の練習をした。";
        let segments = parse(text);
        assert_eq!(
            to_plain(text),
            "「明日は晴れるだろう」と、彼女は静かに呟いた。\n漢字の練習をした。"
        );
        // どのセグメントも空文字列にはならない。
        for segment in &segments {
            match segment {
                Segment::Ruby { base, reading } => {
                    assert_ne!(base, "");
                    assert_ne!(reading, "");
                }
                Segment::Text { text } | Segment::Emphasis { text } => assert_ne!(text, ""),
            }
        }
    }

    #[test]
    fn does_not_panic_on_pathological_input() {
        // 閉じ括弧の無い 《 が大量に続いても panic せず、すべて本文として扱われる。
        let text = "《".repeat(5000);
        let segments = parse(&text);
        assert_eq!(segments, vec![Segment::Text { text: text.clone() }]);
        assert_eq!(to_plain(&text), text);
    }
}
