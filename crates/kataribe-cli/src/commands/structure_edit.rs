//! `add` / `remove` が共有する、構成の操作の流れ。
//!
//! 作品を開き、変更案を作り、利用者に確かめてほしいこと（ゴミ箱へ移るもの・参照が切れるシーン・注意書き）を
//! 標準エラー出力に出してから、適用する（`--dry-run` なら変更案を標準出力に出すだけ）。
//! GUI のような確認の手順は挟まないので、何が起きるかは必ずここで知らせる。
//! LLM も設定も使わないので、`Project::open` だけで動く。

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use kataribe_engine::{
    FileChange, RenumberedChapter, SceneReference, StructureEdit, StructurePlan, TrashedFile,
};
use kataribe_project::{Project, RelPath};

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
    console.eprint(&format!("{}\n", plan.completed_summary))?;
    Ok(Outcome::Success)
}

/// 適用で起きることのうち、利用者が知っておくべきこと。何も無ければ空文字列。
fn render_consequences(plan: &StructurePlan) -> String {
    let mut rendered = String::new();
    let trashed: Vec<(&RelPath, &[TrashedFile])> = plan
        .change_set
        .files
        .iter()
        .filter_map(|change| match change {
            FileChange::Trash { path, files } => Some((path, files.as_slice())),
            FileChange::Write { .. } | FileChange::Move { .. } | FileChange::Expect { .. } => None,
        })
        .collect();
    if !trashed.is_empty() {
        rendered.push_str("ゴミ箱（.kataribe/trash/）へ移るもの:\n");
        for (path, files) in trashed {
            rendered.push_str(&render_trashed(path, files));
        }
    }
    if !plan.renumbered.is_empty() {
        rendered.push_str("番号が変わる章（フォルダの名前も一緒に変わります）:\n");
        for chapter in &plan.renumbered {
            let _ = writeln!(rendered, "  {}", render_renumbered(chapter));
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

/// ゴミ箱へ移るもの。フォルダなら中のファイルを並べ、空のフォルダならそう書く。
fn render_trashed(path: &RelPath, files: &[TrashedFile]) -> String {
    if files.is_empty() {
        format!("  {path}（空のフォルダ）\n")
    } else {
        render_trashed_files(files)
    }
}

fn render_renumbered(chapter: &RenumberedChapter) -> String {
    let title = chapter
        .title
        .as_deref()
        .map(|title| format!("「{title}」"))
        .unwrap_or_default();
    format!(
        "第{}章{title} → 第{}章",
        chapter.from.number(),
        chapter.to.number()
    )
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
            completed_summary: String::new(),
            created: None,
            references: vec![reference(false, true)],
            renumbered: Vec::new(),
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
            completed_summary: String::new(),
            created: None,
            references: Vec::new(),
            renumbered: Vec::new(),
            notices: Vec::new(),
        };

        assert_eq!(render_consequences(&plan), "");
    }

    #[test]
    fn the_consequences_list_a_trashed_folder_once_and_the_renumbered_chapters() {
        let folder = RelPath::new("manuscript/03").unwrap();
        let file = |name: &str, chars| TrashedFile {
            path: RelPath::new(&format!("manuscript/03/{name}")).unwrap(),
            base_hash: None,
            chars,
        };
        let mut change_set = ChangeSet::new("削除");
        change_set.files.push(FileChange::Trash {
            path: RelPath::new("plot/chapters/03.md").unwrap(),
            files: vec![TrashedFile {
                path: RelPath::new("plot/chapters/03.md").unwrap(),
                base_hash: None,
                chars: 10,
            }],
        });
        change_set.files.push(FileChange::Trash {
            path: folder,
            files: vec![file("s01.txt", 1200), file("s02.txt", 800)],
        });
        let plan = StructurePlan {
            change_set,
            completed_summary: String::new(),
            created: None,
            references: Vec::new(),
            renumbered: vec![
                RenumberedChapter {
                    from: ChapterId::from_number(4),
                    to: ChapterId::from_number(3),
                    title: Some("雨の匂い".to_owned()),
                },
                RenumberedChapter {
                    from: ChapterId::from_number(5),
                    to: ChapterId::from_number(4),
                    title: None,
                },
            ],
            notices: Vec::new(),
        };

        let rendered = render_consequences(&plan);

        assert_eq!(
            rendered,
            "ゴミ箱（.kataribe/trash/）へ移るもの:\n\
             \x20 plot/chapters/03.md（10 字）\n\
             \x20 manuscript/03/s01.txt（1200 字）\n\
             \x20 manuscript/03/s02.txt（800 字）\n\
             番号が変わる章（フォルダの名前も一緒に変わります）:\n\
             \x20 第4章「雨の匂い」 → 第3章\n\
             \x20 第5章 → 第4章\n"
        );
    }

    #[test]
    fn an_empty_trashed_folder_is_said_to_be_empty() {
        let folder = RelPath::new("manuscript/03").unwrap();

        assert_eq!(
            render_trashed(&folder, &[]),
            "  manuscript/03（空のフォルダ）\n"
        );
    }
}
