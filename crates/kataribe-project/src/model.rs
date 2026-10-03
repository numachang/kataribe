//! 作品フォルダの各ファイルに対応する型。
//!
//! `render()` はファイルの内容（テキスト）を生成する。解析は [`crate::project::Project`] が行う。

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::frontmatter::{self, Document, YamlError};
use crate::slug;

/// 現在サポートしている作品フォルダの形式バージョン。
pub const FORMAT_VERSION: u32 = 1;

/// 年齢区分。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    /// 全年齢。
    #[default]
    General,
    /// 15 才以上推奨。
    R15,
    /// 18 才以上推奨。
    R18,
}

/// `kataribe.yaml`（作品情報）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct Manifest {
    /// 作品フォルダの形式バージョン。
    pub format: u32,
    /// 題名。
    pub title: String,
    /// 著者名（任意）。
    pub author: Option<String>,
    /// ジャンルプリセットの id。
    pub genre: String,
    /// ジャンルの補足（任意）。
    pub genre_note: Option<String>,
    /// 年齢区分。
    pub rating: Rating,
    /// 目標総文字数。
    pub target_length: u32,
    /// 企画の種。最初に LLM へ渡す指示。
    pub idea: String,
    /// 作品ごとの設定（LLM・生成のしかた）。形は執筆エンジン（kataribe-engine）が決めるので、
    /// ここでは解釈せずにそのまま持つ。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub settings: Option<serde_json::Value>,
    /// 利用者が追加した未知の項目。書き戻すときも保持する。
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Manifest {
    /// `kataribe.yaml` の内容を解析する。front matter は伴わない単独の YAML。
    pub fn parse(text: &str) -> Result<Self, YamlError> {
        frontmatter::parse_yaml(text)
    }

    /// `kataribe.yaml` に書き出す内容を生成する。
    pub fn render(&self) -> Result<String, YamlError> {
        frontmatter::render_yaml(self)
    }
}

/// front matter が任意の Markdown 文書に共通するメタデータ。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DocMeta {
    /// 表示用の題名（任意）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 利用者が追加した未知の項目。
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// front matter が任意の Markdown 文書（`world/*.md` など）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarkdownDoc {
    /// front matter のメタデータ。
    pub meta: DocMeta,
    /// 本文。
    pub body: String,
}

impl MarkdownDoc {
    /// テキストを解析する。front matter は任意（無ければ `meta` は既定値になる）。
    pub fn parse(text: &str) -> Result<Self, YamlError> {
        let document: Document<DocMeta> = frontmatter::parse_optional(text)?;
        Ok(Self {
            meta: document.meta,
            body: document.body,
        })
    }

    /// 表示名を決める: `meta.title` → 本文の最初の `# 見出し` → `fallback` の順。
    #[must_use]
    pub fn display_title(&self, fallback: &str) -> String {
        if let Some(title) = &self.meta.title {
            let trimmed = title.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(heading) = first_heading(&self.body) {
            return heading;
        }
        fallback.to_string()
    }

    /// ファイルに書き出す内容を生成する。
    ///
    /// メタデータが空（`title` が無く未知の項目も無い）なら front matter を省略し、
    /// 素の Markdown として書き出す。
    pub fn render(&self) -> Result<String, YamlError> {
        if self.meta.title.is_none() && self.meta.extra.is_empty() {
            return Ok(self.body.clone());
        }
        frontmatter::render(&Document {
            meta: self.meta.clone(),
            body: self.body.clone(),
        })
    }
}

fn first_heading(body: &str) -> Option<String> {
    for line in body.lines() {
        if let Some(rest) = line.trim().strip_prefix("# ") {
            let heading = rest.trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
        }
    }
    None
}

/// id・番号の検証に失敗したときのエラー。
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ModelError {
    /// 登場人物 id が規則（小文字英数字とハイフン、48 文字以内）に合わない。
    #[error("登場人物 ID は英小文字・数字・ハイフンのみ、48 文字以内にしてください: {0}")]
    InvalidCharacterId(String),
    /// 世界観の資料の名前が規則（小文字英数字とハイフン、48 文字以内。`overview` は不可）に合わない。
    #[error(
        "世界観の資料のファイル名は英小文字・数字・ハイフンのみ、48 文字以内にしてください（overview は使えません）: {0}"
    )]
    InvalidWorldDocumentName(String),
    /// 章番号が 2〜3 桁の数字になっていない。
    #[error("章番号は 2〜3 桁の数字にしてください: {0}")]
    InvalidChapterId(String),
    /// シーン id が `s` + 2〜3 桁の数字になっていない。
    #[error("シーン ID は 's' に続く 2〜3 桁の数字にしてください: {0}")]
    InvalidSceneId(String),
}

