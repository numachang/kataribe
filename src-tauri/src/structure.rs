//! 作品の構成の操作（人物・世界観の資料・シーンの追加と削除）。`plan_structure_edit` /
//! `suggest_character_id` コマンドの中身。
//!
//! 変更案を作るだけで、適用は `apply_change_set` が行う（画面が確認してから適用できるように分けている）。

use kataribe_engine::{StructureEdit, StructurePlan};
use kataribe_project::Project;

use crate::error::CommandError;

/// 構成の操作の変更案と、確認の材料（ゴミ箱へ移るもの・参照が切れるシーン・注意書き）を作る。
/// 作品フォルダは書き換えない。入力の誤りは `invalid_input`、対象が無ければ `not_found`。
pub fn plan_structure_edit(
    project: &Project,
    edit: &StructureEdit,
) -> Result<StructurePlan, CommandError> {
    Ok(kataribe_engine::plan_structure_edit(project, edit)?)
}

/// 人物の ID の案を、読み（かな）からローマ字で作る。使用済みの ID は避ける。
pub fn suggest_character_id(
    project: &Project,
    reading: &str,
    name: &str,
) -> Result<String, CommandError> {
    let id = kataribe_engine::suggest_character_id(project, reading, name)?;
    Ok(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use kataribe_engine::{FileChange, NewProject, create_project};
    use kataribe_project::{
        BackupMode, CharacterMeta, Rating, RelPath, WriteCondition, WriteOptions,
    };
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn open_project(dir: &TempDir) -> Project {
        create_project(
            dir.path(),
            NewProject {
                title: "テスト作品".to_owned(),
                author: None,
                genre: "general".to_owned(),
                genre_note: None,
                rating: Rating::General,
                target_length: 10_000,
                idea: "静かな夜の物語".to_owned(),
            },
        )
        .unwrap()
    }

    fn put(project: &Project, path: &str, content: &str) {
        project
            .store()
            .write_text(
                &RelPath::new(path).unwrap(),
                content,
                WriteOptions {
                    condition: WriteCondition::Any,
                    backup: BackupMode::Never,
                },
            )
            .unwrap();
    }

    fn add_character(name: &str, reading: &str) -> StructureEdit {
        StructureEdit::AddCharacter {
            id: None,
            meta: CharacterMeta {
                name: name.to_owned(),
                reading: Some(reading.to_owned()),
                ..CharacterMeta::default()
            },
            body: String::new(),
        }
    }

    fn add_character_with_id(id: &str) -> StructureEdit {
        StructureEdit::AddCharacter {
            id: Some(id.to_owned()),
            meta: CharacterMeta {
                name: "凛".to_owned(),
                ..CharacterMeta::default()
            },
            body: String::new(),
        }
    }

    fn remove_character(file_stem: &str) -> StructureEdit {
        StructureEdit::RemoveCharacter {
            path: RelPath::new(&format!("characters/{file_stem}.md")).unwrap(),
        }
    }

    #[test]
    fn plan_structure_edit_returns_the_change_set_and_what_to_open() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let plan =
            plan_structure_edit(&project, &add_character("霧島 凛", "きりしま りん")).unwrap();

        assert_eq!(
            plan.created,
            Some(RelPath::new("characters/kirishima-rin.md").unwrap())
        );
        assert_eq!(plan.change_set.files.len(), 1);
        assert!(matches!(plan.change_set.files[0], FileChange::Write { .. }));
        assert_eq!(
            RelPath::new("characters/kirishima-rin.md")
                .map(|path| project.store().exists(&path))
                .unwrap(),
            false,
            "計画しただけでは書かない"
        );
    }

    #[test]
    fn plan_structure_edit_for_a_removal_carries_the_trash_and_the_references() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put(&project, "characters/rin.md", "---\nname: 霧島 凛\n---\n");
        put(
            &project,
            "plot/chapters/01.md",
            "---\ntitle: 一\nscenes:\n  - id: s01\n    title: 朝\n    summary: 要約\n    pov: 凛\n---\n",
        );

        let plan = plan_structure_edit(&project, &remove_character("rin")).unwrap();

        assert!(matches!(plan.change_set.files[0], FileChange::Trash { .. }));
        assert_eq!(plan.references.len(), 1);
        assert!(plan.references[0].as_pov);
    }

    #[test]
    fn plan_structure_edit_maps_an_input_mistake_to_invalid_input() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = plan_structure_edit(&project, &add_character("  ", "")).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
        assert_eq!(error.message, "人物の名前を入力してください。");
    }

    #[test]
    fn plan_structure_edit_maps_an_unusable_character_id_to_invalid_input() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = plan_structure_edit(&project, &add_character_with_id("Rin")).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
        assert!(
            error.message.starts_with("ID「Rin」は使えません。"),
            "{}",
            error.message
        );
        let unwritten = RelPath::new("characters/Rin.md").unwrap();
        assert!(!project.store().exists(&unwritten));
    }

    #[test]
    fn a_character_file_named_against_the_id_rules_can_be_removed() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put(&project, "characters/Rin.md", "---\nname: 霧島 凛\n---\n");
        put(&project, "characters/凛.md", "---\nname: [\n---\n");

        let readable = plan_structure_edit(&project, &remove_character("Rin")).unwrap();
        let broken = plan_structure_edit(&project, &remove_character("凛")).unwrap();
        readable.change_set.apply(&project).unwrap();
        broken.change_set.apply(&project).unwrap();

        assert_eq!(readable.notices, Vec::<String>::new());
        assert_eq!(broken.notices.len(), 1);
        for path in ["characters/Rin.md", "characters/凛.md"] {
            let removed = RelPath::new(path).unwrap();
            assert!(!project.store().exists(&removed), "{path}");
        }
    }

    #[test]
    fn plan_structure_edit_maps_a_non_character_path_to_invalid_input() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put(&project, "world/glossary.md", "# 用語集\n");

        let error = plan_structure_edit(
            &project,
            &StructureEdit::RemoveCharacter {
                path: RelPath::new("world/glossary.md").unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::InvalidInput);
    }

    #[test]
    fn plan_structure_edit_maps_a_missing_target_to_not_found() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let error = plan_structure_edit(&project, &remove_character("rin")).unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[test]
    fn suggest_character_id_makes_a_romaji_id_from_the_reading() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let id = suggest_character_id(&project, "きりしま りん", "霧島 凛").unwrap();

        assert_eq!(id, "kirishima-rin");
    }

    #[test]
    fn suggest_character_id_avoids_an_id_in_use() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put(
            &project,
            "characters/kirishima-rin.md",
            "---\nname: 霧島 凛\n---\n",
        );

        let id = suggest_character_id(&project, "きりしま りん", "霧島 凛").unwrap();

        assert_eq!(id, "kirishima-rin-2");
    }
}
