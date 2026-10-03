import type { OverviewEntry } from "../../../api/types";
import type { RemoveEdit, StructureRequest } from "../../../features/structure/structureRequest";
import { isAdditionalWorldDocumentPath, isCharacterDocumentPath } from "../../../lib/entryPaths";

/** 目次の行のメニューに出す、構成の操作。 */
export interface EntryAction {
  label: string;
  request: StructureRequest;
}

function removal(edit: RemoveEdit, label = "削除"): EntryAction {
  return { label, request: { kind: "remove", edit } };
}

/** 章立ての行（`plot/chapters/<NN>.md`）。章の前後に章を足せる。 */
function planChapterActions(chapter: string, nextChapter: string | null): EntryAction[] {
  return [
    { label: "この前に章を追加", request: { kind: "add_chapter", before: chapter } },
    { label: "この後に章を追加", request: { kind: "add_chapter", before: nextChapter } },
    removal({ kind: "remove_chapter", chapter }),
  ];
}

/**
 * 目次の項目に対して、できる構成の操作。何もできない項目は空。
 * `nextSibling` は、同じ階層で次の項目（最後なら null）。「この後に追加」の位置に使う。
 */
export function entryActionsFor(
  entry: OverviewEntry,
  nextSibling: OverviewEntry | null,
): EntryAction[] {
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
        request: { kind: "add_scene", chapter, before: nextSibling?.scene ?? null },
      },
      removal({ kind: "remove_scene", chapter, scene }),
    ];
  }
  if (entry.kind === "chapter" && chapter !== null) {
    // 章立てのファイル（パスを持つ）は章を足す・消す操作、本文の章見出し（パスを持たない）はシーンを足す操作と章を消す操作
    if (entry.path !== null) {
      return planChapterActions(chapter, nextSibling?.chapter ?? null);
    }
    return [
      { label: "シーンを追加", request: { kind: "add_scene", chapter, before: null } },
      removal({ kind: "remove_chapter", chapter }, "章を削除"),
    ];
  }
  return [];
}