/// 登場人物の id（ローマ字の slug）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, type = "string"))]
pub struct CharacterId(String);

impl CharacterId {
    /// 文字列を検証して `CharacterId` を作る。
    pub fn new(input: &str) -> Result<Self, ModelError> {
        if slug::is_valid(input) {
            Ok(Self(input.to_string()))
        } else {
            Err(ModelError::InvalidCharacterId(input.to_string()))
        }
    }

    /// 任意の文字列（LLM が出したローマ字など）から id を作る。
    ///
    /// 小文字化し、英数字以外の並びをハイフン一つに正規化し、端のハイフンと長さを整える。
    /// 結果が空になる場合は `"character"`、`taken` に既に含まれる場合は `-2`, `-3`, … を付ける。
    #[must_use]
    pub fn from_hint(hint: &str, taken: &[CharacterId]) -> Self {
        Self(slug::from_hint(hint, "character", |candidate| {
            taken.iter().any(|id| id.0 == candidate)
        }))
    }

    /// `/` 区切りパスの一要素として使える文字列表現。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for CharacterId {
    type Err = ModelError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl fmt::Display for CharacterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CharacterId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CharacterId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        CharacterId::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// 世界観の資料（`world/<name>.md`）のファイル名。`overview` は世界観の概要（`world/overview.md`）の
/// ものなので使えない。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorldDocumentName(String);

impl WorldDocumentName {
    /// `world/overview.md`（世界観の概要）の名前。[`crate::layout::WORLD_OVERVIEW`] と一致させること。
    const RESERVED: &'static str = "overview";

    /// 文字列を検証して `WorldDocumentName` を作る。規則は人物の id と同じ（小文字英数字とハイフン、
    /// 48 文字以内）で、さらに概要の名前は使えない。
    pub fn new(input: &str) -> Result<Self, ModelError> {
        if slug::is_valid(input) && input != Self::RESERVED {
            Ok(Self(input.to_string()))
        } else {
            Err(ModelError::InvalidWorldDocumentName(input.to_string()))
        }
    }

    /// 任意の文字列（題をローマ字にしたものなど）から名前を作る。
    ///
    /// 人物の id と同じ規則で整える。結果が空になる場合は `"doc"`、概要の名前や `taken` に既に含まれる場合は
    /// `-2`, `-3`, … を付ける。
    #[must_use]
    pub fn from_hint(hint: &str, taken: &[WorldDocumentName]) -> Self {
        Self(slug::from_hint(hint, "doc", |candidate| {
            candidate == Self::RESERVED || taken.iter().any(|name| name.0 == candidate)
        }))
    }

    /// `/` 区切りパスの一要素として使える文字列表現。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorldDocumentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 章の id の番号の上限（桁数が 3 桁までのため）。
const MAX_ID_NUMBER: u32 = 999;

/// 章の id（`01`, `02`, … `999` まで）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, type = "string"))]
pub struct ChapterId {
    number: u16,
    width: u8,
}

impl ChapterId {
    /// `NN` または `NNN` 形式の文字列を検証して作る。
    pub fn new(input: &str) -> Result<Self, ModelError> {
        parse_numeric_id(input)
            .map(|(number, width)| Self { number, width })
            .ok_or_else(|| ModelError::InvalidChapterId(input.to_string()))
    }

    /// 数値から作る。2 桁ゼロ埋め（100 以上は 3 桁になる）。
    #[must_use]
    pub fn from_number(number: u32) -> Self {
        let width = if number >= 100 { 3 } else { 2 };
        Self {
            number: u16::try_from(number).unwrap_or(u16::MAX),
            width,
        }
    }

    /// 章番号。
    #[must_use]
    pub fn number(&self) -> u32 {
        u32::from(self.number)
    }

