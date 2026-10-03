//! 世界観の資料の追加と削除。

use kataribe_project::{MarkdownDoc, Project, ProjectError, RelPath, WorldDocumentName, layout};
use kataribe_text::romaji::{is_romanizable, to_romaji};

use super::plan::{StructurePlan, Wording};
use crate::change_set::ChangeSet;
use crate::error::{EngineError, Result};

/// 世界観の資料を足す変更案。`world/<name>.md` を新規に書く。
///
/// 世界観の資料は front matter を持たない文字列のまま開く文書なので、題は本文の先頭の見出しにする
/// （生成した資料と同じ形で、目次の表示名は見出しから拾われる）。
pub(super) fn add(
    project: &Project,
    requested_name: Option<&str>,
    title: &str,
    body: &str,
) -> Result<StructurePlan> {
    let title = title.trim();
    if title.is_empty() {
        return Err(EngineError::InvalidInput(
            "資料の題を入力してください。".into(),
        ));
    }
    if title.contains(['\n', '\r']) {
        return Err(EngineError::InvalidInput(
            "資料の題は 1 行で入力してください。".into(),
        ));
    }
    let name = resolve_name(project, requested_name, title)?;
    let path = layout::world_document_path(&name);
    if project.store().exists(&path) {
        return Err(EngineError::InvalidInput(format!(
            "ファイル名「{name}」はもう使われています。"
        )));
    }
    let wording = Wording::new(format!("世界観の資料「{title}」を追加し"));
    let mut changes = ChangeSet::new(wording.planned());
    changes.put(path.clone(), render(title, body), None);
    Ok(StructurePlan::new(changes, &wording).opening(path))
}

/// 足した世界観の資料を消す（ゴミ箱へ移す）変更案。
pub(super) fn remove(project: &Project, path: &RelPath) -> Result<StructurePlan> {
    if !layout::is_additional_world_document(path) {
        return Err(EngineError::InvalidInput(format!(
            "{path} は消せる世界観の資料ではありません（消せるのは、world/ 直下の概要以外の Markdown です）。"
        )));
    }
    let file = project
        .store()
        .read_text_opt(path)?
        .ok_or_else(|| EngineError::NotFound(format!("世界観の資料 {path} がありません。")))?;
    let label = MarkdownDoc::parse(&file.content).map_or_else(
        |_| path.file_stem().to_owned(),
        |document| document.display_title(path.file_stem()),
    );
    let wording = Wording::new(format!("世界観の資料「{label}」をゴミ箱へ移し"));
    let mut changes = ChangeSet::new(wording.planned());
    changes.trash_file(path.clone(), &file);
    Ok(StructurePlan::new(changes, &wording))
}

/// 資料のファイル名を決める。指定があればそれを検証して使い、無ければ題から作る。
fn resolve_name(
    project: &Project,
    requested: Option<&str>,
    title: &str,
) -> Result<WorldDocumentName> {
    if let Some(requested) = requested.map(str::trim).filter(|name| !name.is_empty()) {
        return Ok(WorldDocumentName::new(requested).map_err(ProjectError::from)?);
    }
    let taken = project.world_document_names()?;
    Ok(WorldDocumentName::from_hint(&name_hint(title), &taken))
}

/// 題から作るファイル名の元。全部がかな・英数字・区切りで書けているときだけローマ字にする。
/// 漢字を含む題をローマ字にすると漢字が落ちて別の語になってしまうので、そのときは空（`doc` になる）。
fn name_hint(title: &str) -> String {
    if is_romanizable(title) {
        to_romaji(title)
    } else {
        String::new()
    }
}

/// ファイルの内容。題を見出しにし、本文があれば空行を挟んで続ける。
fn render(title: &str, body: &str) -> String {
    let body = body.trim_start_matches(['\r', '\n']).trim_end();
    if body.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{body}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_written_in_kana_becomes_a_romaji_name_hint() {
        assert_eq!(name_hint("ようご しゅう"), "yogo shu");
        assert_eq!(name_hint("City Map"), "City Map");
    }

    #[test]
    fn a_title_with_kanji_gives_no_name_hint() {
        assert_eq!(name_hint("港町の歴史"), "");
    }

    #[test]
    fn the_document_has_the_title_as_a_heading_followed_by_the_body() {
        assert_eq!(
            render("用語集", "霧：朝に出る。\n"),
            "# 用語集\n\n霧：朝に出る。\n"
        );
    }

    #[test]
    fn an_empty_body_leaves_only_the_heading() {
        assert_eq!(render("用語集", "  \n\n"), "# 用語集\n");
    }

    #[test]
    fn leading_blank_lines_of_the_body_are_dropped_but_indentation_is_kept() {
        assert_eq!(render("題", "\n\n　字下げ\n"), "# 題\n\n　字下げ\n");
    }
}
