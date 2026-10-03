import type {
  ChangeSet,
  ExpectFileChange,
  MoveFileChange,
  TrashFileChange,
  WriteFileChange,
} from "../api/types";

/** 変更案の、書き込みの変更だけを取り出す。 */
export function writeChanges(changeSet: ChangeSet): WriteFileChange[] {
  return changeSet.files.filter((file): file is WriteFileChange => file.kind === "write");
}

/** 変更案の、ゴミ箱へ移す変更だけを取り出す。 */
export function trashChanges(changeSet: ChangeSet): TrashFileChange[] {
  return changeSet.files.filter((file): file is TrashFileChange => file.kind === "trash");
}

/** 変更案の、移動の変更だけを取り出す。 */
export function moveChanges(changeSet: ChangeSet): MoveFileChange[] {
  return changeSet.files.filter((file): file is MoveFileChange => file.kind === "move");
}

/** 変更案の、状態を確かめるだけの変更を取り出す。 */
export function expectChanges(changeSet: ChangeSet): ExpectFileChange[] {
  return changeSet.files.filter((file): file is ExpectFileChange => file.kind === "expect");
}
