//! LLM の出力から、本文や資料として使う部分だけを取り出す後処理。

use std::collections::HashSet;

use kataribe_text::meta::{is_meta_line, is_meta_phrase_line};
use kataribe_text::normalize::{BlankLinePolicy, NormalizeOptions, normalize};

/// シーン区切りとみなす記号。「・・・」は本文の沈黙の表現でもあるので含めない。
const SEPARATOR_SYMBOLS: &str = "◇◆□■＊*＃#-=—~〜";

/// 新しく書かせたシーン本文の後処理。前置き・後書き・見出し・区切り・直前の本文との重複を除き、表記を整える。
pub(crate) fn clean_prose(raw: &str, previous_text: &str) -> String {
    let body = trim_edges(&remove_code_fences(raw), is_meta_line);
    let body = remove_separator_lines(&body);
    let body = strip_overlap(previous_text, &body);
    let body = drop_repeated_paragraphs(previous_text, &body);
    normalize(
        &body,
        &NormalizeOptions {
            blank_lines: BlankLinePolicy::Remove,
            ..NormalizeOptions::default()
        },
    )
}

/// 書き直させた本文の後処理。元の原稿の書式（空行・字下げ・記号）を保つため、
/// コードブロックと前置き・後書きの決まり文句を除くだけにする。
pub(crate) fn clean_revision(raw: &str) -> String {
    finish_with_newline(&trim_edges(&remove_code_fences(raw), is_meta_phrase_line))
}

/// 章単位で書かせた出力を、「◇」などの区切り行でシーンごとに分ける。
pub(crate) fn split_scenes(raw: &str) -> Vec<String> {
    let mut scenes = vec![String::new()];
    for line in remove_code_fences(raw).lines() {
        if is_separator_line(line) {
            scenes.push(String::new());
        } else if let Some(current) = scenes.last_mut() {
            current.push_str(line);
            current.push('\n');
        }
    }
    scenes.retain(|scene| !scene.trim().is_empty());
    scenes
}

/// 要約など、見出しの無い短い文章の後処理。前置き・後書きを除き、前後の空白を落とす。
pub(crate) fn clean_plain(raw: &str) -> String {
    trim_edges(&remove_code_fences(raw), is_meta_line)
        .trim()
        .to_owned()
}

/// Markdown の資料の後処理。前後の前置き・後書きの決まり文句だけを除く。
/// 箇条書きや強調などの Markdown の記法は、資料の本文として残す。
pub(crate) fn clean_markdown(raw: &str) -> String {
    finish_with_newline(&trim_edges(&remove_code_fences(raw), is_meta_phrase_line))
}

/// front matter 付きの文書の後処理。最初の `---` より前と、末尾の後書きを除く。
/// front matter が見つからなければ、通常の Markdown として扱う。
pub(crate) fn clean_front_matter_document(raw: &str) -> String {
    let text = remove_code_fences(raw);
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.trim_end() == "---") else {
        return clean_markdown(raw);
    };
    if !lines[start + 1..]
        .iter()
        .any(|line| line.trim_end() == "---")
    {
        return clean_markdown(raw);
    }
    finish_with_newline(&trim_edges(&lines[start..].join("\n"), is_meta_phrase_line))
}

/// 出力が上限で途中まで切れたとき、最後の書きかけの文を落とす。
pub(crate) fn drop_unfinished_sentence(text: &str) -> String {
    let trimmed = text.trim_end();
    let finished = trimmed
        .char_indices()
        .rev()
        .find(|&(_, character)| is_sentence_end(character))
        .map_or(0, |(offset, character)| offset + character.len_utf8());
    finish_with_newline(&trimmed[..finished])
}

fn is_sentence_end(character: char) -> bool {
    matches!(
        character,
        '。' | '」' | '』' | '！' | '？' | '…' | '―' | '）'
    )
}

