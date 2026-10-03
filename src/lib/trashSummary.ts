import type { ChangeSet, TrashedFile } from "../api/types";

const MANUSCRIPT_FOLDER = "manuscript/";

/** 変更案がゴミ箱へ移すファイルを、すべて並べる。 */
export function trashedFilesOf(changeSet: ChangeSet): TrashedFile[] {
  return changeSet.files.flatMap((file) => (file.kind === "trash" ? file.files : []));
}

/** ゴミ箱へ移るファイルのうち、本文（原稿）の分。何が失われるかを強調して見せるために数える。 */
export function manuscriptAmong(files: TrashedFile[]): { count: number; chars: number } {
  const manuscript = files.filter((file) => file.path.startsWith(MANUSCRIPT_FOLDER));
  return { count: manuscript.length, chars: manuscript.reduce((sum, file) => sum + file.chars, 0) };
}
