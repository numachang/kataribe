import type { Task } from "../api/types";

/** Task を一意な文字列に変換する。React のリストキーなどに使う。 */
export function taskKey(task: Task): string {
  switch (task.kind) {
    case "character":
      return `character:${task.id}`;
    case "scene_plan":
      return `scene_plan:${task.chapter}`;
    case "draft":
      return `draft:${task.chapter}:${task.scene}`;
    case "revise":
      return `revise:${task.path}`;
    default:
      return task.kind;
  }
}
