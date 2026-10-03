import type { ChangeSet, FileChange } from "../api/types";

/** `path` が `folder` そのもの、またはその中のパスか。 */
function isWithin(path: string, folder: string): boolean {
  return path === folder || path.startsWith(`${folder}/`);
}

/**
 * `path` を、移動 `from` → `to` のあとのパスにする。`from` がフォルダなら、その下のパスは前の部分を置き換えて求める。
 * `from` の中のパスでなければ null。
 */
function movedBy(path: string, from: string, to: string): string | null {
  if (!isWithin(path, from)) {
    return null;
  }
  return `${to}${path.slice(from.length)}`;
}

function isTouchedBy(file: FileChange, path: string): boolean {
  switch (file.kind) {
    case "write":
      return file.path === path;
    case "trash":
      return isWithin(path, file.path);
    case "move":
      return isWithin(path, file.from);
    case "expect":
      return false;
  }
}

/**
 * 変更案が `path` の文書に触れるか（書き換え・ゴミ箱へ移す・移動、のどれでも。フォルダの下の文書も含む）。
 * 状態を確かめるだけの変更（Expect）は何も変えないので、触れるものに数えない。
 */
export function touchesPath(changeSet: ChangeSet, path: string): boolean {
  return changeSet.files.some((file) => isTouchedBy(file, path));
}

/** 変更案が `path` の文書の内容を書き換えるか。移動やゴミ箱へ移すだけなら false。 */
export function rewritesPath(changeSet: ChangeSet, path: string): boolean {
  return changeSet.files.some((file) => file.kind === "write" && file.path === path);
}

/**
 * 変更案を適用したあと、`path` の文書がどこへ行くか。
 * 動かない（書き換えるだけの場合も）なら undefined、ゴミ箱へ移るなら null、改名されるなら新しいパス。
 * 文書を開いたままの画面が、読み直すか・閉じるか・パスを付け替えて続けるかを決めるのに使う。
 * 移動は同時に起きるので、章の番号をずらす連鎖（03 → 04、04 → 05）でも、1 回分だけ動く。
 */
export function relocatedPath(changeSet: ChangeSet, path: string): string | null | undefined {
  for (const file of changeSet.files) {
    if (file.kind === "trash" && isWithin(path, file.path)) {
      return null;
    }
    if (file.kind === "move") {
      const moved = movedBy(path, file.from, file.to);
      if (moved !== null) {
        return moved;
      }
    }
  }
  return undefined;
}

/**
 * 変更案を適用すると、`openPath`（今エディタで開いている文書。無ければ null）が改名されるか。
 * 改名される文書は、パスを付け替えて開いたまま続けるので、適用で作った文書を開いてその編集を中断させてはならない。
 */
export function renamesOpenDocument(changeSet: ChangeSet, openPath: string | null): boolean {
  return openPath !== null && typeof relocatedPath(changeSet, openPath) === "string";
}

/** 変更案の一覧で、1 つの変更を見分けるための key。 */
export function changeKey(file: FileChange): string {
  switch (file.kind) {
    case "move":
      return `move:${file.from}->${file.to}`;
    default:
      return `${file.kind}:${file.path}`;
  }
}
