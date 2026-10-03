import type { OverviewEntry } from "../../../api/types";
import type { RemoveEdit, StructureRequest } from "../../../features/structure/structureRequest";
import { isAdditionalWorldDocumentPath, isCharacterDocumentPath } from "../../../lib/entryPaths";

/** 目次の行のメニューに出す、構成の操作。 */
export interface EntryAction {
  label: string;
  request: StructureRequest;
}

function removal(edit: RemoveEdit): EntryAction {
  return { label: "削除", request: { kind: "remove", edit } };
}

/**
 * 目次の項目に対して、できる構成の操作。何もできない項目は空。
 * `nextSceneId` は、同じ章でこのシーンの次のシーン（最後なら null）。「この後に追加」の位置に使う。
 */
export function entryActionsFor(entry: OverviewEntry, nextSceneId: string | null): EntryAction[] {
  if (entry.path !== null) {
    // 人物は ID ではなくパスで指す。ファイル名が ID の規則に合わない資料（`characters/Rin.md` など）も、目次に出ていれば消せる
    if (entry.kind === "character" && isCharacterDocumentPath(entry.path)) {
      return [removal({ kind: "remove_character", path: entry.path })];
    }
    if (isAdditionalWorldDocumentPath(entry.path)) {
      return [removal({ kind: "remove_world_document", path: entry.path })];
    }
  }

  const { chapter, scene } = entry;
  if (entry.kind === "scene" && chapter !== null && scene !== null) {
    return [
      { label: "この前にシーンを追加", request: { kind: "add_scene", chapter, before: scene } },
      {
        label: "この後にシーンを追加",
        request: { kind: "add_scene", chapter, before: nextSceneId },
      },
      removal({ kind: "remove_scene", chapter, scene }),
    ];
  }
  // 本文の章見出し（パスを持たない）。章立てのファイル自体（パスを持つ）には、シーンの追加を出さない
  if (entry.kind === "chapter" && entry.path === null && chapter !== null) {
    return [{ label: "シーンを追加", request: { kind: "add_scene", chapter, before: null } }];
  }
  return [];
}
