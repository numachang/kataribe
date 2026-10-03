//! 人物を消すとき、その人物の名前を挙げているシーンを調べる。

use kataribe_project::Project;

use super::plan::SceneReference;
use crate::error::Result;
use crate::names::refers_to;

/// 参照を調べた結果。
pub(super) struct ReferenceReport {
    /// 名前を挙げているシーン。
    pub references: Vec<SceneReference>,
    /// 読めなくて調べられなかった章の知らせ。
    pub notices: Vec<String>,
}

/// `character_name` の人物を、視点人物または登場人物として挙げているシーンを集める。
///
/// 全部の章を 1 つずつ読む。壊れた章があっても止めず、調べられなかったことを `notices` に残す
/// （人物を消せなくなるより、知らせて続けるほうが利用者の助けになるため）。
pub(super) fn scenes_mentioning(
    project: &Project,
    character_name: &str,
) -> Result<ReferenceReport> {
    let mut report = ReferenceReport {
        references: Vec::new(),
        notices: Vec::new(),
    };
    for chapter_id in project.chapter_ids()? {
        let chapter = match project.chapter(&chapter_id) {
            Ok(Some(chapter)) => chapter,
            // 一覧を取ってから読むまでの間に消えた章は、調べるものが無い
            Ok(None) => continue,
            Err(error) => {
                report.notices.push(format!(
                    "第{}章は読めないため、参照を確かめられませんでした（{error}）。",
                    chapter_id.number()
                ));
                continue;
            }
        };
        for scene in &chapter.meta.scenes {
            let as_pov = scene
                .pov
                .as_deref()
                .is_some_and(|pov| refers_to(character_name, pov));
            let as_character = scene
                .characters
                .iter()
                .any(|name| refers_to(character_name, name));
            if as_pov || as_character {
                report.references.push(SceneReference {
                    chapter: chapter.id,
                    chapter_title: chapter.meta.title.clone(),
                    scene: scene.id,
                    scene_title: scene.title.clone(),
                    as_pov,
                    as_character,
                });
            }
        }
    }
    Ok(report)
}
