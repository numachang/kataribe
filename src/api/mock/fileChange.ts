import { hashText } from "../../lib/hash";
import { computeTextStats } from "../../lib/textStats";
import { BackendError } from "../backend";
import type { FileChange, TrashedFile } from "../types";
import { chapterTextDir, scenePath } from "./paths";
import { readMockFile } from "./render";
import type { ProjectState } from "./state";
import { findChapter } from "./state";

// 変更案の 1 ファイルぶん（FileChange）を、状態から作る。生成と構成の操作が共有する。

/** `path` を `content` にする変更。今のファイルがあれば、それを変更前（と競合の基準）にする。 */
export function writeChange(state: ProjectState, path: string, content: string): FileChange {
  const previous = readMockFile(state, path);
  return {
    kind: "write",
    path,
    content,
    previous,
    base_hash: previous !== null ? hashText(previous) : null,
  };
}

/** `path` のファイルをゴミ箱へ移す変更。ファイルが無ければ not_found。 */
export function trashChange(state: ProjectState, path: string): FileChange {
  const content = readMockFile(state, path);
  if (content === null) {
    throw new BackendError("not_found", `「${path}」がありません。`);
  }
  return {
    kind: "trash",
    path,
    files: [{ path, base_hash: hashText(content), chars: computeTextStats(content).chars }],
  };
}

/** 章の本文のフォルダの中のファイル（本文のあるシーンだけ）。フォルダが無ければ（本文が 1 つも無ければ）空。 */
export function chapterTextFiles(state: ProjectState, chapterId: string): TrashedFile[] {
  const scenes = findChapter(state, chapterId)?.scenes ?? [];
  return scenes.flatMap((scene) => {
    if (scene.draft === null) {
      return [];
    }
    return [
      {
        path: scenePath(chapterId, scene.id),
        base_hash: hashText(scene.draft),
        chars: computeTextStats(scene.draft).chars,
      },
    ];
  });
}

/** 章の本文のフォルダを、中のファイル全部ごとゴミ箱へ移す変更。フォルダが無ければ null。 */
export function trashChapterTextChange(state: ProjectState, chapterId: string): FileChange | null {
  const files = chapterTextFiles(state, chapterId);
  return files.length === 0 ? null : { kind: "trash", path: chapterTextDir(chapterId), files };
}

export function moveChange(from: string, to: string): FileChange {
  return { kind: "move", from, to };
}

/** 適用のときに `path` が何も無い状態であることを確かめる変更。 */
export function expectAbsentChange(path: string): FileChange {
  return { kind: "expect", path, base_hash: null };
}
