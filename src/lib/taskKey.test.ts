import { describe, expect, it } from "vitest";
import type { Task } from "../api/types";
import { taskKey } from "./taskKey";

describe("taskKey", () => {
  it("指示の違う人物の追加は、別のキーになる", () => {
    const first: Task = { kind: "add_character", instruction: "幼なじみの医師" };
    const second: Task = { kind: "add_character", instruction: "港の見張り番" };

    expect(taskKey(first)).not.toBe(taskKey(second));
    expect(taskKey(first)).toBe(taskKey({ ...first }));
  });

  it("世界観の資料の追加は、ファイル名か指示が違えば別のキーになる", () => {
    const base: Task = { kind: "add_world_document", name: null, instruction: "天気の言い伝え" };

    expect(taskKey(base)).not.toBe(taskKey({ ...base, name: "weather" }));
    expect(taskKey(base)).not.toBe(taskKey({ ...base, instruction: "地名の由来" }));
  });

  it("同じ指示でも、人物と資料の追加は別のキーになる", () => {
    const instruction = "港町の暮らし";

    expect(taskKey({ kind: "add_character", instruction })).not.toBe(
      taskKey({ kind: "add_world_document", name: null, instruction }),
    );
  });
});
