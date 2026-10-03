//! クレート全体で使うエラー型。
//!
//! [`ProjectError`] の `Display` は、利用者にそのまま見せられる日本語にする。
//! 関係するパスは常にメッセージに含める。

use std::path::PathBuf;

use crate::frontmatter::YamlError;
use crate::model::{ModelError, SceneId};
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

    /// 章立ての `scenes` に、同じ `id` のシーンが複数あった。
    ///
    /// 本文ファイルの名前がシーンの `id` なので、重複したまま項目に分けて保存すると別のシーンを上書きしてしまう。
    #[error(
        "{path} のシーンの id「{id}」が重複しているため、項目に分けて扱えません。id を直してください。"
    )]
    DuplicateSceneId {
        /// 対象ファイル。
        path: RelPath,
        /// 重複していた id。
        id: SceneId,
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

    /// まとめて反映する変更の形が正しくなかった（ゴミ箱へ移せない・動かせないパス、同じパスへの変更の重なりなど）。
    ///
    /// 画面から戻ってくる値なので、反映の前に必ず検証する。何も変えていない。
    #[error("変更案が正しくありません: {reason}")]
    InvalidChangeSet {
        /// 正しくない理由。
        reason: String,
    },

    /// 複数のファイルをまとめて反映する途中で失敗し、反映済みの変更の一部を元に戻せなかった。
    #[error(
        "反映の途中で失敗し、元に戻せなかったファイルがあります。{} 原因: {source}",
        describe_unrestored(not_restored, still_trashed, still_moved, journal.as_ref())
    )]
    PartialWrite {
        /// 書き込み前の内容に戻せず、新しい内容のまま残ったファイル。
        not_restored: Vec<RelPath>,
        /// ゴミ箱から元の場所へ戻せなかったファイル。実物はゴミ箱の中にしかない
        /// （ゴミ箱へ移すファイルはバックアップを取らない）。
        still_trashed: Vec<StillTrashed>,
        /// 改名（移動）の途中で止まり、元の場所へ戻せなかったもの。実物は `current` にある。
        still_moved: Vec<StillMoved>,
        /// 反映の前に書いた操作の記録（`.kataribe/staging/<日時>/journal.json`）。
        /// 移動やゴミ箱を伴う反映にだけあり、戻せなかったものを人が見て戻すときの手がかりになる。
        journal: Option<RelPath>,
        /// 途中で起きた失敗。
        #[source]
        source: Box<ProjectError>,
    },

    /// 改名（移動）で、ほかのアプリがファイルを開いているために移せなかった。
    ///
    /// Windows では、中のファイルを開かれたフォルダは改名できない。閉じてもらえば再試行できる。
    #[error(
        "{path} を移せませんでした。ほかのアプリが {path} かその中のファイルを開いている可能性があります。{}閉じてからもう一度試してください。",
        if *restored { "ここまでの変更はすべて元に戻しました。" } else { "" }
    )]
    FilesInUse {
        /// 移せなかったファイルまたはフォルダ。
        path: RelPath,
        /// 反映済みだった変更をすべて元に戻せたか。
        restored: bool,
    },

    /// 作品を新規作成しようとしたフォルダが空ではなかった。
    #[error("{} が空ではないため、作品を作成できません", root.display())]
    DirectoryNotEmpty {
        /// 作成先のフォルダ。
        root: PathBuf,
    },
}

/// ゴミ箱から元の場所へ戻せなかったファイル。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StillTrashed {
    /// 元の場所。
    pub original: RelPath,
    /// 今ある場所（`.kataribe/trash/<日時>/…`）。
    pub trashed: RelPath,
}

/// 改名（移動）の途中で止まり、元の場所へ戻せなかったもの。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StillMoved {
    /// 元の場所。
    pub original: RelPath,
    /// 今ある場所。移動の途中の置き場（`.kataribe/staging/<日時>/…`）か、移し先。
    pub current: RelPath,
}

