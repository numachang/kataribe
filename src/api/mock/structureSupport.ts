import { BackendError } from "../backend";
import type { FileChange, StructurePlan } from "../types";
import { chapterPath } from "./paths";
import type { MockChapter, ProjectState } from "./state";
import { findChapter } from "./state";

// 構成の操作の変更案を作る関数が共有する小さな部品。

export function invalidInput(message: string): BackendError {
  return new BackendError("invalid_input", message);
}

export function notFound(message: string): BackendError {
  return new BackendError("not_found", message);
}

export function chapterNumber(chapterId: string): number {
  return Number.parseInt(chapterId, 10);
}

export function requireChapter(state: ProjectState, chapterId: string): MockChapter {
  const chapter = findChapter(state, chapterId);
  if (chapter === null) {
    throw notFound(`第${chapterNumber(chapterId)}章（${chapterPath(chapterId)}）がありません。`);
  }
  return chapter;
}

/**
 * 変更の言い回し（「ます」「ました」の前まで）から、適用する前の説明（「〜します。」）と
 * 適用したあとの知らせ（「〜しました。」）を作る（本物の Wording と同じ）。
 */
export function described(
  state: ProjectState,
  stem: string,
  files: FileChange[],
): Pick<StructurePlan, "change_set" | "completed_summary"> {
  return {
    change_set: { summary: `${stem}ます。`, files, project_root: state.folder },
    completed_summary: `${stem}ました。`,
  };
}

/**
 * 項目を動かせる位置か確かめる（本物の `ensure_new_position` と同じ）。範囲の外と、今と同じ位置は、利用者が直せる入力の誤り。
 * 今と同じ位置を空の変更案にしないのは、何も起きない操作を成功として返すと、呼び出し側の数え間違いが見えなくなるため。
 * `subject` は「人物「霧島 凛」」のような呼び方、`unit` は「人物」「章」「シーン」のような数え方の名前。
 */
export function ensureNewPosition(
  subject: string,
  unit: string,
  current: number,
  position: number,
  count: number,
): void {
  if (!Number.isInteger(position) || position < 0 || position >= count) {
    throw invalidInput(
      `${subject}を ${position + 1} 番目へは移せません（${unit}は全部で ${count} 件です）。`,
    );
  }
  if (position === current) {
    throw invalidInput(`${subject}はすでに ${position + 1} 番目です。`);
  }
}

/** `items` の `from` 番目を、並べ替えたあとに `position` 番目に来るよう動かした、新しい並び。 */
export function movedItem<T>(items: T[], from: number, position: number): T[] {
  return items.toSpliced(from, 1).toSpliced(position, 0, ...items.slice(from, from + 1));
}