    /// 番号を `delta` だけずらした id。章を途中に足す・消すときの、番号の振り直しに使う。
    ///
    /// 桁数は [`ChapterId::from_number`] の規則で付け直す（`001` のように手で付けた 3 桁も、
    /// ずらした章だけが 2 桁になる。並び順は番号で決まるので崩れない）。
    /// 0 未満、または 999 を超えるときは `None`。
    #[must_use]
    pub fn shifted(&self, delta: i32) -> Option<Self> {
        let shifted = i32::from(self.number).checked_add(delta)?;
        let number = u32::try_from(shifted).ok()?;
        (number <= MAX_ID_NUMBER).then(|| Self::from_number(number))
    }
}

impl std::str::FromStr for ChapterId {
    type Err = ModelError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl fmt::Display for ChapterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:0width$}",
            self.number,
            width = usize::from(self.width)
        )
    }
}

impl PartialOrd for ChapterId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChapterId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // `Eq` は `number` と `width` の両方を見るため、`Ord` も両方を見て一致させる
        // （例: "01" と "001" は番号が同じでも `width` が違うので等しくない）。
        self.number
            .cmp(&other.number)
            .then_with(|| self.width.cmp(&other.width))
    }
}

impl Serialize for ChapterId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ChapterId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ChapterId::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// シーンの id（`s01`, `s02`, … `s999` まで）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, type = "string"))]
pub struct SceneId {
    number: u16,
    width: u8,
}

impl SceneId {
    /// `sNN` または `sNNN` 形式の文字列を検証して作る。
    pub fn new(input: &str) -> Result<Self, ModelError> {
        let Some(rest) = input.strip_prefix('s') else {
            return Err(ModelError::InvalidSceneId(input.to_string()));
        };
        parse_numeric_id(rest)
            .map(|(number, width)| Self { number, width })
            .ok_or_else(|| ModelError::InvalidSceneId(input.to_string()))
    }

    /// 数値から作る。2 桁ゼロ埋め（100 以上は 3 桁になる）。
    #[must_use]
    pub fn from_number(number: u32) -> Self {
        let width = if number >= 100 { 3 } else { 2 };
        Self {
            number: u16::try_from(number).unwrap_or(u16::MAX),
            width,
        }
    }

    /// シーン番号。
    #[must_use]
    pub fn number(&self) -> u32 {
        u32::from(self.number)
    }

    /// `existing` のどの id とも番号が重ならない、最小の番号を持つシーン id を返す。
    ///
    /// 桁数（`s01` と `s001` のような表記の違い）ではなく、番号そのもので重複を判定する。
    #[must_use]
    pub fn next_available(existing: &[SceneId]) -> SceneId {
        let mut candidate_number = 1u32;
        loop {
            let already_used = existing
                .iter()
                .any(|scene| scene.number() == candidate_number);
            if !already_used {
                return SceneId::from_number(candidate_number);
            }
            candidate_number += 1;
        }
    }
}

impl std::str::FromStr for SceneId {
    type Err = ModelError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl fmt::Display for SceneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "s{:0width$}",
            self.number,
            width = usize::from(self.width)
        )
    }
}

impl PartialOrd for SceneId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SceneId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // `Eq` は `number` と `width` の両方を見るため、`Ord` も両方を見て一致させる
        // （例: "s01" と "s001" は番号が同じでも `width` が違うので等しくない）。
        self.number
            .cmp(&other.number)
            .then_with(|| self.width.cmp(&other.width))
    }
}

impl Serialize for SceneId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SceneId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        SceneId::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// 2〜3 桁の数字だけからなる文字列を `(数値, 桁数)` に変換する。
fn parse_numeric_id(input: &str) -> Option<(u16, u8)> {
    let len = input.len();
    if !(2..=3).contains(&len) || !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: u16 = input.parse().ok()?;
    let width = u8::try_from(len).unwrap_or(2);
    Some((number, width))
}

/// `characters/<id>.md` の front matter。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CharacterMeta {
    /// 表示名。
    pub name: String,
    /// 読み仮名（任意）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    /// 役回り（主人公、探偵、など）。
    #[serde(default)]
    pub role: String,
    /// 一言紹介。
    #[serde(default)]
    pub summary: String,
    /// 表示順（任意）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    /// 利用者が追加した未知の項目。
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// 登場人物（`characters/<id>.md` 一つ分）。
#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    /// id（ファイル名の元になる）。
    pub id: CharacterId,
    /// front matter のメタデータ。
    pub meta: CharacterMeta,
    /// 本文。
    pub body: String,
}

