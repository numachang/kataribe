//! `export` サブコマンド: 本文を章題付きの一つのテキストにまとめる。

use anyhow::Context;
use kataribe_project::{Chapter, Project};

use crate::args::ExportArgs;
use crate::output::Console;

use super::Outcome;

pub fn run(args: &ExportArgs, console: &dyn Console) -> anyhow::Result<Outcome> {
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let text = export_text(&project)?;

    match &args.output {
        Some(path) => std::fs::write(path, &text)
            .with_context(|| format!("書き出し先に書き込めません: {}", path.display()))?,
        None => console.print(&text),
    }
    Ok(Outcome::Success)
}

/// 章ごとに「第N章　章題」を見出しにし、シーンを空行で区切って一つの文章にまとめる。
fn export_text(project: &Project) -> anyhow::Result<String> {
    let mut chapters_text = Vec::new();
    for chapter in project.chapters().context("章の一覧を取得できません")? {
        chapters_text.push(export_chapter(project, &chapter)?);
    }
    let mut result = chapters_text.join("\n\n");
    result.push('\n');
    Ok(result)
}

fn export_chapter(project: &Project, chapter: &Chapter) -> anyhow::Result<String> {
    let heading = format!("第{}章　{}", chapter.id.number(), chapter.meta.title);
    let mut blocks = vec![heading];
    for scene in &chapter.meta.scenes {
        if let Some(text) = project.scene_text(&chapter.id, &scene.id)? {
            // 行頭の全角字下げは原稿の一部なので残し、末尾の空行だけ整える。
            let trimmed = text.trim_end();
            if !trimmed.trim().is_empty() {
                blocks.push(trimmed.to_owned());
            }
        }
    }
    Ok(blocks.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_engine::{NewProject, create_project};
    use kataribe_project::{ChapterId, ChapterMeta, Rating, SceneId, ScenePlan, WriteOptions};

    fn new_project(folder: &std::path::Path) -> Project {
        create_project(
            folder,
            NewProject {
                title: "みさき館の殺人".into(),
                author: None,
                genre: "mystery".into(),
                genre_note: None,
                rating: Rating::General,
                target_length: 6000,
                idea: "嵐で孤立した洋館で起きる密室殺人。".into(),
            },
        )
        .unwrap()
    }

    fn write_chapter_with_one_scene(project: &Project) {
        let chapter_id = ChapterId::from_number(1);
        let scene_id = SceneId::from_number(1);
        let chapter = Chapter {
            id: chapter_id,
            meta: ChapterMeta {
                title: "雨の匂い".into(),
                scenes: vec![ScenePlan {
                    id: scene_id,
                    title: "事務所に届いた依頼".into(),
                    summary: "雨の夜…".into(),
                    pov: None,
                    characters: vec![],
                    place: None,
                    time: None,
                    target_chars: None,
                    beats: vec![],
                    extra: std::collections::BTreeMap::new(),
                }],
                extra: std::collections::BTreeMap::new(),
            },
            storyline: "この章のストーリーライン".into(),
        };
        project
            .store()
            .write_text(
                &kataribe_project::layout::chapter_path(&chapter_id),
                &chapter.render().unwrap(),
                WriteOptions::default(),
            )
            .unwrap();
        project
            .store()
            .write_text(
                &kataribe_project::layout::scene_text_path(&chapter_id, &scene_id),
                "　雨が降っていた。\n",
                WriteOptions::default(),
            )
            .unwrap();
    }

    #[test]
    fn export_joins_heading_and_scene_text() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        write_chapter_with_one_scene(&project);

        let text = export_text(&project).unwrap();

        assert_eq!(text, "第1章　雨の匂い\n\n　雨が降っていた。\n");
    }

    #[test]
    fn export_skips_unwritten_scenes() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        let chapter_id = ChapterId::from_number(1);
        let chapter = Chapter {
            id: chapter_id,
            meta: ChapterMeta {
                title: "雨の匂い".into(),
                scenes: vec![ScenePlan {
                    id: SceneId::from_number(1),
                    title: "未執筆のシーン".into(),
                    summary: String::new(),
                    pov: None,
                    characters: vec![],
                    place: None,
                    time: None,
                    target_chars: None,
                    beats: vec![],
                    extra: std::collections::BTreeMap::new(),
                }],
                extra: std::collections::BTreeMap::new(),
            },
            storyline: String::new(),
        };
        project
            .store()
            .write_text(
                &kataribe_project::layout::chapter_path(&chapter_id),
                &chapter.render().unwrap(),
                WriteOptions::default(),
            )
            .unwrap();

        let text = export_text(&project).unwrap();

        assert_eq!(text, "第1章　雨の匂い\n");
    }
}
