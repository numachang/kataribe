import { describe, expect, it } from "vitest";
import type { GenerationSettings, LlmSettings } from "../api/types";
import {
  applyProjectSettings,
  countProjectSettings,
  isSameProjectSettings,
  withProjectSetting,
} from "./projectSettings";

const APP_LLM: LlmSettings = {
  provider: "openai_compatible",
  base_url: "http://localhost:1234/v1",
  model: "gemma",
  claude_command: "claude",
  claude_model: "sonnet",
};

const APP_GENERATION: GenerationSettings = {
  draft_unit: "beat",
  chars_per_call: 1500,
  context_tokens: 16384,
  temperature: 0.8,
  polish: false,
  quality_retries: 1,
  disable_thinking: true,
};

describe("applyProjectSettings", () => {
  it("作品の設定で書いてある項目だけを上書きする", () => {
    const applied = applyProjectSettings(
      { llm: APP_LLM, generation: APP_GENERATION },
      { provider: "claude_code", claude_model: "haiku", context_tokens: 100000 },
    );

    expect(applied.llm).toEqual({ ...APP_LLM, provider: "claude_code", claude_model: "haiku" });
    expect(applied.generation).toEqual({ ...APP_GENERATION, context_tokens: 100000 });
  });

  it("作品の設定が空なら、アプリ全体の設定のまま", () => {
    const base = { llm: APP_LLM, generation: APP_GENERATION };

    expect(applyProjectSettings(base, {})).toEqual(base);
  });

  it("false や 0 も、書いてある値として上書きする", () => {
    const applied = applyProjectSettings(
      { llm: APP_LLM, generation: { ...APP_GENERATION, polish: true } },
      { polish: false, quality_retries: 0 },
    );

    expect(applied.generation.polish).toBe(false);
    expect(applied.generation.quality_retries).toBe(0);
  });
});

describe("isSameProjectSettings", () => {
  it("項目の順番が違っても、同じ項目と値なら同じとみなす", () => {
    expect(
      isSameProjectSettings(
        { provider: "claude_code", polish: true },
        { polish: true, provider: "claude_code" },
      ),
    ).toBe(true);
  });

  it("片方にしか無い項目があれば違うとみなす", () => {
    expect(isSameProjectSettings({ polish: true }, {})).toBe(false);
  });
});

describe("countProjectSettings", () => {
  it("この版が知っている項目だけを数える", () => {
    const received = { polish: true, future_option: "新しい版の設定" } as never;

    expect(countProjectSettings(received)).toBe(1);
  });
});

describe("withProjectSetting", () => {
  it("undefined を渡すと項目ごと消す", () => {
    const next = withProjectSetting({ polish: true, temperature: 0.5 }, "polish", undefined);

    expect(next).toEqual({ temperature: 0.5 });
    expect("polish" in next).toBe(false);
  });

  it("元の設定は書き換えない", () => {
    const original = { polish: true };

    withProjectSetting(original, "temperature", 0.5);

    expect(original).toEqual({ polish: true });
  });
});