fn remove_code_fences(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 先頭と末尾から、空行と `is_noise` に当たる行を取り除く。
fn trim_edges(text: &str, is_noise: fn(&str) -> bool) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let is_edge_noise = |line: &&str| line.trim().is_empty() || is_noise(line);
    match (
        lines.iter().position(|line| !is_edge_noise(line)),
        lines.iter().rposition(|line| !is_edge_noise(line)),
    ) {
        (Some(start), Some(end)) => lines[start..=end].join("\n"),
        _ => String::new(),
    }
}

fn finish_with_newline(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

fn remove_separator_lines(text: &str) -> String {
    text.lines()
        .filter(|line| !is_separator_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 「◇」「＊ ＊ ＊」「---」のような、区切りの記号（と空白）だけでできた行。
fn is_separator_line(line: &str) -> bool {
    let mut symbols = line
        .chars()
        .filter(|character| !character.is_whitespace())
        .peekable();
    symbols.peek().is_some() && symbols.all(|character| SEPARATOR_SYMBOLS.contains(character))
}

/// 続きとして書かせた文章が、直前の本文の末尾をそのまま繰り返して始まっていたら、その部分を除く。
fn strip_overlap(previous_text: &str, next_text: &str) -> String {
    const MIN_OVERLAP: usize = 8;
    const MAX_OVERLAP: usize = 400;

    let previous_key: Vec<char> = previous_text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let next_key: Vec<char> = next_text.chars().filter(|c| !c.is_whitespace()).collect();
    let longest = (MIN_OVERLAP..=MAX_OVERLAP.min(next_key.len()))
        .rev()
        .find(|&length| previous_key.ends_with(&next_key[..length]));
    match longest {
        Some(length) => skip_non_whitespace(next_text, length).to_owned(),
        None => next_text.to_owned(),
    }
}

/// 直前の本文、またはこの出力の前のほうと一字一句同じ段落を除く。
/// 書き継ぎで LLM が直前の本文を書き写したり、同じ段落を繰り返したりする失敗への対策。
/// 短い段落（「はい」など）は偶然の一致がありうるので残す。
fn drop_repeated_paragraphs(previous_text: &str, next_text: &str) -> String {
    const MIN_PARAGRAPH_CHARS: usize = 15;
    let is_long = |key: &String| key.chars().count() >= MIN_PARAGRAPH_CHARS;
    let mut seen: HashSet<String> = previous_text
        .lines()
        .map(paragraph_key)
        .filter(is_long)
        .collect();
    next_text
        .lines()
        .filter(|line| {
            let key = paragraph_key(line);
            !is_long(&key) || seen.insert(key)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 段落の比較に使う形（字下げなどの空白を除いたもの）。
fn paragraph_key(line: &str) -> String {
    line.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

/// 空白以外の文字を `count` 個読み飛ばした残り。
fn skip_non_whitespace(text: &str, count: usize) -> &str {
    let mut remaining = count;
    for (offset, character) in text.char_indices() {
        if remaining == 0 {
            return &text[offset..];
        }
        if !character.is_whitespace() {
            remaining -= 1;
        }
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn prose_loses_preamble_headings_and_trailer() {
        let raw = "以下が本文です。\n\n# 第一章\n\n雨が降っていた。\n\n「行こう」\n\n（続く）\n";
        assert_eq!(clean_prose(raw, ""), "　雨が降っていた。\n「行こう」\n");
    }

    #[test]
    fn prose_loses_code_fences_and_separators() {
        let raw = "```\n雨が降っていた。\n◇\n風が吹いた。\n```";
        assert_eq!(clean_prose(raw, ""), "　雨が降っていた。\n　風が吹いた。\n");
    }

    #[test]
    fn prose_keeps_natural_first_and_last_lines() {
        let raw =
            "この物語は、僕が彼女と出会うまでの記録だ。\n（このまま、ずっと続くといいのに）\n";
        assert_eq!(
            clean_prose(raw, ""),
            "　この物語は、僕が彼女と出会うまでの記録だ。\n（このまま、ずっと続くといいのに）\n"
        );
    }

    #[test]
    fn prose_loses_paragraphs_copied_from_the_previous_text() {
        let previous = "　嵐が館の骨組みを震わせ、窓が鳴った。\n「はい」\n";
        let raw =
            "凛は耳を澄ませた。\n嵐が館の骨組みを震わせ、窓が鳴った。\n「はい」\n佐藤が頷いた。\n";
        assert_eq!(
            clean_prose(raw, previous),
            "　凛は耳を澄ませた。\n「はい」\n　佐藤が頷いた。\n"
        );
    }

    #[test]
    fn prose_loses_paragraphs_repeated_within_the_output() {
        let raw = "書斎の扉は、内側から固く閉ざされていた。\n凛は息を止めた。\n書斎の扉は、内側から固く閉ざされていた。\n";
        assert_eq!(
            clean_prose(raw, ""),
            "　書斎の扉は、内側から固く閉ざされていた。\n　凛は息を止めた。\n"
        );
    }

    #[test]
    fn dotted_silence_is_not_a_separator() {
        assert!(!is_separator_line("・・・"));
        assert!(is_separator_line("＊ ＊ ＊"));
        assert!(is_separator_line("　◇　"));
    }

    #[test]
    fn continuation_that_repeats_the_previous_ending_is_trimmed() {
        let previous = "　彼女は窓の外を見た。雨はまだ止みそうになかった。\n";
        let next = "雨はまだ止みそうになかった。\n　傘を手に取る。";
        assert_eq!(clean_prose(next, previous), "　傘を手に取る。\n");
    }

    #[test]
    fn short_accidental_overlap_is_kept() {
        assert_eq!(
            strip_overlap("彼は言った。", "言った。それから"),
            "言った。それから"
        );
    }

    #[test]
    fn chapter_output_is_split_at_separator_lines() {
        let raw = "一つ目のシーン。\n\n◇\n\n二つ目のシーン。\n＊ ＊ ＊\n三つ目。\n";
        assert_eq!(
            split_scenes(raw),
            vec!["一つ目のシーン。\n\n", "\n二つ目のシーン。\n", "三つ目。\n"]
        );
    }

    #[test]
    fn markdown_keeps_trailing_lists_and_drops_chatter() {
        let raw = concat!(
            "承知しました。以下が世界観です。\n\n",
            "# 世界観\n## 用語\n- 霧笛: 岬の灯台の合図\n- 潮見: 館の見張り\n\n",
            "いかがでしょうか。\n",
        );
        assert_eq!(
            clean_markdown(raw),
            "# 世界観\n## 用語\n- 霧笛: 岬の灯台の合図\n- 潮見: 館の見張り\n"
        );
    }

    #[test]
    fn markdown_without_headings_is_kept() {
        assert_eq!(clean_markdown("ただの文章です。\n"), "ただの文章です。\n");
    }

    #[test]
    fn revision_keeps_the_manuscript_layout() {
        let raw = "書き直しました。以下が新しい原稿です。\n```\n雨が降っていた。\n\n　風が吹いた。\n```\n";
        assert_eq!(clean_revision(raw), "雨が降っていた。\n\n　風が吹いた。\n");
    }

    #[test]
    fn front_matter_document_keeps_yaml_and_drops_chatter() {
        let raw = concat!(
            "書き直しました。\n```markdown\n",
            "---\nname: 凛\n---\n## 性格\n快活。\n",
            "```\n以上です。\n",
        );
        assert_eq!(
            clean_front_matter_document(raw),
            "---\nname: 凛\n---\n## 性格\n快活。\n"
        );
    }

    #[test]
    fn unfinished_last_sentence_is_dropped() {
        assert_eq!(
            drop_unfinished_sentence("雨が降った。風が吹い"),
            "雨が降った。\n"
        );
        assert_eq!(
            drop_unfinished_sentence("「行こう」と彼は言っ"),
            "「行こう」\n"
        );
        assert_eq!(drop_unfinished_sentence("書きかけ"), "");
    }
}
