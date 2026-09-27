//! LLM が出力しがちな、本文ではない行（前置き・後書き・見出し・区切り）の検出。
//!
//! `kataribe-engine` はここで得た判定を使い、生成結果の先頭・末尾から
//! 本文ではない行を取り除く。`kataribe_text::quality` の
//! `meta_commentary` / `markdown_artifact` の検出もここに揃える。

/// LLM の前置き・後書き・見出し・区切りとして書きがちな行かどうかを、
/// メタ発言らしいか（[`is_meta_phrase_line`]）と Markdown の記法か
/// （[`is_markdown_artifact_line`]）のどちらかで判定する。
///
/// 「」『』で始まる本文中の自然なせりふや、地の文として自然な文は false になる。
#[must_use]
pub fn is_meta_line(line: &str) -> bool {
    is_markdown_artifact_line(line) || is_meta_phrase_line(line)
}

/// Markdown の記法だけの行（見出し・強調・区切り線・コードフェンス・箇条書き）かどうか。
#[must_use]
pub fn is_markdown_artifact_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    is_heading(trimmed)
        || is_horizontal_rule(trimmed)
        || is_bold_only(trimmed)
        || is_code_fence(trimmed)
        || is_list_item(trimmed)
}

/// LLM の前置き・後書き・見出し（Markdown ではないもの）らしい行かどうか。
///
/// 「以下は本文です」「承知しました」のような決まり文句、`【本文】` のような
/// 角括弧の見出し、`シーン1：` のような場面の区切り、`（続く）` のような
/// 注記を対象にする。誤検知を避けるため、対話の鉤括弧を含む行は対象にしない。
#[must_use]
pub fn is_meta_phrase_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || contains_dialogue_quote(trimmed) {
        return false;
    }

    META_PHRASE_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        || is_fiction_disclaimer(trimmed)
        || is_bracket_header(trimmed)
        || is_scene_or_chapter_marker(trimmed)
        || is_parenthetical_note(trimmed)
}

// 「この物語は」「以上が」は地の文・せりふでも自然に使われる（「この物語は、僕が
// 彼女と出会うまでの記録だ。」「以上が、私の推理のすべてだ。」など）ため、決まり文句
// の一覧には入れない。「この物語は」は [`is_fiction_disclaimer`] で「フィクションで
// す」と続く明白な場合だけを拾う。
const META_PHRASE_PREFIXES: &[&str] = &[
    "以下は",
    "以下、",
    "以下が",
    "承知しました",
    "かしこまりました",
    "AIとして",
    "本文：",
    "本文:",
    "以上です",
    "いかがでしょうか",
    "ご確認ください",
    "了解しました",
    "書き直しました",
    "修正しました",
    "推敲しました",
];

/// 「この物語はフィクションです」のような、創作物であることの明白な断り書きか。
/// 「この物語は、〜の記録だ。」のような自然な書き出しと区別するため、
/// 「フィクション」という語を含む場合だけに限る。
fn is_fiction_disclaimer(trimmed: &str) -> bool {
    trimmed.starts_with("この物語は") && trimmed.contains("フィクション")
}

/// 章・シーンの見出しとみなす行の長さの上限。これより長い行は本文とみなす。
const MAX_MARKER_CHARS: usize = 40;

fn contains_dialogue_quote(trimmed: &str) -> bool {
    trimmed.contains('「') || trimmed.contains('『')
}

fn is_heading(trimmed: &str) -> bool {
    trimmed.starts_with('#')
}

fn is_horizontal_rule(trimmed: &str) -> bool {
    trimmed.chars().count() >= 3 && trimmed.chars().all(|c| matches!(c, '-' | '_' | '*'))
}

fn is_bold_only(trimmed: &str) -> bool {
    trimmed.starts_with("**") && trimmed.ends_with("**") && trimmed.chars().count() > 4
}

fn is_code_fence(trimmed: &str) -> bool {
    trimmed.starts_with("```")
}

fn is_list_item(trimmed: &str) -> bool {
    if trimmed.starts_with("- ") {
        return true;
    }
    let digit_count = trimmed.chars().take_while(char::is_ascii_digit).count();
    digit_count > 0 && trimmed[digit_count..].starts_with(". ")
}

/// `【本文】` のように、行全体が角括弧だけで構成される見出しかどうか。
fn is_bracket_header(trimmed: &str) -> bool {
    trimmed.starts_with('【') && trimmed.ends_with('】') && trimmed.chars().count() > 2
}

/// `シーン1：` や `第一章：` のような場面・章の区切りかどうか。
///
/// 「第二の犠牲者が出たという話を…」のような本文と区別するため、見出しの形
/// （`第`＋数字＋`章`/`話`/`幕`/`部`、または `シーン`＋数字で始まる）に厳密に限り、
/// さらに句点を含まない短い行であることも求める。
fn is_scene_or_chapter_marker(trimmed: &str) -> bool {
    let looks_like_heading = !trimmed.contains('。') && trimmed.chars().count() <= MAX_MARKER_CHARS;
    looks_like_heading && (is_chapter_marker(trimmed) || is_scene_marker(trimmed))
}

