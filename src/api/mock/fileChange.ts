import { hashText } from "../../lib/hash";
import { computeTextStats } from "../../lib/textStats";
import { BackendError } from "../backend";
import type { FileChange } from "../types";
import { readMockFile } from "./render";
import type { ProjectState } from "./state";

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
