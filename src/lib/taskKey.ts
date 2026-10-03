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
    // 指示が違えば別の仕事。種類だけで区別すると、指示の違う追加が同じキーになる
    case "add_character":
      return `add_character:${task.instruction}`;
    case "add_world_document":
      return `add_world_document:${task.name ?? ""}:${task.instruction}`;
    default:
      return task.kind;
  }
}
