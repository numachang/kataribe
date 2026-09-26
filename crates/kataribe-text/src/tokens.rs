//! トークン数の保守的な見積もり。
//!
//! LLM に渡す文脈がモデルの上限を超えないようにするための見積もりなので、
//! 実際のトークナイザより少なく見積もることは避け、常に切り上げる。

/// 4 文字で 1 トークンとして切り上げる区分（ASCII の英数字、半角スペース・タブ）に
/// 何文字分の余地が使われているかを数える単位。
const CHARS_PER_TOKEN_FOR_ASCII: usize = 4;

/// 本文のトークン数を保守的に見積もる。
///
/// かな・カナ・漢字・全角記号など（ASCII の英数字と半角スペース・タブを除くすべて）
/// は 1 文字を 1 トークンとして数える。全角空白・改行は段落や字下げのたびに現れ、
/// 少なく見積もると危険なため、ここに含めて 1 文字 1 トークンとして扱う。
/// ASCII の英数字と半角スペース・タブだけは、英文が 4 文字前後で 1 トークンになる
/// 実際のトークナイザに合わせて、4 文字につき 1 トークンとして切り上げる。
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    let mut single_char_tokens = 0usize;
    let mut ascii_alnum_chars = 0usize;
    let mut ascii_space_or_tab_chars = 0usize;

    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            ascii_alnum_chars += 1;
        } else if is_ascii_space_or_tab(c) {
            ascii_space_or_tab_chars += 1;
        } else {
            single_char_tokens += 1;
        }
    }

    single_char_tokens
        + ascii_alnum_chars.div_ceil(CHARS_PER_TOKEN_FOR_ASCII)
        + ascii_space_or_tab_chars.div_ceil(CHARS_PER_TOKEN_FOR_ASCII)
}

fn is_ascii_space_or_tab(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn empty_text_has_no_tokens() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn kana_and_kanji_are_one_token_per_char() {
        assert_eq!(estimate_tokens("吾輩は猫である"), 7);
    }

    #[test]
    fn ascii_alnum_is_four_chars_per_token_rounded_up() {
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("a"), 1);
    }

    #[test]
    fn ascii_space_and_tab_are_four_chars_per_token_rounded_up() {
        assert_eq!(estimate_tokens("    "), 1);
        assert_eq!(estimate_tokens("\t\t\t\t\t"), 2);
    }

    #[test]
    fn fullwidth_space_and_newlines_are_one_token_per_char() {
        // 段落の字下げや改行のたびに現れるため、少なく見積もらないよう
        // 4 文字 1 トークンにはまとめず、1 文字 1 トークンとして数える。
        assert_eq!(estimate_tokens("\n\n\n\n\n"), 5);
        assert_eq!(estimate_tokens("\u{3000}\u{3000}\u{3000}"), 3);
    }

    #[test]
    fn fullwidth_punctuation_is_one_token_per_char() {
        assert_eq!(estimate_tokens("、。！？"), 4);
    }

    #[test]
    fn mixed_text_sums_each_category_independently() {
        // 漢字 2 + ASCII 5 文字(切り上げ2) + 空白 1 文字(切り上げ1) = 5
        let text = "漢字hello ";
        assert_eq!(estimate_tokens(text), 2 + 2 + 1);
    }
}
