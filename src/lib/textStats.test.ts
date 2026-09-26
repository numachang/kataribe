import { describe, expect, it } from "vitest";
import { computeTextStats } from "./textStats";

describe("computeTextStats", () => {
  it("空文字列は 0 文字", () => {
    expect(computeTextStats("")).toEqual({
      chars: 0,
      paragraphs: 0,
      dialogue_lines: 0,
      manuscript_pages: 0,
    });
  });

  it("空白・改行・ルビの読みを除いて文字数を数える", () => {
    const stats = computeTextStats("霧島《きりしま》は\n　振り返った。");
    // 「霧島は振り返った。」で 9 文字（全角空白の字下げも除く）
    expect(stats.chars).toBe(9);
  });

  it("「または『で始まる行を会話行として数える", () => {
    const stats = computeTextStats("「おはよう」\n地の文。\n『こんにちは』");
    expect(stats.paragraphs).toBe(3);
    expect(stats.dialogue_lines).toBe(2);
  });

  it("400 字で 1 枚として原稿用紙換算する", () => {
    const stats = computeTextStats("あ".repeat(800));
    expect(stats.manuscript_pages).toBe(2);
  });
});
