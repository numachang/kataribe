//! サブコマンドごとの実装。

pub mod add;
pub mod api_key;
pub mod export;
pub mod generate;
pub mod models;
pub mod new;
pub mod project_settings;
pub mod quality;
pub mod remove;
pub mod run;
pub mod status;
mod structure_edit;

use std::fmt::Write as _;
use std::sync::Arc;

use anyhow::Context;
use kataribe_engine::{ChangeSet, FileChange, LlmSettings, TrashedFile};
use kataribe_llm::ChatModel;
use kataribe_project::Project;

use crate::ApplyGuard;
use crate::output::Console;
use crate::settings;

/// コマンドの実行結果。中止されたかどうかで終了コードを決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Cancelled,
}

/// 変更案を作品フォルダに適用する（`generate` / `run` で共有する）。
///
/// 適用している間は `apply_guard` を保持する。これにより、2 回目の Ctrl+C を受けて
/// 応答しなくなった処理を待たずに終了するときも、適用の途中であれば `main.rs` が
/// 完了を待ってから終了できる（原稿を失わないため）。
///
/// 適用に失敗したときは、失う内容（生成した内容や、入力した内容）が分かるよう変更案を先に表示してから、適用のエラーを
/// 返す。表示（標準出力・標準エラー出力）に失敗しても、本来の失敗理由を隠さないよう無視する。
pub(crate) async fn apply_change_set(
    changes: &ChangeSet,
    project: &Project,
    apply_guard: &ApplyGuard,
    console: &dyn Console,
) -> anyhow::Result<()> {
    let result = {
        let _guard = apply_guard.enter().await;
        changes.apply(project)
    };
    result.map_err(|error| {
        let _ = console.print(&render_change_set(changes));
        let _ = console.eprint("適用に失敗しました。変更の内容を表示します。\n");
        anyhow::Error::from(error).context("変更を作品フォルダに書き込めません")
    })
}

/// 接続設定から生成に使う LLM を作る。`generate` / `run` で共有する。
pub(crate) fn build_chat_model(
    llm: &LlmSettings,
    api_key_env: &str,
) -> anyhow::Result<Arc<dyn ChatModel>> {
    let api_key = api_key_for(llm, api_key_env)?;
    kataribe_engine::build_chat_model(llm, api_key).context("LLM クライアントを初期化できません")
}

/// API キーを使う接続先のときだけ API キーを探す（使わない接続先では資格情報マネージャーに触らない）。
pub(crate) fn api_key_for(llm: &LlmSettings, api_key_env: &str) -> anyhow::Result<Option<String>> {
    if llm.provider.uses_api_key() {
        settings::resolve_api_key(api_key_env)
    } else {
        Ok(None)
    }
}

/// 変更案を人が読める形にする。書き込むファイルは、パスと書き込まれるはずの内容を、
/// ゴミ箱へ移すものは、パスと移るファイルの一覧を並べる。
///
/// `--dry-run` で見せる内容と、適用に失敗したときに失われる内容を表示するのに使う
/// （`generate` / `run` / `add` / `remove` で共有する）。
pub(crate) fn render_change_set(changes: &ChangeSet) -> String {
    let mut rendered = format!("{}\n", changes.summary);
    for change in &changes.files {
        // String への write! は失敗しないため、戻り値は無視してよい。
        let _ = match change {
            FileChange::Write { path, content, .. } => {
                write!(rendered, "\n=== {path} ===\n{content}\n")
            }
            FileChange::Trash { path, files } => write!(
                rendered,
                "\n=== ゴミ箱へ移す: {path} ===\n{}",
                render_trashed_files(files)
            ),
        };
    }
    rendered
}

/// ゴミ箱へ移すファイルを、1 行に 1 つ（パスと文字数）で並べる。
pub(crate) fn render_trashed_files(files: &[TrashedFile]) -> String {
    let mut rendered = String::new();
    for file in files {
        let _ = writeln!(rendered, "  {}（{} 字）", file.path, file.chars);
    }
    rendered
}

/// 適用した変更を、種類ごとに標準エラー出力へ知らせる（書き込んだファイルと、ゴミ箱へ移したもの）。
/// `indent` は各行の頭に付ける空白。
pub(crate) fn report_applied_changes(
    console: &dyn Console,
    changes: &ChangeSet,
    indent: &str,
) -> std::io::Result<()> {
    for change in &changes.files {
        let line = match change {
            FileChange::Write { path, .. } => format!("{indent}書き込み: {path}\n"),
            FileChange::Trash { path, .. } => format!("{indent}ゴミ箱へ: {path}\n"),
        };
        console.eprint(&line)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kataribe_project::RelPath;

    use super::*;
    use crate::output::BufferConsole;

    fn path(path: &str) -> RelPath {
        RelPath::new(path).unwrap()
    }

    fn mixed_change_set() -> ChangeSet {
        let mut changes = ChangeSet::new("まとめ");
        changes.files.push(FileChange::Write {
            path: path("plot/chapters/01.md"),
            content: "新しい章立て".to_owned(),
            previous: None,
            base_hash: None,
        });
        changes.files.push(FileChange::Trash {
            path: path("manuscript/01/s02.txt"),
            files: vec![TrashedFile {
                path: path("manuscript/01/s02.txt"),
                base_hash: None,
                chars: 1234,
            }],
        });
        changes
    }

    #[test]
    fn render_change_set_shows_the_content_of_writes_and_the_files_to_be_trashed() {
        let rendered = render_change_set(&mixed_change_set());

        assert_eq!(
            rendered,
            "まとめ\n\
             \n=== plot/chapters/01.md ===\n新しい章立て\n\
             \n=== ゴミ箱へ移す: manuscript/01/s02.txt ===\n  manuscript/01/s02.txt（1234 字）\n"
        );
    }

    #[test]
    fn report_applied_changes_tells_writes_and_trashed_files_apart() {
        let console = BufferConsole::new();

        report_applied_changes(&console, &mixed_change_set(), "  ").unwrap();

        assert_eq!(
            console.stderr(),
            "  書き込み: plot/chapters/01.md\n  ゴミ箱へ: manuscript/01/s02.txt\n"
        );
    }
}
