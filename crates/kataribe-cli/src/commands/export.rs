//! `export` サブコマンド: 本文を章題付きの一つのテキストにまとめる。

use std::io::Write as _;
use std::path::{Path, PathBuf};

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

/// `path` に書き出す。作品フォルダの中を指すパスは拒否し、フォルダを指すパスも拒否する。
/// 既存ファイルの上書きは `--force` を指定したときだけ許す。
fn write_output(project: &Project, path: &Path, force: bool, text: &str) -> anyhow::Result<()> {
    reject_path_inside_project(project, path)?;
    reject_a_directory_destination(path)?;
    write_atomically(path, text, force)
}

/// `path` が作品フォルダの中を指していないことを確かめる。
///
/// パスの文字列としての前方一致では、同じ実体を別の綴りで指すパス（Windows の管理共有
/// `\\localhost\c$\...` など）を見逃してしまう。そのため、書き出し先から実在する祖先を
/// 一つ見つけ、そこから上のフォルダをすべて実体で（[`same_file::is_same_file`]）比べる。
/// 実体で比べれば、管理共有はもちろん、大文字小文字の違いやジャンクション越しの別名も、
/// 実際には作品フォルダそのものなら見抜ける。
fn reject_path_inside_project(project: &Project, path: &Path) -> anyhow::Result<()> {
    let start = existing_ancestor(syntactic_parent(path))
        .with_context(|| format!("書き出し先の親フォルダを確認できません: {}", path.display()))?;
    for ancestor in start.ancestors() {
        let is_project_root = same_file::is_same_file(ancestor, project.root())
            .with_context(|| format!("書き出し先を確認できません: {}", ancestor.display()))?;
        if is_project_root {
            anyhow::bail!(
                "書き出し先に作品フォルダの中は指定できません（原稿と資料を巻き込んで上書きしてしまうため）: {}",
                path.display()
            );
        }
    }
    Ok(())
}

/// `path` が既存のフォルダを指していたら、専用のメッセージで拒否する。
///
/// `Path::exists` はリンク先の無いシンボリックリンクを「存在しない」として扱ってしまうため、
/// リンクそのものの有無を確かめられる `symlink_metadata` を使う。
fn reject_a_directory_destination(path: &Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => anyhow::bail!(
            "書き出し先にはフォルダが指定されています。ファイル名を指定してください: {}",
            path.display()
        ),
        _ => Ok(()),
    }
}

/// `path` の字面上の親フォルダ（実在するとは限らない）。親が無ければカレントディレクトリ。
fn syntactic_parent(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// `path` から、実在する祖先を一つ見つける（`path` 自身が実在しなくてもよい）。
fn existing_ancestor(path: &Path) -> std::io::Result<PathBuf> {
    for ancestor in path.ancestors() {
        if ancestor.try_exists()? {
            return Ok(ancestor.to_path_buf());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "実在する祖先フォルダが見つかりません",
    ))
}

/// `path` と同じフォルダの一時ファイルに書いてから置き換える。書き込みの途中で失敗しても、
/// 既存のファイル（あれば）はそのまま残る。
///
/// `force` でなければ、既存ファイルが無いことを確かめてから置き換えるのではなく
/// （その間に別プロセスがファイルを作る競合が起こりうる）、置き換え自体を無条件では行わない
/// `persist_noclobber` を使い、既存判定と置き換えを一つの操作にする。
///
/// [`kataribe_project::ProjectStore`] と違い、一時的なロックの再試行はしない。書き出しは
/// 作品フォルダの外への一度きりの操作で、書き込みの競合が繰り返し起きる想定がないため。
fn write_atomically(path: &Path, text: &str, force: bool) -> anyhow::Result<()> {
    let parent = syntactic_parent(path);
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("書き出し先に一時ファイルを作れません: {}", parent.display()))?;
    temp.write_all(text.as_bytes())
        .context("一時ファイルへの書き込みに失敗しました")?;
    temp.as_file()
        .sync_all()
        .context("一時ファイルの同期に失敗しました")?;
    let result = if force {
        temp.persist(path).map(|_file| ())
    } else {
        temp.persist_noclobber(path).map(|_file| ())
    };
    result.map_err(|error| describe_persist_error(error.error, path, force))
}

