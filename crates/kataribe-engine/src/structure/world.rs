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
    let name = match parse_requested_name(requested_name)? {
        Some(name) => name,
        None => name_from_title(project, title)?,
    };
    ensure_unused(project, &name)?;
    let path = layout::world_document_path(&name);
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

/// 利用者が指定したファイル名を、足す前に確かめる。規則に合わない名前と、使用済みの名前は失敗にする。
///
/// 空欄（空白だけを含む）なら `None`（題から決める）。LLM に作らせる前に、待たせずに断るために使う。
pub(crate) fn check_name(
    project: &Project,
    requested: Option<&str>,
) -> Result<Option<WorldDocumentName>> {
    let Some(name) = parse_requested_name(requested)? else {
        return Ok(None);
    };
    ensure_unused(project, &name)?;
    Ok(Some(name))
}

/// 指定されたファイル名の規則を確かめる。空欄なら `None`。
fn parse_requested_name(requested: Option<&str>) -> Result<Option<WorldDocumentName>> {
    let Some(requested) = requested.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(None);
    };
    let name = WorldDocumentName::new(requested).map_err(ProjectError::from)?;
    Ok(Some(name))
}

/// ファイル名がまだ使われていないことを、ファイルの有無で確かめる（大文字小文字の違うファイルも使用済み）。
fn ensure_unused(project: &Project, name: &WorldDocumentName) -> Result<()> {
    if project.store().exists(&layout::world_document_path(name)) {
        return Err(EngineError::InvalidInput(format!(
            "ファイル名「{name}」はもう使われています。"
        )));
    }
    Ok(())
}

/// 題から、使用済みの名前を避けてファイル名を作る。
fn name_from_title(project: &Project, title: &str) -> Result<WorldDocumentName> {
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
    use kataribe_project::{Rating, WriteOptions};

    use super::*;
    use crate::new_project::{NewProject, create_project};

    fn project_with_world_document(folder: &std::path::Path, name: &str) -> Project {
        let project = create_project(
            folder,
            NewProject {
                title: "題".into(),
                author: None,
                genre: "general".into(),
                genre_note: None,
                rating: Rating::General,
                target_length: 6000,
                idea: "種".into(),
            },
        )
        .unwrap();
        let path = RelPath::new(&format!("world/{name}.md")).unwrap();
        project
            .store()
            .write_text(&path, "# 資料\n", WriteOptions::default())
            .unwrap();
        project
    }

    #[test]
    fn an_unspecified_or_blank_name_is_left_to_the_title() {
        let folder = tempfile::tempdir().unwrap();
        let project = project_with_world_document(folder.path(), "glossary");

        assert_eq!(check_name(&project, None).unwrap(), None);
        assert_eq!(check_name(&project, Some(" \n")).unwrap(), None);
    }

    #[test]
    fn an_unused_valid_name_is_accepted_after_trimming() {
        let folder = tempfile::tempdir().unwrap();
        let project = project_with_world_document(folder.path(), "glossary");

        let name = check_name(&project, Some(" port-town ")).unwrap();

        assert_eq!(name.unwrap().as_str(), "port-town");
    }

    #[test]
    fn a_used_name_is_refused_as_an_input_error() {
        let folder = tempfile::tempdir().unwrap();
        let project = project_with_world_document(folder.path(), "glossary");

        let error = check_name(&project, Some("glossary")).unwrap_err();

        assert!(matches!(error, EngineError::InvalidInput(_)), "{error:?}");
    }

    #[test]
    fn a_name_that_breaks_the_rules_is_refused() {
        let folder = tempfile::tempdir().unwrap();
        let project = project_with_world_document(folder.path(), "glossary");

        for name in ["Port Town", "overview", "港町"] {
            let error = check_name(&project, Some(name)).unwrap_err();
            assert!(
                matches!(error, EngineError::Project(_)),
                "{name}: {error:?}"
            );
        }
    }

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
