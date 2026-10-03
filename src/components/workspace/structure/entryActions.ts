import type { OverviewEntry } from "../../../api/types";
import type {
  MoveEdit,
  RemoveEdit,
  StructureRequest,
} from "../../../features/structure/structureRequest";
import { isAdditionalWorldDocumentPath, isCharacterDocumentPath } from "../../../lib/entryPaths";

/** 目次の行のメニューに出す、構成の操作。ダイアログを開くものと、そのまま並べ替えるものがある。 */
export type EntryAction =
  | { label: string; kind: "dialog"; request: StructureRequest }
  | { label: string; kind: "move"; edit: MoveEdit };

function dialog(label: string, request: StructureRequest): EntryAction {
  return { label, kind: "dialog", request };
}

function removal(edit: RemoveEdit, label = "削除"): EntryAction {
  return dialog(label, { kind: "remove", edit });
}

/** `from` から `step` の向きへ進んで最初に見つかる、入れ替えられる項目の位置。 */
function nearestIndex(
  list: OverviewEntry[],
  from: number,
  step: 1 | -1,
  isMovable: (entry: OverviewEntry) => boolean,
): number | null {
  for (let index = from + step; index >= 0 && index < list.length; index += step) {
    const candidate = list[index];
    if (candidate !== undefined && isMovable(candidate)) {
      return index;
    }
  }
  return null;
}

/**
 * 「上へ移す」「下へ移す」。`edit` は、並べ替えたあとに一覧の何番目（0 始まり）に来るかから、操作を作る。`list` は、動かす項目と同じ種類の項目を目次の並びで並べたもの（本物が数える一覧と同じ）。
 * 動かせない項目（読めない人物資料）は飛ばして隣の動かせる項目と入れ替え、動かせる隣が無い側は出さない。
 * 動かす項目自身が動かせないときは何も出さない。
 */
function moveActions(
  entry: OverviewEntry,
  list: OverviewEntry[],
  labels: { up: string; down: string },
  edit: (position: number) => MoveEdit,
  isMovable: (entry: OverviewEntry) => boolean = () => true,
): EntryAction[] {
  const index = list.indexOf(entry);
  if (index === -1 || !isMovable(entry)) {
    return [];
  }
  // 動かしたあとの位置は、入れ替える相手が今いる位置（相手が前なら上へ、後ろなら下へ動く）
  const up = nearestIndex(list, index, -1, isMovable);
  const down = nearestIndex(list, index, 1, isMovable);
  const actions: EntryAction[] = [];
  if (up !== null) {
    actions.push({ label: labels.up, kind: "move", edit: edit(up) });
  }
  if (down !== null) {
    actions.push({ label: labels.down, kind: "move", edit: edit(down) });
  }
  return actions;
}

const PLAIN_LABELS = { up: "上へ移す", down: "下へ移す" };
const CHAPTER_LABELS = { up: "章を上へ移す", down: "章を下へ移す" };

/** 人物の行。目次の人物資料の中で並べ替える。YAML が読めない資料（目次では警告が付く）は動かせない。 */
function characterActions(entry: OverviewEntry, path: string, siblings: OverviewEntry[]) {
  const characters = siblings.filter(
    (candidate) => candidate.path !== null && isCharacterDocumentPath(candidate.path),
  );
  return [
    ...moveActions(
      entry,
      characters,
      PLAIN_LABELS,
      (position) => ({ kind: "move_character", path, position }),
      (candidate) => candidate.error === null,
    ),
    removal({ kind: "remove_character", path }),
  ];
}

/** 章立ての行（`plot/chapters/<NN>.md`）。章の前後に章を足せ、章の順を変えられる。 */
function planChapterActions(
  entry: OverviewEntry,
  chapter: string,
  siblings: OverviewEntry[],
): EntryAction[] {
  const nextChapter = siblings[siblings.indexOf(entry) + 1]?.chapter ?? null;
  return [
    dialog("この前に章を追加", { kind: "add_chapter", before: chapter }),
    dialog("この後に章を追加", { kind: "add_chapter", before: nextChapter }),
    ...chapterMoveActions(entry, chapter, siblings, PLAIN_LABELS),
    removal({ kind: "remove_chapter", chapter }),
  ];
}

function chapterMoveActions(
  entry: OverviewEntry,
  chapter: string,
  siblings: OverviewEntry[],
  labels: { up: string; down: string },
): EntryAction[] {
  // 目次の章（あらすじなどは含まない。章立ての無いときの「章立て」の仮の行も含まない）の並びが、番号順の章の一覧と同じ
  const chapters = siblings.filter(
    (candidate) => candidate.kind === "chapter" && candidate.chapter !== null,
  );
  return moveActions(entry, chapters, labels, (position) => ({
    kind: "move_chapter",
    chapter,
    position,
  }));
}

function sceneActions(
  entry: OverviewEntry,
  chapter: string,
  scene: string,
  siblings: OverviewEntry[],
): EntryAction[] {
  const nextScene = siblings[siblings.indexOf(entry) + 1]?.scene ?? null;
  const scenes = siblings.filter(
    (candidate) => candidate.kind === "scene" && candidate.scene !== null,
  );
  return [
    dialog("この前にシーンを追加", { kind: "add_scene", chapter, before: scene }),
    dialog("この後にシーンを追加", { kind: "add_scene", chapter, before: nextScene }),
    ...moveActions(entry, scenes, PLAIN_LABELS, (position) => ({
      kind: "move_scene",
      chapter,
      scene,
      position,
    })),
    removal({ kind: "remove_scene", chapter, scene }),
  ];
}

/**
 * 目次の項目に対して、できる構成の操作。何もできない項目は空。
 * `siblings` は、同じ階層の項目の並び（`entry` を含む）。「この後に追加」の位置と、並べ替えの行き先に使う。
 */
export function entryActionsFor(entry: OverviewEntry, siblings: OverviewEntry[]): EntryAction[] {
  if (entry.path !== null) {
    // 人物は ID ではなくパスで指す。ファイル名が ID の規則に合わない資料（`characters/Rin.md` など）も、目次に出ていれば消せる
    if (entry.kind === "character" && isCharacterDocumentPath(entry.path)) {
      return characterActions(entry, entry.path, siblings);
    }
    if (isAdditionalWorldDocumentPath(entry.path)) {
      return [removal({ kind: "remove_world_document", path: entry.path })];
    }
  }

  const { chapter, scene } = entry;
  if (entry.kind === "scene" && chapter !== null && scene !== null) {
    return sceneActions(entry, chapter, scene, siblings);
  }
  if (entry.kind === "chapter" && chapter !== null) {
    // 章立てのファイル（パスを持つ）は章を足す・消す・動かす操作、本文の章見出し（パスを持たない）はシーンを足す操作と章を動かす・消す操作
    if (entry.path !== null) {
      return planChapterActions(entry, chapter, siblings);
    }
    return [
      dialog("シーンを追加", { kind: "add_scene", chapter, before: null }),
      ...chapterMoveActions(entry, chapter, siblings, CHAPTER_LABELS),
      removal({ kind: "remove_chapter", chapter }, "章を削除"),
    ];
  }
  return [];
}
