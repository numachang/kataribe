//! 作品フォルダ内の相対パス（`RelPath`）。
//!
//! 区切り文字は必ず `/`。絶対パス・ドライブ指定・`..`・Windows の予約名など、
//! ファイルシステム上で問題を起こしうる入力をすべて拒否する。

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// パス全体の長さの上限（文字数）。
const MAX_LENGTH: usize = 260;

/// どの要素にも含めてはいけない文字。
const FORBIDDEN_CHARS: [char; 7] = [':', '*', '?', '"', '<', '>', '|'];

/// Windows の予約デバイス名（拡張子を除いた部分と大小文字を無視して比較する）。
const RESERVED_STEMS: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// 作品フォルダのルートを基準にした相対パス。
///
/// 常に検証済みの状態でしか存在できない（`new` を通らない限り作れない）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, type = "string"))]
pub struct RelPath(String);

/// `RelPath::new` が拒否した理由。
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    /// 空文字列だった。
    #[error("パスを指定してください")]
    Empty,
    /// `/` から始まる絶対パスだった。
    #[error("絶対パスは使えません: {0}")]
    Absolute(String),
    /// `C:` のようなドライブ指定を含んでいた。
    #[error("ドライブ指定は使えません: {0}")]
    DriveLetter(String),
    /// `\` を区切り文字として使っていた。
    #[error("区切り文字には '/' を使ってください（'\\' は使えません）: {0}")]
    BackslashSeparator(String),
    /// `a//b` のように空の要素を含んでいた。
    #[error("空の要素は使えません: {0}")]
    EmptySegment(String),
    /// `.` または `..` を要素として含んでいた。
    #[error("'.' や '..' は使えません: {0}")]
    DotSegment(String),
    /// 使えない文字を含んでいた。
    #[error("使えない文字 '{1}' が含まれています: {0}")]
    ForbiddenChar(String, char),
    /// Windows の予約名（CON, PRN, NUL, COM1 など）だった。
    #[error("Windows の予約名は使えません: {0}")]
    ReservedName(String),
    /// 要素の末尾がドットまたは空白だった。
    #[error("要素の末尾にドットや空白は使えません: {0}")]
    TrailingDotOrSpace(String),
    /// パス全体が長すぎた。
    #[error("パスが長すぎます（260 文字以内にしてください）: {0}")]
    TooLong(String),
}

impl RelPath {
    /// 文字列を検証して `RelPath` を作る。
    pub fn new(input: &str) -> Result<Self, PathError> {
        if input.is_empty() {
            return Err(PathError::Empty);
        }
        if input.chars().count() > MAX_LENGTH {
            return Err(PathError::TooLong(input.to_string()));
        }
        if input.contains('\\') {
            return Err(PathError::BackslashSeparator(input.to_string()));
        }
        if input.starts_with('/') {
            return Err(PathError::Absolute(input.to_string()));
        }
        if has_drive_letter(input) {
            return Err(PathError::DriveLetter(input.to_string()));
        }
        for segment in input.split('/') {
            validate_segment(input, segment)?;
        }
        Ok(Self(input.to_string()))
    }

    /// クレート内部で、検証済みであることが構成上保証されている文字列から作る。
    ///
    /// `layout` モジュールが検証済みの ID から組み立てるパスにのみ使う。
    pub(crate) fn trusted(input: String) -> Self {
        debug_assert!(
            RelPath::new(&input).is_ok(),
            "trusted な RelPath は検証済みでなければならない: {input}"
        );
        Self(input)
    }

    /// `/` 区切りの文字列表現。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 末尾に要素を追加した新しい `RelPath` を作る。
    pub fn join(&self, segment: &str) -> Result<Self, PathError> {
        Self::new(&format!("{}/{}", self.0, segment))
    }

    /// 親ディレクトリの `RelPath`。最上位（要素が一つ）の場合は `None`。
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        self.0
            .rsplit_once('/')
            .map(|(parent, _)| Self(parent.to_string()))
    }

    /// 最後の要素（ファイル名またはディレクトリ名）。
    #[must_use]
    pub fn file_name(&self) -> &str {
        match self.0.rsplit_once('/') {
            Some((_, name)) => name,
            None => &self.0,
        }
    }

    /// 拡張子を除いたファイル名。
    #[must_use]
    pub fn file_stem(&self) -> &str {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem,
            _ => name,
        }
    }

    /// 拡張子（先頭の `.` は含まない）。
    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => Some(ext),
            _ => None,
        }
    }
}

