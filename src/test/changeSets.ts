import type { ChangeSet, TrashFileChange, WriteFileChange } from "../api/types";

/** 変更案の、書き込みの変更だけを取り出す。 */
export function writeChanges(changeSet: ChangeSet): WriteFileChange[] {
  return changeSet.files.filter((file): file is WriteFileChange => file.kind === "write");
}

/** 変更案の、ゴミ箱へ移す変更だけを取り出す。 */
export function trashChanges(changeSet: ChangeSet): TrashFileChange[] {
  return changeSet.files.filter((file): file is TrashFileChange => file.kind === "trash");
}
