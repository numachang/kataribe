//! 構成の操作の結果。

use kataribe_project::{ChapterId, Project, RelPath, SceneId};
use serde::{Deserialize, Serialize};

use crate::change_set::ChangeSet;

/// 構成の操作の変更案と、利用者に見せる材料。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct StructurePlan {
    /// 作る変更案。作品フォルダを開いている `Project` のものとして印が付いている。
    /// `summary` は適用する前に見せる説明（「人物「霧島 凛」を追加します。」）。
    pub change_set: ChangeSet,
    /// 適用したあとに利用者へ知らせる文（「人物「霧島 凛」を追加しました。」）。
    pub completed_summary: String,
    /// 適用したあとに開く文書。何も開かなければ `None`。
    pub created: Option<RelPath>,
    /// 人物を消すとき、その人物の名前を挙げているシーン。名前は書き換えない（知らせるだけ）。
    pub references: Vec<SceneReference>,
    /// 章を足す・消す・並べ替えるときに、番号が変わる章（後ろの章。並べ替えでは動く範囲の章）。番号の小さい順。
    pub renumbered: Vec<RenumberedChapter>,
    /// 利用者への注意書き（「第 3 章は読めないため参照を確かめられませんでした」など）。
    pub notices: Vec<String>,
}

/// 変更の言い回し。適用する前の説明（「〜します。」）と適用したあとの知らせ（「〜しました。」）を、
/// 同じ言い回しから作って食い違わないようにする。
pub(super) struct Wording {
    /// 「ます」「ました」の前まで（例「人物「霧島 凛」を追加し」）。
    stem: String,
}

impl Wording {
    pub(super) fn new(stem: impl Into<String>) -> Self {
        Self { stem: stem.into() }
    }

    /// 適用する前の説明。
    pub(super) fn planned(&self) -> String {
        format!("{}ます。", self.stem)
    }

    /// 適用したあとの知らせ。
    fn completed(&self) -> String {
        format!("{}ました。", self.stem)
    }
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

/// 番号が変わる章。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RenumberedChapter {
    /// 変わる前の番号。
    pub from: ChapterId,
    /// 変わった後の番号。
    pub to: ChapterId,
    /// 章題。章立てが読めなければ `None`。
    pub title: Option<String>,
}

impl StructurePlan {
    /// 変更案だけの計画。何も開かず、参照も注意書きも番号の変わる章も無い。
    /// `change_set` は `wording.planned()` を説明にして作っておく。
    pub(super) fn new(change_set: ChangeSet, wording: &Wording) -> Self {
        Self {
            change_set,
            completed_summary: wording.completed(),
            created: None,
            references: Vec::new(),
            renumbered: Vec::new(),
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
