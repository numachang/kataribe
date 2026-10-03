import { describe, expect, it } from "vitest";
import { describeTask } from "./describeTask";

describe("describeTask", () => {
  it("指示から作って足す仕事は、何を作るのかを説明する", () => {
    expect(describeTask({ kind: "add_character", instruction: "幼なじみ" })).toBe(
      "AI に作らせて追加: 人物",
    );
    expect(describeTask({ kind: "add_world_document", name: null, instruction: "天気" })).toBe(
      "AI に作らせて追加: 世界観の資料",
    );
  });

  it("工程の生成と書き直しには、説明を添えない", () => {
    expect(describeTask({ kind: "concept" })).toBeNull();
    expect(describeTask({ kind: "revise", path: "concept.md", instruction: "短く" })).toBeNull();
  });
});
