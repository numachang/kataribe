//! `add` / `remove` が共有する、構成の操作の流れ。
//!
//! 作品を開き、変更案を作り、利用者に確かめてほしいこと（ゴミ箱へ移るもの・参照が切れるシーン・注意書き）を
//! 標準エラー出力に出してから、適用する（`--dry-run` なら変更案を標準出力に出すだけ）。
//! GUI のような確認の手順は挟まないので、何が起きるかは必ずここで知らせる。
//! LLM も設定も使わないので、`Project::open` だけで動く。

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use kataribe_engine::{FileChange, SceneReference, StructureEdit, StructurePlan};
use kataribe_project::Project;

use crate::ApplyGuard;
use crate::output::Console;

use super::{
    Outcome, apply_change_set, render_change_set, render_trashed_files, report_applied_changes,
};

pub(super) async fn run(
    folder: &Path,
    edit: &StructureEdit,
    dry_run: bool,
    console: &dyn Console,
    apply_guard: &ApplyGuard,
) -> anyhow::Result<Outcome> {
    let project = Project::open(folder).context("作品フォルダを開けません")?;
    let plan = kataribe_engine::plan_structure_edit(&project, edit)?;
    console.eprint(&render_consequences(&plan))?;
    if dry_run {
        console.print(&render_change_set(&plan.change_set))?;
        return Ok(Outcome::Success);
    }
    apply_change_set(&plan.change_set, &project, apply_guard, console).await?;
    report_applied_changes(console, &plan.change_set, "")?;
    Ok(Outcome::Success)
}

/// 適用で起きることのうち、利用者が知っておくべきこと。何も無ければ空文字列。
fn render_consequences(plan: &StructurePlan) -> String {
    let mut rendered = String::new();
    for change in &plan.change_set.files {
        if let FileChange::Trash { files, .. } = change {
            let _ = write!(
                rendered,
                "ゴミ箱（.kataribe/trash/）へ移るもの:\n{}",
                render_trashed_files(files)
            );
        }
    }
    if !plan.references.is_empty() {
        rendered.push_str(
            "この人物の名前を挙げているシーン（名前は書き換えません。必要なら直してください）:\n",
        );
        for reference in &plan.references {
            let _ = writeln!(rendered, "  {}", render_reference(reference));
        }
    }
    for notice in &plan.notices {
        let _ = writeln!(rendered, "注意: {notice}");
    }
    rendered
}

fn render_reference(reference: &SceneReference) -> String {
    let roles = match (reference.as_pov, reference.as_character) {
        (true, true) => "視点・登場人物",
        (true, false) => "視点",
        (false, _) => "登場人物",
    };
    format!(
        "第{}章「{}」 {}「{}」（{roles}）",
        reference.chapter.number(),
        reference.chapter_title,
        reference.scene,
        reference.scene_title
    )
}

#[cfg(test)]
mod tests {
    use kataribe_engine::{ChangeSet, TrashedFile};
    use kataribe_project::{ChapterId, RelPath, SceneId};

    use super::*;

    fn reference(as_pov: bool, as_character: bool) -> SceneReference {
        SceneReference {
            chapter: ChapterId::from_number(1),
            chapter_title: "雨の匂い".to_owned(),
            scene: SceneId::from_number(2),
            scene_title: "書斎".to_owned(),
            as_pov,
            as_character,
        }
    }

    #[test]
    fn a_reference_says_whether_the_person_is_the_viewpoint_or_a_character() {
        assert_eq!(
            render_reference(&reference(true, true)),
            "第1章「雨の匂い」 s02「書斎」（視点・登場人物）"
        );
        assert_eq!(
            render_reference(&reference(true, false)),
            "第1章「雨の匂い」 s02「書斎」（視点）"
        );
        assert_eq!(
            render_reference(&reference(false, true)),
            "第1章「雨の匂い」 s02「書斎」（登場人物）"
        );
    }

    #[test]
    fn the_consequences_list_trashed_files_with_their_size_references_and_notices() {
        let path = RelPath::new("manuscript/01/s02.txt").unwrap();
        let mut change_set = ChangeSet::new("削除");
        change_set.files.push(FileChange::Trash {
            path: path.clone(),
            files: vec![TrashedFile {
                path,
                base_hash: None,
                chars: 1234,
            }],
        });
        let plan = StructurePlan {
            change_set,
            created: None,
            references: vec![reference(false, true)],
            notices: vec!["第3章は読めません。".to_owned()],
        };

        let rendered = render_consequences(&plan);

        assert!(
            rendered.contains("manuscript/01/s02.txt（1234 字）"),
            "{rendered}"
        );
        assert!(rendered.contains(".kataribe/trash/"), "{rendered}");
        assert!(rendered.contains("書斎」（登場人物）"), "{rendered}");
        assert!(rendered.contains("注意: 第3章は読めません。"), "{rendered}");
    }

    #[test]
    fn nothing_is_said_when_a_plan_has_nothing_to_warn_about() {
        let plan = StructurePlan {
            change_set: ChangeSet::new("追加"),
            created: None,
            references: Vec::new(),
            notices: Vec::new(),
        };

        assert_eq!(render_consequences(&plan), "");
    }
}
