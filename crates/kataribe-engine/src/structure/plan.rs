//! 構成の操作の結果。

use kataribe_project::{ChapterId, Project, RelPath, SceneId};
use serde::{Deserialize, Serialize};

use crate::change_set::ChangeSet;

/// 構成の操作の変更案と、利用者に見せる材料。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct StructurePlan {
    /// 作る変更案。作品フォルダを開いている `Project` のものとして印が付いている。
    pub change_set: ChangeSet,
    /// 適用したあとに開く文書。何も開かなければ `None`。
    pub created: Option<RelPath>,
    /// 人物を消すとき、その人物の名前を挙げているシーン。名前は書き換えない（知らせるだけ）。
    pub references: Vec<SceneReference>,
    /// 利用者への注意書き（「第 3 章は読めないため参照を確かめられませんでした」など）。
    pub notices: Vec<String>,
}

/// 人物の名前を挙げているシーン。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SceneReference {
    /// 章。
    pub chapter: ChapterId,
    /// 章題。
    pub chapter_title: String,
    /// シーン。
    pub scene: SceneId,
    /// シーン題。
    pub scene_title: String,
    /// 視点人物として挙げている。
    pub as_pov: bool,
    /// 登場人物として挙げている。
    pub as_character: bool,
}

impl StructurePlan {
    /// 変更案だけの計画。何も開かず、参照も注意書きも無い。
    pub(super) fn new(change_set: ChangeSet) -> Self {
        Self {
            change_set,
            created: None,
            references: Vec::new(),
            notices: Vec::new(),
        }
    }

    /// 適用したあとに開く文書を決める。
    #[must_use]
    pub(super) fn opening(self, path: RelPath) -> Self {
        Self {
            created: Some(path),
            ..self
        }
    }

    /// 変更案を `project` のものとして印を付ける。
    #[must_use]
    pub(super) fn made_for(self, project: &Project) -> Self {
        Self {
            change_set: self.change_set.made_for(project),
            ..self
        }
    }
}
