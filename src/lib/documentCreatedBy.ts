import type { ChangeSet, Task } from "../api/types";

/**
 * `task` の変更案を適用したあとに開く、新しく作った文書のパス。開くものが無ければ null。
 * 開くのは、指示から作って足した人物・世界観の資料だけ（変更案の、新規の書き込み）。
 * 工程の生成は、利用者がいま見ている文書とは別の文書を作ることがあり、勝手に切り替えると編集を中断させるので開かない。
 */
export function documentCreatedBy(task: Task, changeSet: ChangeSet): string | null {
  switch (task.kind) {
    case "add_character":
    case "add_world_document":
      return newlyWrittenPath(changeSet);
    default:
      return null;
  }
}

function newlyWrittenPath(changeSet: ChangeSet): string | null {
  for (const file of changeSet.files) {
    if (file.kind === "write" && file.previous === null) {
      return file.path;
    }
  }
  return null;
}