impl std::str::FromStr for RelPath {
    type Err = PathError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RelPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        RelPath::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// クレート内部向け: Windows の予約デバイス名かどうか（大小文字を無視）。
pub(crate) fn is_windows_reserved_name(stem: &str) -> bool {
    RESERVED_STEMS.contains(&stem.to_ascii_lowercase().as_str())
}

fn has_drive_letter(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn validate_segment(whole: &str, segment: &str) -> Result<(), PathError> {
    if segment.is_empty() {
        return Err(PathError::EmptySegment(whole.to_string()));
    }
    if segment == "." || segment == ".." {
        return Err(PathError::DotSegment(whole.to_string()));
    }
    if let Some(forbidden) = segment
        .chars()
        .find(|c| FORBIDDEN_CHARS.contains(c) || c.is_control())
    {
        return Err(PathError::ForbiddenChar(whole.to_string(), forbidden));
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err(PathError::TrailingDotOrSpace(whole.to_string()));
    }
    let stem = match segment.split_once('.') {
        Some((stem, _)) => stem,
        None => segment,
    };
    if is_windows_reserved_name(stem) {
        return Err(PathError::ReservedName(whole.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn accepts_ordinary_relative_paths() {
        let cases = [
            "kataribe.yaml",
            "world/overview.md",
            "plot/chapters/01.md",
            "characters/rin.md",
        ];
        for case in cases {
            assert!(RelPath::new(case).is_ok(), "should accept {case}");
        }
    }

    #[test]
    fn rejects_invalid_inputs() {
        let cases: &[(&str, &str)] = &[
            ("", "empty"),
            ("/etc/passwd", "absolute"),
            ("C:/windows", "drive letter"),
            ("a\\b", "backslash"),
            ("../secret", "dot-dot segment"),
            ("./a", "dot segment"),
            ("a/./b", "dot segment in middle"),
            ("a/../b", "dot-dot segment in middle"),
            ("a//b", "empty segment"),
            ("a/", "trailing slash makes empty segment"),
            ("a:b", "colon"),
            ("a*b", "asterisk"),
            ("a?b", "question mark"),
            ("a\"b", "quote"),
            ("a<b", "less than"),
            ("a>b", "greater than"),
            ("a|b", "pipe"),
            ("a\u{0}b", "control character"),
            ("CON", "reserved name CON"),
            ("con.md", "reserved name with extension"),
            ("COM1", "reserved name COM1"),
            ("lpt9.txt", "reserved name lpt9 with extension"),
            ("a.", "trailing dot"),
            ("a ", "trailing space"),
            ("world/a.", "trailing dot in nested segment"),
        ];
        for (case, description) in cases {
            assert!(
                RelPath::new(case).is_err(),
                "should reject {case} ({description})"
            );
        }
    }

    #[test]
    fn rejects_paths_longer_than_260_characters() {
        let long = "a".repeat(261);
        assert!(matches!(RelPath::new(&long), Err(PathError::TooLong(_))));
    }

    #[test]
    fn accepts_path_of_exactly_260_characters() {
        let long = format!("{}.md", "a".repeat(257));
        assert_eq!(long.chars().count(), 260);
        assert!(RelPath::new(&long).is_ok());
    }

    #[test]
    fn join_appends_and_validates() {
        let base = RelPath::new("characters").unwrap();
        let joined = base.join("rin.md").unwrap();
        assert_eq!(joined.as_str(), "characters/rin.md");

        let rejected = base.join("../escape");
        assert!(rejected.is_err());
    }

    #[test]
    fn parent_file_name_stem_and_extension() {
        let path = RelPath::new("plot/chapters/01.md").unwrap();
        assert_eq!(path.parent().unwrap().as_str(), "plot/chapters");
        assert_eq!(path.file_name(), "01.md");
        assert_eq!(path.file_stem(), "01");
        assert_eq!(path.extension(), Some("md"));

        let top_level = RelPath::new("kataribe.yaml").unwrap();
        assert_eq!(top_level.parent(), None);
    }

    #[test]
    fn dotfile_has_no_extension_and_stem_is_whole_name() {
        let path = RelPath::new(".kataribe/cache").unwrap();
        assert_eq!(path.file_name(), "cache");

        let dotfile = RelPath::new("a/.gitignore").unwrap();
        assert_eq!(dotfile.file_stem(), ".gitignore");
        assert_eq!(dotfile.extension(), None);
    }

    #[test]
    fn multi_dot_extension_splits_at_last_dot() {
        let path = RelPath::new("a/archive.tar.gz").unwrap();
        assert_eq!(path.file_stem(), "archive.tar");
        assert_eq!(path.extension(), Some("gz"));
    }

    #[test]
    fn serializes_and_deserializes_as_plain_string() {
        let path = RelPath::new("characters/rin.md").unwrap();
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"characters/rin.md\"");

        let back: RelPath = serde_json::from_str(&json).unwrap();
        assert_eq!(back, path);
    }

    #[test]
    fn deserialize_rejects_invalid_string() {
        let result: Result<RelPath, _> = serde_json::from_str("\"../escape\"");
        assert!(result.is_err());
    }

    #[test]
    fn dot_kataribe_paths_are_allowed() {
        assert!(RelPath::new(".kataribe/backups/characters/rin.md/20260101-000000-000.md").is_ok());
    }
}
