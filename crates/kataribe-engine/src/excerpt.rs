//! 資料を文脈の予算に収めるための切り詰め。文の途中で切らない。

const OMISSION_MARK: &str = "（以下略）";

/// 先頭から `max_chars` 文字以内に収める。段落・文の区切りで切り、切った場合は省略の印を付ける。
pub(crate) fn head(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let limit = byte_offset_of_char(text, max_chars);
    let window = &text[..limit];
    let cut = last_boundary(window).unwrap_or(limit);
    format!("{}\n{OMISSION_MARK}", window[..cut].trim_end())
}

/// 末尾の `max_chars` 文字以内を取り出す。文の頭から始まるように切る。
pub(crate) fn tail(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    let total = text.chars().count();
    if max_chars == 0 {
        return String::new();
    }
    if total <= max_chars {
        return text.to_owned();
    }
    let start = byte_offset_of_char(text, total - max_chars);
    let window = &text[start..];
    let sentence_start = first_boundary(window).unwrap_or(0);
    window[sentence_start..].trim_start().to_owned()
}

/// Markdown の資料を `max_chars` 文字以内に収める。見出しに `preferred` の語を含む節を先に残し、
/// 残りの節は元の順で入るだけ入れる（文体ガイドの「文体見本」や人物資料の「口調」を落とさないため）。
pub(crate) fn prioritized(markdown: &str, preferred: &[&str], max_chars: usize) -> String {
    let sections = split_sections(markdown);
    let is_preferred = |index: &usize| {
        let heading = sections[*index].lines().next().unwrap_or_default();
        heading.starts_with('#') && preferred.iter().any(|word| heading.contains(word))
    };
    let priority_order = (0..sections.len())
        .filter(is_preferred)
        .chain((0..sections.len()).filter(|index| !is_preferred(index)));
    let mut remaining = max_chars;
    let mut chosen: Vec<(usize, String)> = Vec::new();
    for index in priority_order {
        if remaining == 0 {
            break;
        }
        let excerpt = head(sections[index], remaining);
        remaining = remaining.saturating_sub(excerpt.chars().count());
        chosen.push((index, excerpt));
    }
    chosen.sort_by_key(|(index, _)| *index);
    chosen
        .into_iter()
        .map(|(_, excerpt)| excerpt)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Markdown の資料から、見出しに `keyword` を含む最初の節の本文（見出しの行を除く）を
/// `max_chars` 文字以内で取り出す。該当する節が無ければ空文字列。
pub(crate) fn section(markdown: &str, keyword: &str, max_chars: usize) -> String {
    split_sections(markdown)
        .into_iter()
        .find(|section| {
            let heading = section.lines().next().unwrap_or_default();
            heading.starts_with('#') && heading.contains(keyword)
        })
        .map(|section| {
            let body = section.split_once('\n').map_or("", |(_, body)| body);
            head(body, max_chars)
        })
        .unwrap_or_default()
}

/// 見出し（`#` で始まる行）ごとに分ける。最初の見出しより前の部分も 1 つの節とする。
fn split_sections(markdown: &str) -> Vec<&str> {
    let markdown = markdown.trim();
    let mut starts: Vec<usize> = markdown
        .match_indices('\n')
        .map(|(offset, _)| offset + 1)
        .filter(|&offset| markdown[offset..].starts_with('#'))
        .collect();
    starts.insert(0, 0);
    starts.dedup();
    starts
        .iter()
        .zip(
            starts
                .iter()
                .skip(1)
                .chain(std::iter::once(&markdown.len())),
        )
        .map(|(&start, &end)| markdown[start..end].trim())
        .filter(|section| !section.is_empty())
        .collect()
}

fn byte_offset_of_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(offset, _)| offset)
}

/// 区切りの直後の位置（バイト）のうち最後のもの。段落の区切りを優先する。
fn last_boundary(window: &str) -> Option<usize> {
    let minimum = window.len() / 2;
    let paragraph = window.rfind('\n').filter(|&offset| offset >= minimum);
    paragraph.or_else(|| {
        window
            .char_indices()
            .rev()
            .filter(|&(_, character)| is_sentence_end(character))
            .map(|(offset, character)| offset + character.len_utf8())
            .find(|&offset| offset >= minimum)
    })
}

/// 区切りの直後の位置（バイト）のうち最初のもの。
fn first_boundary(window: &str) -> Option<usize> {
    let maximum = window.len() / 2;
    window
        .char_indices()
        .find(|&(_, character)| character == '\n' || is_sentence_end(character))
        .map(|(offset, character)| offset + character.len_utf8())
        .filter(|&offset| offset <= maximum)
}

fn is_sentence_end(character: char) -> bool {
    matches!(character, '。' | '！' | '？' | '」' | '』')
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn short_text_is_returned_whole() {
        assert_eq!(head("  短い文。 ", 10), "短い文。");
        assert_eq!(tail("短い文。", 10), "短い文。");
    }

    #[test]
    fn head_cuts_at_sentence_boundary_and_marks_omission() {
        let text = "一つ目の文です。二つ目の文です。三つ目の文です。";
        assert_eq!(
            head(text, 18),
            "一つ目の文です。二つ目の文です。\n（以下略）"
        );
    }

    #[test]
    fn head_prefers_paragraph_boundary() {
        let text = "第一段落の文。続きの文。\n第二段落の文です。さらに続く。";
        assert_eq!(head(text, 20), "第一段落の文。続きの文。\n（以下略）");
    }

    #[test]
    fn tail_starts_at_sentence_boundary() {
        let text = "一つ目の文です。二つ目の文です。三つ目の文です。";
        assert_eq!(tail(text, 12), "三つ目の文です。");
    }

    #[test]
    fn preferred_sections_survive_a_tight_budget() {
        let guide = "# 文体ガイド
## 人称と視点
三人称。
## 語り口
短い文を重ねる。長い説明はしない。
## 文体見本
　雨が降っていた。";
        let excerpt = prioritized(guide, &["文体見本"], 30);
        assert!(
            excerpt.contains(
                "## 文体見本
　雨が降っていた。"
            ),
            "{excerpt}"
        );
        assert!(
            excerpt.starts_with("# 文体ガイド"),
            "元の順序を保つ: {excerpt}"
        );
    }

    #[test]
    fn prioritized_keeps_everything_when_it_fits() {
        let text = "## 口調
一人称は「わたし」。
## 経歴
孤児院育ち。";
        assert_eq!(prioritized(text, &["口調"], 1000), text);
    }

    #[test]
    fn section_returns_the_body_under_the_matching_heading() {
        let text =
            "# 文体ガイド\n## 人称と視点\n- 三人称。\n- 視点は凛に固定。\n## 語り口\n現在形。";
        assert_eq!(
            section(text, "人称", 1000),
            "- 三人称。\n- 視点は凛に固定。"
        );
    }

    #[test]
    fn section_is_empty_when_no_heading_matches() {
        assert_eq!(section("## 語り口\n現在形。", "人称", 1000), "");
        assert_eq!(section("人称について書いた本文。", "人称", 1000), "");
    }

    #[test]
    fn zero_budget_yields_empty() {
        assert_eq!(head("文。", 0), "");
        assert_eq!(tail("文。", 0), "");
    }

    #[test]
    fn text_without_boundaries_is_cut_at_the_limit() {
        assert_eq!(head("あいうえおかきくけこ", 5), "あいうえお\n（以下略）");
        assert_eq!(tail("あいうえおかきくけこ", 5), "かきくけこ");
    }
}
