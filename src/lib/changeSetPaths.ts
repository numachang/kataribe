import type { ChangeSet, FileChange } from "../api/types";

/** 変更案が `path` の文書に触れるか（書き換え・ゴミ箱へ移す、のどちらでも）。 */
export function touchesPath(changeSet: ChangeSet, path: string): boolean {
  return changeSet.files.some((file) => file.path === path);
}

/**
 * 変更案を適用したあと、`path` の文書がどこへ行くか。
 * 触れないなら（書き換えるだけの場合も）undefined、ゴミ箱へ移るなら null。
 * 文書を開いたままの画面が、読み直すか閉じるかを決めるのに使う。
 */
export function relocatedPath(changeSet: ChangeSet, path: string): string | null | undefined {
  const movedToTrash = changeSet.files.some((file) => file.kind === "trash" && file.path === path);
  return movedToTrash ? null : undefined;
}

/** 変更案の一覧で、1 つの変更を見分けるための key。 */
export function changeKey(file: FileChange): string {
  return `${file.kind}:${file.path}`;
}