impl Character {
    /// front matter 付きのテキストを解析する。
    pub fn parse(id: CharacterId, text: &str) -> Result<Self, YamlError> {
        let document: Document<CharacterMeta> = frontmatter::parse(text)?;
        Ok(Self {
            id,
            meta: document.meta,
            body: document.body,
        })
    }

    /// ファイルに書き出す内容を生成する。
    pub fn render(&self) -> Result<String, YamlError> {
        frontmatter::render(&Document {
            meta: self.meta.clone(),
            body: self.body.clone(),
        })
    }
}

/// 1 シーンぶんの設計（`plot/chapters/<NN>.md` の `scenes` の要素）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ScenePlan {
    /// シーン id。本文ファイル名にも使う。
    pub id: SceneId,
    /// シーン題。
    pub title: String,
    /// このシーンの要約。
    pub summary: String,
    /// 視点人物（任意）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pov: Option<String>,
    /// 登場人物名の一覧。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    /// 場所（任意）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// 時間（任意）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// 目標文字数（任意）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_chars: Option<u32>,
    /// ビート単位で生成したときの展開の一覧。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beats: Vec<String>,
    /// 利用者が追加した未知の項目。
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// `plot/chapters/<NN>.md` の front matter。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ChapterMeta {
    /// 章題。
    pub title: String,
    /// シーン構成。並び順がそのままシーンの順序になる。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<ScenePlan>,
    /// 利用者が追加した未知の項目。
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// 章（`plot/chapters/<NN>.md` 一つ分）。
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    /// 章 id（ファイル名の元になる）。
    pub id: ChapterId,
    /// front matter のメタデータ。
    pub meta: ChapterMeta,
    /// この章のストーリーライン（本文）。
    pub storyline: String,
}

impl Chapter {
    /// front matter 付きのテキストを解析する。
    pub fn parse(id: ChapterId, text: &str) -> Result<Self, YamlError> {
        let document: Document<ChapterMeta> = frontmatter::parse(text)?;
        Ok(Self {
            id,
            meta: document.meta,
            storyline: document.body,
        })
    }

    /// ファイルに書き出す内容を生成する。
    pub fn render(&self) -> Result<String, YamlError> {
        frontmatter::render(&Document {
            meta: self.meta.clone(),
            body: self.storyline.clone(),
        })
    }

