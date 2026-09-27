//! 画面向けのテキストファイル表現。

use serde::Serialize;

/// 画面へ返すテキストファイル。[`kataribe_project::TextFile`] の `hash` を文字列にしたもの。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TextFile {
    pub content: String,
    /// 読み込んだ時点の内容のハッシュ（16 進小文字）。上書き時の競合検出に使う。
    pub hash: String,
}

/// 画面へ返す作品の設定。保存するときの競合の検出に使う、読んだ時点の `kataribe.yaml` のハッシュも付ける。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectSettingsFile {
    pub settings: kataribe_engine::ProjectSettings,
    /// 読み込んだ時点の `kataribe.yaml` のハッシュ（16 進小文字）。
    pub hash: String,
}

impl From<kataribe_project::TextFile> for TextFile {
    fn from(file: kataribe_project::TextFile) -> Self {
        Self {
            content: file.content,
            hash: file.hash.to_string(),
        }
    }
}
