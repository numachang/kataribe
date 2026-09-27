import { describe, expect, it } from "vitest";
import { sanitizeGenerationNumber } from "./settingsNumbers";

// 期待値は Rust の GenerationSettings::sanitized のテストと同じ範囲にする（画面と生成で値が食い違わないように）
describe("sanitizeGenerationNumber", () => {
  it("範囲の中の値はそのまま", () => {
    expect(sanitizeGenerationNumber("chars_per_call", 1500)).toBe(1500);
    expect(sanitizeGenerationNumber("temperature", 0.7)).toBe(0.7);
  });

  it("範囲を外れた値は、範囲の端に収める", () => {
    expect(sanitizeGenerationNumber("chars_per_call", 50)).toBe(200);
    expect(sanitizeGenerationNumber("chars_per_call", 50000)).toBe(20000);
    expect(sanitizeGenerationNumber("context_tokens", 1000)).toBe(2048);
    expect(sanitizeGenerationNumber("temperature", 3)).toBe(2);
    expect(sanitizeGenerationNumber("quality_retries", 10)).toBe(5);
  });

  it("空欄や不正な入力（NaN）は既定値にする", () => {
    expect(sanitizeGenerationNumber("chars_per_call", Number.NaN)).toBe(1500);
    expect(sanitizeGenerationNumber("temperature", Number.NaN)).toBe(0.8);
  });
});
