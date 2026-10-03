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