fn describe_persist_error(error: std::io::Error, path: &Path, force: bool) -> anyhow::Error {
    if !force && error.kind() == std::io::ErrorKind::AlreadyExists {
        anyhow::anyhow!(
            "書き出し先に既にファイルがあります（上書きするには --force を指定してください）: {}",
            path.display()
        )
    } else {
        anyhow::Error::new(error).context(format!("書き出し先に書き込めません: {}", path.display()))
    }
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
    use crate::test_support::new_test_project as new_project;
    use kataribe_project::{ChapterId, ChapterMeta, SceneId, ScenePlan, WriteOptions};

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
    fn write_output_rejects_the_project_folder_itself_as_the_destination() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());

        let error = write_output(&project, project.root(), false, "本文").unwrap_err();

        assert!(error.to_string().contains("フォルダ"));
    }

    #[test]
    fn write_output_rejects_an_unrelated_existing_directory_as_the_destination() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        let other_dir = tempfile::tempdir().unwrap();

        let error = write_output(&project, other_dir.path(), false, "本文").unwrap_err();

        assert!(error.to_string().contains("フォルダ"));
    }

    /// `..` を含む字面上のパスでも、実際に辿った先が作品フォルダの中なら拒否できること。
    #[test]
    fn write_output_rejects_a_destination_that_escapes_back_into_the_project_via_dot_dot() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        std::fs::create_dir(project.root().join("sub")).unwrap();
        let via_dot_dot = project.root().join("sub").join("..").join("novel.txt");

        let error = write_output(&project, &via_dot_dot, false, "本文").unwrap_err();

        assert!(error.to_string().contains("作品フォルダの中"));
    }

    /// Windows のジャンクション（ディレクトリの別名）越しに作品フォルダを指しても拒否できること。
    /// ジャンクションは通常の権限でも作れるが、環境によっては作れないことがあるので、その場合は
    /// このテストを skip する。
    #[cfg(windows)]
    #[test]
    fn write_output_rejects_the_project_folder_via_a_junction() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());
        let junction_parent = tempfile::tempdir().unwrap();
        let junction = junction_parent.path().join("alias-to-project");

        if !create_junction(&junction, project.root()) {
            eprintln!("ジャンクションを作れない環境のため、このテストは skip します");
            return;
        }

        let output = junction.join("novel.txt");
        let error = write_output(&project, &output, false, "本文").unwrap_err();

        assert!(error.to_string().contains("作品フォルダの中"));
    }

    #[cfg(windows)]
    fn create_junction(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Windows の管理共有（`\\localhost\<ドライブ>$\...`）越しに作品フォルダを指しても拒否できること。
    /// パスの文字列としての前方一致比較では見抜けなかった不具合の再現テスト。管理共有が使えない
    /// （ループバックの SMB が無効など）環境では skip する。
    #[cfg(windows)]
    #[test]
    fn write_output_rejects_the_project_folder_via_an_administrative_share_alias() {
        let folder = tempfile::tempdir().unwrap();
        let project = new_project(folder.path());

        let Some(share_root) = administrative_share_path(project.root()) else {
            eprintln!("管理共有のパスを組み立てられない環境のため、このテストは skip します");
            return;
        };
        if !matches!(
            same_file::is_same_file(&share_root, project.root()),
            Ok(true)
        ) {
            eprintln!("管理共有が使えない環境のため、このテストは skip します");
            return;
        }

        let output = share_root.join("novel.txt");
        let error = write_output(&project, &output, false, "本文").unwrap_err();

        assert!(error.to_string().contains("作品フォルダの中"));
    }

    /// `path` のドライブ文字を管理共有の形式（`\\localhost\<ドライブ>$`）に書き換える。
    /// ドライブ文字を取り出せない場合（UNC パスなど）は `None`。
    #[cfg(windows)]
    fn administrative_share_path(path: &Path) -> Option<PathBuf> {
        use std::path::{Component, Prefix};

        let mut components = path.components();
        let Some(Component::Prefix(prefix)) = components.next() else {
            return None;
        };
        let drive_letter = match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter as char,
            _ => return None,
        };
        let mut share = PathBuf::from(format!(r"\\localhost\{drive_letter}$"));
        for component in components {
            if component != Component::RootDir {
                share.push(component.as_os_str());
            }
        }
        Some(share)
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
