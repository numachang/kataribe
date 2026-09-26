//! ジャンルと年齢区分ごとの書き方の指針（`presets/genres.yaml`）。

use std::collections::BTreeMap;

use kataribe_project::Rating;
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};

const GENRES_YAML: &str = include_str!("../presets/genres.yaml");
const FALLBACK_GENRE_ID: &str = "general";

/// 画面の選択肢とプロンプトに使うジャンル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GenrePreset {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(skip_serializing)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub guidance: String,
}

impl GenrePreset {
    /// 特定のジャンルに寄せない「その他」か。
    pub fn is_general(&self) -> bool {
        self.id == FALLBACK_GENRE_ID
    }
}

#[derive(Debug, Deserialize)]
struct PresetFile {
    genres: Vec<GenrePreset>,
    ratings: BTreeMap<String, String>,
}

/// 組み込みのジャンル一覧。
#[derive(Debug)]
pub struct GenreCatalog {
    genres: Vec<GenrePreset>,
    rating_rules: BTreeMap<String, String>,
}

impl GenreCatalog {
    /// ビルド時に埋め込んだプリセットを読み込む。
    pub fn builtin() -> Result<Self> {
        let file: PresetFile = serde_saphyr::from_str(GENRES_YAML)
            .map_err(|error| EngineError::Preset(error.to_string()))?;
        Ok(Self {
            genres: file.genres,
            rating_rules: file.ratings,
        })
    }

    /// 画面の選択肢に出す順のジャンル一覧。
    pub fn genres(&self) -> &[GenrePreset] {
        &self.genres
    }

    /// id に対応するジャンル。未知の id なら「その他」を返す（利用者が YAML を手で書き換えても動くように）。
    pub fn resolve(&self, id: &str) -> Option<&GenrePreset> {
        self.genres.iter().find(|genre| genre.id == id).or_else(|| {
            self.genres
                .iter()
                .find(|genre| genre.id == FALLBACK_GENRE_ID)
        })
    }

    /// 年齢区分の規則（プロンプトにそのまま入る文）。
    pub fn rating_rule(&self, rating: Rating) -> &str {
        self.rating_rules
            .get(rating_key(rating))
            .map_or("", String::as_str)
    }
}

pub fn rating_label(rating: Rating) -> &'static str {
    match rating {
        Rating::General => "全年齢",
        Rating::R15 => "R15",
        Rating::R18 => "R18（成人向け）",
    }
}

fn rating_key(rating: Rating) -> &'static str {
    match rating {
        Rating::General => "general",
        Rating::R15 => "r15",
        Rating::R18 => "r18",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_presets_parse_and_are_complete() {
        let file: PresetFile = serde_saphyr::from_str(GENRES_YAML).unwrap();

        assert!(file.genres.len() >= 10);
        for genre in &file.genres {
            assert!(!genre.label.is_empty(), "{} に label がない", genre.id);
            assert!(
                !genre.guidance.trim().is_empty(),
                "{} に guidance がない",
                genre.id
            );
        }
        for key in ["general", "r15", "r18"] {
            assert!(
                file.ratings.contains_key(key),
                "年齢区分 {key} の規則がない"
            );
        }
    }

    #[test]
    fn genre_ids_are_unique() {
        let catalog = GenreCatalog::builtin().unwrap();
        let mut ids: Vec<_> = catalog.genres().iter().map(|genre| &genre.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), catalog.genres().len());
    }

    #[test]
    fn unknown_genre_falls_back_to_general() {
        let catalog = GenreCatalog::builtin().unwrap();
        assert_eq!(catalog.resolve("no-such-genre").unwrap().id, "general");
        assert_eq!(catalog.resolve("mystery").unwrap().label, "ミステリ");
    }

    #[test]
    fn adult_rating_requires_adult_characters() {
        let catalog = GenreCatalog::builtin().unwrap();
        assert!(catalog.rating_rule(Rating::R18).contains("18 歳以上"));
    }

    #[test]
    fn every_rating_forbids_sexualizing_minors() {
        let catalog = GenreCatalog::builtin().unwrap();
        for rating in [Rating::General, Rating::R15, Rating::R18] {
            assert!(
                catalog
                    .rating_rule(rating)
                    .contains("未成年を性的に描写しない"),
                "{rating:?}"
            );
        }
    }
}
