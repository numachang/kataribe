//! ファイル名に使う slug（小文字英数字とハイフン）の規則。
//!
//! 人物の id と世界観の資料の名前が同じ規則を共有する。

use crate::path::is_windows_reserved_name;

/// slug の最大の長さ。
pub(crate) const MAX_LEN: usize = 48;

/// 規則（小文字英数字とハイフン。端と連続にハイフンを置かない。予約名でない。48 文字以内）に合うか。
pub(crate) fn is_valid(input: &str) -> bool {
    if input.is_empty() || input.len() > MAX_LEN || !input.is_ascii() {
        return false;
    }
    if is_windows_reserved_name(input) {
        return false;
    }
    let bytes = input.as_bytes();
    if bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' {
        return false;
    }
    let mut previous_was_hyphen = false;
    for &byte in bytes {
        let is_alnum = byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if is_alnum {
            previous_was_hyphen = false;
        } else if byte == b'-' && !previous_was_hyphen {
            previous_was_hyphen = true;
        } else {
            return false;
        }
    }
    true
}

/// 任意の文字列から slug を作る。
///
/// 小文字化し、英数字以外の並びをハイフン一つに正規化し、端のハイフンと長さを整える。
/// 結果が空なら `fallback`、予約名なら `-id` を付け、`is_taken` が真を返すあいだは `-2`, `-3`, … を付ける。
pub(crate) fn from_hint(hint: &str, fallback: &str, is_taken: impl Fn(&str) -> bool) -> String {
    let mut slug = slugify(hint);
    if slug.is_empty() {
        fallback.clone_into(&mut slug);
    }
    if is_windows_reserved_name(&slug) {
        slug = truncate(&format!("{slug}-id"), MAX_LEN);
    }
    if !is_taken(&slug) {
        return slug;
    }
    let mut suffix_number = 2u32;
    loop {
        let candidate = with_suffix(&slug, suffix_number);
        if !is_taken(&candidate) {
            return candidate;
        }
        suffix_number += 1;
    }
}

fn slugify(hint: &str) -> String {
    let mut slug = String::with_capacity(hint.len());
    for ch in hint.to_lowercase().chars() {
        if matches!(ch, 'a'..='z' | '0'..='9') {
            slug.push(ch);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    truncate(&slug, MAX_LEN)
}

fn truncate(slug: &str, max_len: usize) -> String {
    if slug.len() <= max_len {
        return slug.to_string();
    }
    let mut truncated = slug.chars().take(max_len).collect::<String>();
    while truncated.ends_with('-') {
        truncated.pop();
    }
    truncated
}

fn with_suffix(base: &str, number: u32) -> String {
    let suffix = format!("-{number}");
    let max_base_len = MAX_LEN.saturating_sub(suffix.len());
    let base = truncate(base, max_base_len);
    format!("{base}{suffix}")
}
