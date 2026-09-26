import { describe, expect, it } from "vitest";
import type { GenerationEvent } from "../../api/types";
import { applyGenerationEvent, createEmptyGenerationDisplay } from "./eventAccumulator";

function replay(events: GenerationEvent[]) {
  return events.reduce(applyGenerationEvent, createEmptyGenerationDisplay());
}

describe("applyGenerationEvent", () => {
  it("step_started で新しい段階を追加する", () => {
    const display = replay([
      { kind: "step_started", label: "企画を生成しています", index: 1, total: 1 },
    ]);
    expect(display.steps).toEqual([
      {
        label: "企画を生成しています",
        index: 1,
        total: 1,
        content: "",
        reasoning: "",
        notices: [],
        finished: false,
        promptTokens: null,
        completionTokens: null,
        elapsedMs: null,
      },
    ]);
  });

  it("content は直近の段階に積み上がる", () => {
    const display = replay([
      { kind: "step_started", label: "企画", index: 1, total: 1 },
      { kind: "content", text: "嵐の夜、" },
      { kind: "content", text: "洋館に人々が集まる。" },
    ]);
    expect(display.steps[0]?.content).toBe("嵐の夜、洋館に人々が集まる。");
  });

  it("reasoning は content と別に積み上がる", () => {
    const display = replay([
      { kind: "step_started", label: "企画", index: 1, total: 1 },
      { kind: "reasoning", text: "着想を検討中……" },
      { kind: "content", text: "本文" },
    ]);
    expect(display.steps[0]?.reasoning).toBe("着想を検討中……");
    expect(display.steps[0]?.content).toBe("本文");
  });

  it("notice は一覧に追加される", () => {
    const display = replay([
      { kind: "step_started", label: "本文", index: 1, total: 1 },
      { kind: "notice", level: "info", message: "続きは次回に書き継ぎます。" },
    ]);
    expect(display.steps[0]?.notices).toEqual([
      { level: "info", message: "続きは次回に書き継ぎます。" },
    ]);
  });

  it("step_finished は完了状態とトークン数を記録する", () => {
    const display = replay([
      { kind: "step_started", label: "本文", index: 1, total: 1 },
      { kind: "content", text: "……" },
      { kind: "step_finished", prompt_tokens: 120, completion_tokens: 40, elapsed_ms: 900 },
    ]);
    expect(display.steps[0]).toMatchObject({
      finished: true,
      promptTokens: 120,
      completionTokens: 40,
      elapsedMs: 900,
    });
  });

  it("複数の段階（ビート単位など）を順に積み上げられる", () => {
    const display = replay([
      { kind: "step_started", label: "ビート 1/2", index: 1, total: 2 },
      { kind: "content", text: "最初の展開。" },
      { kind: "step_finished", prompt_tokens: 10, completion_tokens: 5, elapsed_ms: 100 },
      { kind: "step_started", label: "ビート 2/2", index: 2, total: 2 },
      { kind: "content", text: "続きの展開。" },
      { kind: "step_finished", prompt_tokens: 10, completion_tokens: 5, elapsed_ms: 100 },
    ]);
    expect(display.steps).toHaveLength(2);
    expect(display.steps[0]?.content).toBe("最初の展開。");
    expect(display.steps[1]?.content).toBe("続きの展開。");
  });

  it("段階が始まる前のイベントは無視する（防御的）", () => {
    const display = replay([{ kind: "content", text: "迷子のテキスト" }]);
    expect(display.steps).toEqual([]);
  });
});
