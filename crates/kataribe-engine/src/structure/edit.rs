//! 構成の操作の種類。

use kataribe_project::{ChapterId, CharacterMeta, RelPath, SceneId};
use serde::{Deserialize, Serialize};

/// 構成に対する 1 つの操作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StructureEdit {
    /// 人物を足す。
    AddCharacter {
        /// ID（ファイル名）。`None` か空白だけなら、読み（無ければ名前）からローマ字で決める。
        ///
        /// 文字列のまま受けて、使えるかどうかは変更案を作るときに確かめる
        /// （使えない ID は、利用者が直せる入力の誤りとして知らせる）。
        id: Option<String>,
        /// 項目。`order` が `None` なら、今の最大の次の番号（末尾）にする。
        meta: CharacterMeta,
        /// 人物資料の本文。空でもよい（空なら、生成の工程に「人物資料」が取りかかれる工程として出る）。
        body: String,
    },
    /// 人物を消す（ゴミ箱へ移す）。シーンの視点・登場人物の名前は書き換えない。
    RemoveCharacter {
        /// 消す人物資料。`characters/` 直下の Markdown。
        ///
        /// ID ではなくパスで指す。ファイル名が人物 ID の規則に合わない資料（手で足した `Rin.md` や `凛.md`）も
        /// 目次に出るので、消せるようにするため。
        path: RelPath,
    },
    /// 世界観の資料を足す。
    AddWorldDocument {
        /// ファイル名（英小文字・数字・ハイフン。拡張子は付けない）。`None` なら題から決める。
        name: Option<String>,
        /// 題。本文の先頭の見出しになる。
        title: String,
        /// 本文。
        body: String,
    },
    /// 足した世界観の資料を消す（ゴミ箱へ移す）。世界観の概要（`world/overview.md`）は消せない。
    RemoveWorldDocument {
        /// 消す資料。`world/` 直下の Markdown。
        path: RelPath,
    },
    /// 章にシーンを足す。
    AddScene {
        /// 足す章。
        chapter: ChapterId,
        /// このシーンの前に足す。`None` なら章の末尾。
        before: Option<SceneId>,
        /// 足すシーンの設計。
        scene: NewScenePlan,
    },
    /// 章のシーンを消す。本文があれば、本文もゴミ箱へ移る。
    RemoveScene {
        /// 消すシーンのある章。
        chapter: ChapterId,
        /// 消すシーン。
        scene: SceneId,
    },
}

/// 足すシーンの設計。[`kataribe_project::ScenePlan`] から、足すときに決まる項目（id・ビート）を除いたもの。
/// id は足すときに、消したシーンの本文を引き継がないよう決める。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct NewScenePlan {
    /// シーン題。
    pub title: String,
    /// このシーンの要約。
    pub summary: String,
    /// 視点人物。
    pub pov: Option<String>,
    /// 登場人物名の一覧。
    pub characters: Vec<String>,
    /// 場所。
    pub place: Option<String>,
    /// 時間。
    pub time: Option<String>,
    /// 目標文字数。
    pub target_chars: Option<u32>,
}