    /// id に一致するシーン計画を探す。
    #[must_use]
    pub fn scene(&self, id: &SceneId) -> Option<&ScenePlan> {
        self.meta.scenes.iter().find(|scene| &scene.id == id)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn rating_serializes_as_lowercase_string() {
        assert_eq!(
            serde_json::to_string(&Rating::General).unwrap(),
            "\"general\""
        );
        assert_eq!(serde_json::to_string(&Rating::R15).unwrap(), "\"r15\"");
        assert_eq!(serde_json::to_string(&Rating::R18).unwrap(), "\"r18\"");
        assert_eq!(Rating::default(), Rating::General);
    }

    #[test]
    fn character_id_accepts_valid_slugs_and_rejects_invalid_ones() {
        assert!(CharacterId::new("kirishima-rin").is_ok());
        assert!(CharacterId::new("rin2").is_ok());
        assert!(CharacterId::new("").is_err());
        assert!(CharacterId::new("-rin").is_err());
        assert!(CharacterId::new("rin-").is_err());
        assert!(CharacterId::new("rin--kirishima").is_err());
        assert!(CharacterId::new("Rin").is_err());
        assert!(CharacterId::new("con").is_err());
        assert!(CharacterId::new(&"a".repeat(49)).is_err());
    }

    #[test]
    fn character_id_from_hint_normalizes_arbitrary_strings() {
        let id = CharacterId::from_hint("Kirishima Rin!!", &[]);
        assert_eq!(id.as_str(), "kirishima-rin");

        let id = CharacterId::from_hint("霧島 凛", &[]);
        assert_eq!(id.as_str(), "character");

        let id = CharacterId::from_hint("   ", &[]);
        assert_eq!(id.as_str(), "character");
    }

    #[test]
    fn character_id_from_hint_avoids_reserved_names() {
        let id = CharacterId::from_hint("CON", &[]);
        assert_ne!(id.as_str(), "con");
    }

    #[test]
    fn character_id_from_hint_disambiguates_against_taken() {
        let first = CharacterId::new("rin").unwrap();
        let second = CharacterId::from_hint("Rin", std::slice::from_ref(&first));
        assert_eq!(second.as_str(), "rin-2");

        let third = CharacterId::from_hint("Rin", &[first, second]);
        assert_eq!(third.as_str(), "rin-3");
    }

    #[test]
    fn world_document_name_accepts_slugs_and_rejects_the_overview_and_invalid_names() {
        assert!(WorldDocumentName::new("glossary").is_ok());
        assert!(WorldDocumentName::new("city-map-2").is_ok());
        assert!(WorldDocumentName::new("overview").is_err());
        assert!(WorldDocumentName::new("").is_err());
        assert!(WorldDocumentName::new("Glossary").is_err());
        assert!(WorldDocumentName::new("用語集").is_err());
        assert!(WorldDocumentName::new("con").is_err());
        assert!(WorldDocumentName::new(&"a".repeat(49)).is_err());
    }

    #[test]
    fn world_document_name_from_hint_falls_back_to_doc_and_numbers_duplicates() {
        assert_eq!(
            WorldDocumentName::from_hint("City Map", &[]).as_str(),
            "city-map"
        );
        assert_eq!(WorldDocumentName::from_hint("", &[]).as_str(), "doc");
        assert_eq!(WorldDocumentName::from_hint("漢字", &[]).as_str(), "doc");

        let first = WorldDocumentName::from_hint("", &[]);
        let second = WorldDocumentName::from_hint("", std::slice::from_ref(&first));
        let third = WorldDocumentName::from_hint("", &[first, second.clone()]);
        assert_eq!(second.as_str(), "doc-2");
        assert_eq!(third.as_str(), "doc-3");
    }

    #[test]
    fn world_document_name_from_hint_never_returns_the_overview() {
        assert_eq!(
            WorldDocumentName::from_hint("Overview", &[]).as_str(),
            "overview-2"
        );
    }

    #[test]
    fn world_document_name_from_hint_keeps_the_numbered_name_within_the_length_limit() {
        let long = "a".repeat(60);
        let first = WorldDocumentName::from_hint(&long, &[]);
        let second = WorldDocumentName::from_hint(&long, std::slice::from_ref(&first));
        assert_eq!(first.as_str().len(), 48);
        assert!(second.as_str().len() <= 48);
        assert!(second.as_str().ends_with("-2"));
        assert!(WorldDocumentName::new(second.as_str()).is_ok());
    }

    #[test]
    fn chapter_id_from_number_zero_pads_to_two_digits() {
        assert_eq!(ChapterId::from_number(1).to_string(), "01");
        assert_eq!(ChapterId::from_number(9).to_string(), "09");
        assert_eq!(ChapterId::from_number(42).to_string(), "42");
        assert_eq!(ChapterId::from_number(100).to_string(), "100");
    }

    #[test]
    fn chapter_id_parses_two_and_three_digit_strings() {
        assert_eq!(ChapterId::new("01").unwrap().number(), 1);
        assert_eq!(ChapterId::new("100").unwrap().number(), 100);
        assert!(ChapterId::new("1").is_err());
        assert!(ChapterId::new("1000").is_err());
        assert!(ChapterId::new("ab").is_err());
    }

    #[test]
    fn chapter_id_shifted_renumbers_with_the_width_rule_of_from_number() {
        let shift = |id: &str, delta: i32| {
            ChapterId::new(id)
                .unwrap()
                .shifted(delta)
                .map(|shifted| shifted.to_string())
        };
        assert_eq!(shift("01", 1), Some("02".to_owned()));
        assert_eq!(shift("09", 1), Some("10".to_owned()));
        assert_eq!(shift("99", 1), Some("100".to_owned()));
        assert_eq!(shift("100", -1), Some("99".to_owned()));
        assert_eq!(shift("02", -1), Some("01".to_owned()));
        assert_eq!(shift("998", 1), Some("999".to_owned()));
        assert_eq!(shift("05", 0), Some("05".to_owned()));
    }

    #[test]
    fn chapter_id_shifted_is_none_outside_zero_to_999() {
        let shift = |id: &str, delta: i32| ChapterId::new(id).unwrap().shifted(delta);
        assert_eq!(shift("999", 1), None);
        assert_eq!(shift("00", -1), None);
        assert_eq!(shift("01", -2), None);
        assert_eq!(shift("500", i32::MAX), None);
        assert_eq!(shift("500", i32::MIN), None);
    }

    #[test]
    fn chapter_id_shifted_renumbers_a_hand_made_three_digit_id_with_two_digits() {
        let shifted = ChapterId::new("001").unwrap().shifted(1).unwrap();
        assert_eq!(shifted.to_string(), "02");
    }

    #[test]
    fn chapter_id_orders_numerically_not_lexicographically() {
        let mut ids = [
            ChapterId::new("100").unwrap(),
            ChapterId::new("02").unwrap(),
            ChapterId::new("09").unwrap(),
        ];
        ids.sort();
        let rendered: Vec<String> = ids.iter().map(ToString::to_string).collect();
        assert_eq!(rendered, vec!["02", "09", "100"]);
    }

    #[test]
    fn scene_id_next_available_fills_gaps() {
        let existing = vec![SceneId::new("s01").unwrap(), SceneId::new("s02").unwrap()];
        assert_eq!(SceneId::next_available(&existing).to_string(), "s03");

        let with_gap = vec![SceneId::new("s01").unwrap(), SceneId::new("s03").unwrap()];
        assert_eq!(SceneId::next_available(&with_gap).to_string(), "s02");
    }

    #[test]
    fn scene_id_next_available_treats_different_width_as_the_same_number() {
        // "s001" は桁数こそ違うが番号としては 1 と同じなので、"s01" は使用済み扱いになるはず。
        let existing = vec![SceneId::new("s001").unwrap()];
        assert_eq!(SceneId::next_available(&existing).to_string(), "s02");
    }

    #[test]
    fn scene_id_eq_and_ord_agree_on_different_width_same_number() {
        let short = SceneId::new("s01").unwrap();
        let long = SceneId::new("s001").unwrap();

        assert_ne!(short, long, "桁数が違うので等しくないはず");
        assert_ne!(
            short.cmp(&long),
            std::cmp::Ordering::Equal,
            "Eq で等しくないなら cmp も Equal を返してはいけない"
        );
        assert_eq!(short.cmp(&long), std::cmp::Ordering::Less);
    }

    #[test]
    fn chapter_id_eq_and_ord_agree_on_different_width_same_number() {
        let short = ChapterId::new("01").unwrap();
        let long = ChapterId::new("001").unwrap();

        assert_ne!(short, long, "桁数が違うので等しくないはず");
        assert_ne!(
            short.cmp(&long),
            std::cmp::Ordering::Equal,
            "Eq で等しくないなら cmp も Equal を返してはいけない"
        );
        assert_eq!(short.cmp(&long), std::cmp::Ordering::Less);
    }

    #[test]
    fn character_meta_defaults_role_and_summary_when_missing() {
        let yaml = "name: 霧島 凛\n";
        let meta: CharacterMeta = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(meta.name, "霧島 凛");
        assert_eq!(meta.role, "");
        assert_eq!(meta.summary, "");
    }

    #[test]
    fn scene_plan_omits_empty_and_none_fields_when_serialized() {
        let plan = ScenePlan {
            id: SceneId::new("s01").unwrap(),
            title: "事務所に届いた依頼".to_string(),
            summary: "雨の夜、凛の事務所に…".to_string(),
            pov: None,
            characters: vec![],
            place: None,
            time: None,
            target_chars: None,
            beats: vec![],
            extra: BTreeMap::new(),
        };
        let yaml = serde_saphyr::to_string(&plan).unwrap();
        assert!(!yaml.contains("pov"), "yaml was: {yaml}");
        assert!(!yaml.contains("characters"), "yaml was: {yaml}");
        assert!(!yaml.contains("beats"), "yaml was: {yaml}");
    }

    #[test]
    fn markdown_doc_display_title_falls_back_through_heading_then_default() {
        let with_title = MarkdownDoc {
            meta: DocMeta {
                title: Some("世界観".to_string()),
                extra: BTreeMap::new(),
            },
            body: "本文".to_string(),
        };
        assert_eq!(with_title.display_title("既定"), "世界観");

        let with_heading = MarkdownDoc {
            meta: DocMeta::default(),
            body: "# 見出しの題名\n本文".to_string(),
        };
        assert_eq!(with_heading.display_title("既定"), "見出しの題名");

        let with_neither = MarkdownDoc {
            meta: DocMeta::default(),
            body: "本文のみ".to_string(),
        };
        assert_eq!(with_neither.display_title("既定"), "既定");
    }

    #[test]
    fn markdown_doc_render_omits_front_matter_when_meta_is_empty() {
        let doc = MarkdownDoc {
            meta: DocMeta::default(),
            body: "素の本文\n".to_string(),
        };
        assert_eq!(doc.render().unwrap(), "素の本文\n");
    }

    #[test]
    fn manifest_round_trips_through_render_and_parse() {
        let mut extra = BTreeMap::new();
        extra.insert("custom".to_string(), serde_json::json!(true));
        let manifest = Manifest {
            format: FORMAT_VERSION,
            title: "みさき館の殺人".to_string(),
            author: Some("沼田".to_string()),
            genre: "mystery".to_string(),
            genre_note: Some("館もの。本格".to_string()),
            rating: Rating::General,
            target_length: 30_000,
            idea: "嵐で孤立した岬の洋館で…".to_string(),
            settings: Some(
                serde_json::json!({ "provider": "claude_code", "context_tokens": 100_000 }),
            ),
            extra,
        };
        let rendered = manifest.render().unwrap();
        let parsed = Manifest::parse(&rendered).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn a_manifest_without_settings_does_not_write_the_settings_key() {
        let text =
            "format: 1\ntitle: t\ngenre: general\nrating: general\ntarget_length: 1000\nidea: i\n";

        let manifest = Manifest::parse(text).unwrap();

        assert_eq!(manifest.settings, None);
        assert!(!manifest.render().unwrap().contains("settings"));
    }

    #[test]
    fn character_parse_then_render_round_trips() {
        let text = "---\nname: 霧島 凛\nrole: 主人公\nsummary: 盲目の少女探偵。\n---\n## 外見\n…\n";
        let id = CharacterId::new("kirishima-rin").unwrap();
        let character = Character::parse(id.clone(), text).unwrap();

        assert_eq!(character.id, id);
        assert_eq!(character.meta.name, "霧島 凛");
        assert_eq!(character.body, "## 外見\n…\n");
        assert_eq!(character.render().unwrap(), text);
    }

    #[test]
    fn chapter_parse_then_render_round_trips() {
        let text = "---\ntitle: 雨の匂い\n---\nこの章のストーリーライン\n";
        let id = ChapterId::from_number(1);
        let chapter = Chapter::parse(id, text).unwrap();

        assert_eq!(chapter.id, id);
        assert_eq!(chapter.meta.title, "雨の匂い");
        assert_eq!(chapter.storyline, "この章のストーリーライン\n");
        assert_eq!(chapter.render().unwrap(), text);
    }

    #[test]
    fn markdown_doc_parse_allows_missing_front_matter() {
        let without_front_matter = "# 世界観\n本文\n";
        let doc = MarkdownDoc::parse(without_front_matter).unwrap();
        assert_eq!(doc.meta, DocMeta::default());
        assert_eq!(doc.body, without_front_matter);

        let with_front_matter = "---\ntitle: 世界観\n---\n本文\n";
        let doc = MarkdownDoc::parse(with_front_matter).unwrap();
        assert_eq!(doc.meta.title.as_deref(), Some("世界観"));
        assert_eq!(doc.body, "本文\n");
    }

    #[test]
    fn character_render_snapshot_uses_block_scalar_for_multiline_japanese_text() {
        let character = Character {
            id: CharacterId::new("kirishima-rin").unwrap(),
            meta: CharacterMeta {
                name: "霧島 凛".to_string(),
                reading: Some("きりしま りん".to_string()),
                role: "主人公".to_string(),
                summary: "盲目の少女探偵。\n声の揺れで嘘を聞き分ける。".to_string(),
                order: Some(1),
                extra: BTreeMap::new(),
            },
            body: "## 外見\n…\n## 口調\n一人称は「わたし」。…\n".to_string(),
        };

        insta::assert_snapshot!(character.render().unwrap());
    }
}
