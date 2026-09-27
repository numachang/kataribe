//! `export` サブコマンド: 本文を章題付きの一つのテキストにまとめる。

use std::io::Write as _;
use std::path::Path;

use anyhow::Context;
use kataribe_project::{Chapter, Project};

use crate::args::ExportArgs;
use crate::output::Console;

use super::Outcome;

pub fn run(args: &ExportArgs, console: &dyn Console) -> anyhow::Result<Outcome> {
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let text = export_text(&project)?;

    match &args.output {
        Some(path) => write_output(&project, path, args.force, &text)?,
        None => console
            .print(&text)
            .context("標準出力への書き込みに失敗しました")?,
    }
    Ok(Outcome::Success)
}

/// `path` に書き出す。作品フォルダの中を指すパスは拒否し、既存ファイルの上書きは
/// `--force` を指定したときだけ許す。
fn write_output(project: &Project, path: &Path, force: bool, text: &str) -> anyhow::Result<()> {
    reject_path_inside_project(project, path)?;
    if path.exists() && !force {
        anyhow::bail!(
            "書き出し先に既にファイルがあります（上書きするには --force を指定してください）: {}",
            path.display()
        );
    }
    write_atomically(path, text)
}

/// `path` が作品フォルダの中を指していないことを確かめる。
///
/// まだ存在しないファイルは、実在する親フォルダを canonicalize して比べる
/// （ファイル自体はまだ無いので canonicalize できないため）。
fn reject_path_inside_project(project: &Project, path: &Path) -> anyhow::Result<()> {
    let candidate = if path.exists() {
        path.canonicalize()
            .with_context(|| format!("書き出し先を確認できません: {}", path.display()))?
    } else {
        let parent = existing_parent(path);
        parent
            .canonicalize()
            .with_context(|| format!("書き出し先の親フォルダがありません: {}", parent.display()))?
    };
    if candidate.starts_with(project.store().canonical_root()) {
        anyhow::bail!(
            "書き出し先に作品フォルダの中は指定できません（原稿と資料を巻き込んで上書きしてしまうため）: {}",
            path.display()
        );
    }
    Ok(())
}

fn existing_parent(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// `path` と同じフォルダの一時ファイルに書いてから置き換える。書き込みの途中で失敗しても、
/// 既存のファイル（あれば）はそのまま残る。
///
/// [`kataribe_project::ProjectStore`] と違い、一時的なロックの再試行はしない。書き出しは
/// 作品フォルダの外への一度きりの操作で、書き込みの競合が繰り返し起きる想定がないため。
fn write_atomically(path: &Path, text: &str) -> anyhow::Result<()> {
    let parent = existing_parent(path);
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("書き出し先に一時ファイルを作れません: {}", parent.display()))?;
    temp.write_all(text.as_bytes())
        .context("一時ファイルへの書き込みに失敗しました")?;
    temp.as_file()
        .sync_all()
        .context("一時ファイルの同期に失敗しました")?;
    temp.persist(path)
        .map(|_file| ())
        .map_err(|error| error.error)
        .with_context(|| format!("書き出し先に書き込めません: {}", path.display()))
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
    use crate::output::testing::FailingConsole;
    use kataribe_engine::{NewProject, create_project};
    use kataribe_project::{ChapterId, ChapterMeta, Rating, SceneId, ScenePlan, WriteOptions};

    fn new_project(folder: &Path) -> Project {
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

    #[test]
    fn write_output_rejects_a_destination_inside_the_project_folder() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        let inside = project.root().join("novel.txt");

        let error = write_output(&project, &inside, false, "本文").unwrap_err();

        assert!(error.to_string().contains("作品フォルダの中"));
        assert!(!inside.exists());
    }

    #[test]
    fn write_output_rejects_an_existing_file_inside_the_project_folder_too() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        // 既に存在するファイル（この場合は kataribe.yaml）を指しても拒否されること。
        let inside = project.root().join("kataribe.yaml");
        assert!(inside.is_file());

        let error = write_output(&project, &inside, true, "本文").unwrap_err();

        assert!(error.to_string().contains("作品フォルダの中"));
    }

    #[test]
    fn write_output_refuses_to_overwrite_an_existing_file_without_force() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        let output_dir = tempfile::tempdir().unwrap();
        let output = output_dir.path().join("novel.txt");
        std::fs::write(&output, "既存の内容").unwrap();

        let error = write_output(&project, &output, false, "新しい本文").unwrap_err();

        assert!(error.to_string().contains("--force"));
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "既存の内容");
    }

    #[test]
    fn write_output_overwrites_when_force_is_given() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        let output_dir = tempfile::tempdir().unwrap();
        let output = output_dir.path().join("novel.txt");
        std::fs::write(&output, "既存の内容").unwrap();

        write_output(&project, &output, true, "新しい本文").unwrap();

        assert_eq!(std::fs::read_to_string(&output).unwrap(), "新しい本文");
    }

    #[test]
    fn write_output_creates_a_new_file_outside_the_project_folder() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        let output_dir = tempfile::tempdir().unwrap();
        let output = output_dir.path().join("novel.txt");

        write_output(&project, &output, false, "本文").unwrap();

        assert_eq!(std::fs::read_to_string(&output).unwrap(), "本文");
    }

    #[test]
    fn run_reports_a_failure_when_stdout_cannot_be_written_to() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        write_chapter_with_one_scene(&project);
        let args = ExportArgs {
            folder: folder.path().to_path_buf(),
            output: None,
            force: false,
        };

        let error = run(&args, &FailingConsole).unwrap_err();

        assert!(error.to_string().contains("標準出力"));
    }
}
