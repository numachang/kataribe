//! 生成タスク。

use kataribe_project::{ChapterId, CharacterId, RelPath, SceneId};
use serde::{Deserialize, Serialize};

/// 1 回の「生成」で行う仕事。どのタスクも変更案（`ChangeSet`）を返し、ファイルは直接書き換えない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Task {
    Concept,
    Style,
    World,
    Cast,
    Character {
        id: CharacterId,
    },
    Synopsis,
    Outline,
    ScenePlan {
        chapter: ChapterId,
    },
    /// 本文。生成単位が `chapter` のときは、このシーンから、次に本文のあるシーンの手前までをまとめて書く。
    Draft {
        chapter: ChapterId,
        scene: SceneId,
    },
    Revise {
        path: RelPath,
        instruction: String,
    },
    /// 指示から人物を 1 人作って足す（項目と人物資料の本文まで）。工程の一覧には出ない。
    AddCharacter {
        instruction: String,
    },
    /// 指示から世界観の資料を 1 つ作って足す。工程の一覧には出ない。
    AddWorldDocument {
        /// ファイル名（`world/<name>.md`）。`None` なら、題から決める（`StructureEdit::AddWorldDocument` と同じ）。
        name: Option<String>,
        instruction: String,
    },
}
