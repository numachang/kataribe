//! 画面へ返す、読んだ時点のハッシュを付けた内容（文書・作品の設定）。ハッシュは保存時の競合の検出に使う。

use kataribe_project::EditableDocument;
use serde::Serialize;

/// 画面へ返す、項目に分けた文書。[`kataribe_project::LoadedDocument`] の `hash` を文字列にしたもの。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DocumentFile {
    pub document: EditableDocument,
    /// 読み込んだ時点のファイル全体のハッシュ（16 進小文字）。保存時の競合検出に使う。
    pub hash: String,
    /// 人物資料・章立てなのに front matter を解釈できず、文字列として返したときの理由。
    pub parse_error: Option<String>,
}

/// 画面へ返す作品の設定。保存するときの競合の検出に使う、読んだ時点の `kataribe.yaml` のハッシュも付ける。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectSettingsFile {
    pub settings: kataribe_engine::ProjectSettings,
    /// 読み込んだ時点の `kataribe.yaml` のハッシュ（16 進小文字）。
    pub hash: String,
}

impl From<kataribe_project::LoadedDocument> for DocumentFile {
    fn from(file: kataribe_project::LoadedDocument) -> Self {
        Self {
            document: file.document,
            hash: file.hash.to_string(),
            parse_error: file.parse_error,
        }
    }
}
