//! クレート全体で使うエラー型。
//!
//! [`ProjectError`] の `Display` は、利用者にそのまま見せられる日本語にする。
//! 関係するパスは常にメッセージに含める。

use std::path::PathBuf;

use crate::frontmatter::YamlError;
use crate::model::ModelError;
use crate::path::{PathError, RelPath};

/// `kataribe-project` クレートで起こりうるエラー。
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// パスの検証に失敗した。
    #[error(transparent)]
    InvalidPath(#[from] PathError),

    /// id・番号の検証に失敗した。
    #[error(transparent)]
    InvalidId(#[from] ModelError),

    /// front matter または YAML の解析・生成に失敗した。
    #[error("{path} の {source}")]
    Frontmatter {
        /// 対象ファイル。
        path: RelPath,
        #[source]
        source: YamlError,
    },

    /// 書き込み条件（`WriteCondition`）に合わなかった。外部で変更された可能性がある。
    #[error("{path} は外部で変更されています。再読み込みしてください。")]
    Conflict {
        /// 対象ファイル。
        path: RelPath,
    },

    /// 人物資料・章立てとして保存しようとしたが、パスがその種類のファイルではなかった。
    #[error("{path} は{expected}のファイルではないため、{expected}として保存できません")]
    DocumentKindMismatch {
        /// 保存先のパス。
        path: RelPath,
        /// 保存しようとした文書の種類（「人物資料」など）。
        expected: &'static str,
    },

    /// ファイルが見つからなかった。
    #[error("{path} が見つかりません")]
    NotFound {
        /// 対象ファイル。
        path: RelPath,
    },

    /// 移動・改名の先に既にファイルがあった。
    #[error("{path} は既に存在します")]
    AlreadyExists {
        /// 対象ファイル。
        path: RelPath,
    },

    /// ファイルの内容が UTF-8 として読み込めなかった。
    #[error("{path} の内容が UTF-8 として読み込めません")]
    InvalidUtf8 {
        /// 対象ファイル。
        path: RelPath,
    },

    /// シンボリックリンクなどにより、実際のパスが作品フォルダの外を指していた。
    #[error("{path} は作品フォルダの外を指しています")]
    PathEscapesRoot {
        /// 対象パス。
        path: RelPath,
    },

    /// ファイル操作そのものが失敗した（権限・ディスク容量など）。
    #[error("{path} を読み書きできません: {source}")]
    Io {
        /// 対象ファイル。
        path: RelPath,
        #[source]
        source: std::io::Error,
    },

    /// 作品フォルダのルートを開けなかった。
    #[error("{} はフォルダとして開けません: {source}", root.display())]
    RootNotOpenable {
        /// 作品フォルダのルート。
        root: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// `kataribe.yaml` が無いため、作品フォルダとして開けなかった。
    #[error("{} は作品フォルダではありません（kataribe.yaml がありません）", root.display())]
    NotAProject {
        /// 開こうとしたフォルダ。
        root: PathBuf,
    },

    /// 対応していない形式バージョンだった。
    #[error(
        "この作品フォルダの形式（format: {found}）はサポートされていません。\
         対応している最大は {supported} です。アプリを更新してください。"
    )]
    UnsupportedFormat {
        /// ファイルに書かれていた形式バージョン。
        found: u32,
        /// このアプリが対応している最大の形式バージョン。
        supported: u32,
    },

    /// 複数のファイルをまとめて書く途中で失敗し、書き終えたファイルの一部を元に戻せなかった。
    #[error(
        "書き込みの途中で失敗し、{} を元に戻せませんでした（.kataribe/backups から戻せます）。原因: {source}",
        join_paths(not_restored)
    )]
    PartialWrite {
        /// 新しい内容のまま残ったファイル。
        not_restored: Vec<RelPath>,
        /// 途中で起きた失敗。
        #[source]
        source: Box<ProjectError>,
    },

    /// 作品を新規作成しようとしたフォルダが空ではなかった。
    #[error("{} が空ではないため、作品を作成できません", root.display())]
    DirectoryNotEmpty {
        /// 作成先のフォルダ。
        root: PathBuf,
    },
}

fn join_paths(paths: &[RelPath]) -> String {
    paths
        .iter()
        .map(RelPath::as_str)
        .collect::<Vec<_>>()
        .join("、")
}
