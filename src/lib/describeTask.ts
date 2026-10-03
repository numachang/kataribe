import type { Task } from "../api/types";

/**
 * 生成の見出しに添える、仕事の説明。進み具合の文言だけでは分からない仕事（指示から人物や資料を作って足す）のときだけ返し、
 * 工程の名前が進み具合に出る仕事は null。
 */
export function describeTask(task: Task): string | null {
  switch (task.kind) {
    case "add_character":
      return "AI に作らせて追加: 人物";
    case "add_world_document":
      return "AI に作らせて追加: 世界観の資料";
    default:
      return null;
  }
}
