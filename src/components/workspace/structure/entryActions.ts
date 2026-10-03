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
 * 「上へ移す」「下へ移す」。`edit` は、並べ替えたあとに一覧の何番目（0 始まり）に来るかから、操作を作る。
 * `list` は、動かす項目と同じ種類の項目を目次の並びで並べたもの（本物が数える一覧と同じ）で、`index` はその中の動かす項目の位置。
 * 動かせない項目（読めない人物資料）は飛ばして隣の動かせる項目と入れ替え、動かせる隣が無い側は出さない。
 * 動かす項目自身が動かせないとき（`index` が一覧の外のときも）は何も出さない。
 */
function moveActions(
  list: OverviewEntry[],
  index: number,
  labels: { up: string; down: string },
  edit: (position: number) => MoveEdit,
  isMovable: (entry: OverviewEntry) => boolean = () => true,
): EntryAction[] {
  const moving = list[index];
  if (moving === undefined || !isMovable(moving)) {
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
      characters,
      characters.indexOf(entry),
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
  { siblings, planChapters }: EntryActionContext,
): EntryAction[] {
  const nextChapter = siblings[siblings.indexOf(entry) + 1]?.chapter ?? null;
  return [
    dialog("この前に章を追加", { kind: "add_chapter", before: chapter }),
    dialog("この後に章を追加", { kind: "add_chapter", before: nextChapter }),
    ...chapterMoveActions(chapter, planChapters, PLAIN_LABELS),
    removal({ kind: "remove_chapter", chapter }),
  ];
}

/**
 * 章を上へ・下へ移す操作。本物は、読めない章立ても含めた全部の章（番号順）の中で位置を数えるので、
 * 本文の節（読めない章立ては出ない）ではなく、プロットの節の章の一覧で数える。
 */
function chapterMoveActions(
  chapter: string,
  planChapters: OverviewEntry[],
  labels: { up: string; down: string },
): EntryAction[] {
  return moveActions(
    planChapters,
    planChapters.findIndex((candidate) => candidate.chapter === chapter),
    labels,
    (position) => ({ kind: "move_chapter", chapter, position }),
  );
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
    ...moveActions(scenes, scenes.indexOf(entry), PLAIN_LABELS, (position) => ({
      kind: "move_scene",
      chapter,
      scene,
      position,
    })),
    removal({ kind: "remove_scene", chapter, scene }),
  ];
}

/** 目次の項目の操作を決めるのに必要な、周りの項目。 */
export interface EntryActionContext {
  /** 同じ階層の項目の並び（その項目を含む）。「この後に追加」の位置と、人物・シーンの並べ替えの行き先に使う。 */
  siblings: OverviewEntry[];
  /** プロットの節の章（YAML が読めない章立ても含む）。本文の章見出しからも、章の並べ替えの行き先をここで数える。 */
  planChapters: OverviewEntry[];
}

/** 目次の項目に対して、できる構成の操作。何もできない項目は空。 */
export function entryActionsFor(entry: OverviewEntry, context: EntryActionContext): EntryAction[] {
  const { siblings } = context;
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
      return planChapterActions(entry, chapter, context);
    }
    return [
      dialog("シーンを追加", { kind: "add_scene", chapter, before: null }),
      ...chapterMoveActions(chapter, context.planChapters, CHAPTER_LABELS),
      removal({ kind: "remove_chapter", chapter }, "章を削除"),
    ];
  }
  return [];
}