/// `第一章`・`第3話`・`第一幕`・`第二部` のように、`第` の直後に数字が続き、
/// 数字の直後が `章`/`話`/`幕`/`部` である行の先頭部分かどうか。
fn is_chapter_marker(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix('第') else {
        return false;
    };
    let digit_count = rest
        .chars()
        .take_while(|&c| c.is_ascii_digit() || is_kanji_numeral(c))
        .count();
    if digit_count == 0 {
        return false;
    }
    rest.chars()
        .nth(digit_count)
        .is_some_and(|c| matches!(c, '章' | '話' | '幕' | '部'))
}

/// `シーン1`・`シーン二` のように、`シーン` の直後が数字である行の先頭部分かどうか。
fn is_scene_marker(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("シーン") else {
        return false;
    };
    rest.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || is_kanji_numeral(c))
}

/// `（続く）`・`（約1500字）` のように、行全体が括弧書きの注記かどうか。
///
/// 「（このまま、ずっと続くといいのに）」「（あと一字でも…）」のような本文中の
/// 独白と区別するため、中身が決まった言い回しとほぼ完全に一致する場合だけを拾う。
fn is_parenthetical_note(trimmed: &str) -> bool {
    let Some(inner) = strip_parens(trimmed) else {
        return false;
    };
    if inner.is_empty() {
        return false;
    }
    is_continuation_note(inner) || is_char_count_note(inner) || inner.ends_with("ここまで")
}

/// 「続く」「つづく」「続きます」「完」「了」のいずれかとの完全一致。
fn is_continuation_note(inner: &str) -> bool {
    matches!(inner, "続く" | "つづく" | "続きます" | "完" | "了")
}

/// 「（約）N字（程度）」の形（N は算用数字・漢数字）かどうか。
fn is_char_count_note(inner: &str) -> bool {
    let without_prefix = inner.strip_prefix('約').unwrap_or(inner);
    let without_suffix = without_prefix
        .strip_suffix("程度")
        .unwrap_or(without_prefix);
    let Some(digits) = without_suffix.strip_suffix('字') else {
        return false;
    };
    !digits.is_empty()
        && digits
            .chars()
            .all(|c| c.is_ascii_digit() || is_kanji_numeral(c))
}

fn strip_parens(trimmed: &str) -> Option<&str> {
    let without_prefix = trimmed
        .strip_prefix('（')
        .or_else(|| trimmed.strip_prefix('('))?;
    without_prefix
        .strip_suffix('）')
        .or_else(|| without_prefix.strip_suffix(')'))
}

fn is_kanji_numeral(c: char) -> bool {
    matches!(
        c,
        '〇' | '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十' | '百' | '千'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_llm_preface_and_afterword_phrases() {
        assert!(is_meta_line("以下は本文です。"));
        assert!(is_meta_line("承知しました。それでは本文を書きます。"));
        assert!(is_meta_line("かしこまりました。"));
        assert!(is_meta_line("この物語はフィクションです。"));
        assert!(is_meta_line("AIとして、これは架空の物語です。"));
        assert!(is_meta_line("本文：\n"));
        assert!(is_meta_line("（続く）"));
        assert!(is_meta_line("（約1500字）"));
        assert!(is_meta_line("（本文ここまで）"));
        assert!(is_meta_line("以上です。"));
        assert!(is_meta_line("いかがでしょうか。"));
        assert!(is_meta_line("ご確認ください。"));
        assert!(is_meta_line("書き直しました。以下が新しい文書です。"));
        assert!(!is_meta_line("彼は計画を修正しました。"));
    }

    #[test]
    fn detects_markdown_and_heading_style_artifacts() {
        assert!(is_meta_line("# 第一章"));
        assert!(is_meta_line("**第1章**"));
        assert!(is_meta_line("【本文】"));
        assert!(is_meta_line("シーン1：事務所"));
        assert!(is_meta_line("---"));
        assert!(is_meta_line("```"));
        assert!(is_meta_line("- 箇条書き"));
        assert!(is_meta_line("1. 箇条書き"));
    }

    #[test]
    fn does_not_flag_natural_prose_lines() {
        assert!(!is_meta_line("以下の通りだ、と彼は言った。"));
        assert!(!is_meta_line("第三の男が現れた。"));
        assert!(!is_meta_line("第六感が告げていた。あの男の話は嘘だ。"));
        assert!(!is_meta_line("以上の理由から、彼は町を出た。"));
        assert!(!is_meta_line("「承知しました」と彼女は頷いた。"));
        assert!(!is_meta_line("『本文』という表題の本を読んでいた。"));
        assert!(!is_meta_line("雨が静かに降り続いていた。"));
        assert!(!is_meta_line("彼は（本当は嘘だったのだが）頷いた。"));
        // レビューで見つかった誤検知の回帰テスト。
        assert!(!is_meta_line("この物語は、僕が彼女と出会うまでの記録だ。"));
        assert!(!is_meta_line("以上が、私の推理のすべてだ。"));
        assert!(!is_meta_line("（このまま、ずっと続くといいのに）"));
        assert!(!is_meta_line("（あと一字でも…）"));
        assert!(!is_meta_line("第二の犠牲者が出たという話を…――"));
    }

    #[test]
    fn does_not_flag_blank_lines() {
        assert!(!is_meta_line(""));
        assert!(!is_meta_line("   "));
    }
}