/// [`ProjectError::PartialWrite`] のメッセージの、戻せなかったものの案内。
/// 書き込み・ゴミ箱・改名とでは実物のある場所も戻し方も違うので、分けて書く。
fn describe_unrestored(
    not_restored: &[RelPath],
    still_trashed: &[StillTrashed],
    still_moved: &[StillMoved],
    journal: Option<&RelPath>,
) -> String {
    let mut sentences = Vec::new();
    if !not_restored.is_empty() {
        let paths: Vec<&str> = not_restored.iter().map(RelPath::as_str).collect();
        sentences.push(format!(
            "{} は新しい内容のまま残っています（バックアップを取っていれば .kataribe/backups から戻せます）。",
            paths.join("、")
        ));
    }
    for stranded in still_trashed {
        sentences.push(format!(
            "{} はゴミ箱へ移したまま戻せませんでした。実物は {} にあります。",
            stranded.original, stranded.trashed
        ));
    }
    for stranded in still_moved {
        sentences.push(format!(
            "{} は改名（移動）の途中のまま戻せませんでした。実物は {} にあります。",
            stranded.original, stranded.current
        ));
    }
    if let Some(journal) = journal
        && !(still_trashed.is_empty() && still_moved.is_empty())
    {
        sentences.push(format!("操作の記録は {journal} にあります。"));
    }
    sentences.join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(path: &str) -> RelPath {
        RelPath::new(path).unwrap()
    }

    fn partial_write(not_restored: &[&str], still_trashed: &[(&str, &str)]) -> String {
        partial_write_with_moves(not_restored, still_trashed, &[])
    }

    fn partial_write_with_moves(
        not_restored: &[&str],
        still_trashed: &[(&str, &str)],
        still_moved: &[(&str, &str)],
    ) -> String {
        ProjectError::PartialWrite {
            not_restored: not_restored.iter().map(|path| rel(path)).collect(),
            still_trashed: still_trashed
                .iter()
                .map(|(original, trashed)| StillTrashed {
                    original: rel(original),
                    trashed: rel(trashed),
                })
                .collect(),
            still_moved: still_moved
                .iter()
                .map(|(original, current)| StillMoved {
                    original: rel(original),
                    current: rel(current),
                })
                .collect(),
            journal: Some(rel(".kataribe/staging/20231114-221320-000/journal.json")),
            source: Box::new(ProjectError::NotFound {
                path: rel("concept.md"),
            }),
        }
        .to_string()
    }

    #[test]
    fn partial_write_points_to_the_backups_for_files_left_with_new_content() {
        let message = partial_write(&["plot/chapters/01.md", "concept.md"], &[]);

        assert!(message.contains("plot/chapters/01.md、concept.md は新しい内容のまま残っています"));
        assert!(message.contains(".kataribe/backups"));
        assert!(!message.contains("ゴミ箱"));
    }

    #[test]
    fn partial_write_tells_where_the_files_that_stayed_in_the_trash_are() {
        let message = partial_write(
            &[],
            &[(
                "characters/rin.md",
                ".kataribe/trash/20231114-221320-000/characters/rin.md",
            )],
        );

        assert!(message.contains(
            "characters/rin.md はゴミ箱へ移したまま戻せませんでした。\
             実物は .kataribe/trash/20231114-221320-000/characters/rin.md にあります。"
        ));
        assert!(
            !message.contains(".kataribe/backups"),
            "ゴミ箱へ移したファイルはバックアップを取っていない: {message}"
        );
    }

    #[test]
    fn partial_write_separates_the_written_files_from_the_trashed_ones() {
        let message = partial_write(
            &["plot/chapters/01.md"],
            &[(
                "manuscript/01/s02.txt",
                ".kataribe/trash/20231114-221320-000/manuscript/01/s02.txt",
            )],
        );

        assert!(message.contains("plot/chapters/01.md は新しい内容のまま残っています"));
        assert!(message.contains("manuscript/01/s02.txt はゴミ箱へ移したまま"));
        assert!(
            message.ends_with("原因: concept.md が見つかりません"),
            "{message}"
        );
    }

    #[test]
    fn partial_write_tells_where_the_moved_files_are_and_where_the_journal_is() {
        let message = partial_write_with_moves(
            &[],
            &[],
            &[("manuscript/03", ".kataribe/staging/20231114-221320-000/m1")],
        );

        assert!(message.contains(
            "manuscript/03 は改名（移動）の途中のまま戻せませんでした。\
             実物は .kataribe/staging/20231114-221320-000/m1 にあります。"
        ));
        assert!(
            message.contains(
                "操作の記録は .kataribe/staging/20231114-221320-000/journal.json にあります。"
            ),
            "{message}"
        );
    }

    #[test]
    fn partial_write_does_not_mention_the_journal_when_only_written_files_are_left() {
        let message = partial_write(&["plot/chapters/01.md"], &[]);

        assert!(!message.contains("journal"), "{message}");
    }
}
