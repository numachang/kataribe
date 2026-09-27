import { describe, expect, it } from "vitest";
import { analyzeQualityText } from "./quality";

describe("analyzeQualityText", () => {
  it("問題のない短い文章では致命的な問題を報告しない", () => {
    const report = analyzeQualityText(
      "「おはよう」と彼女は微笑んだ。窓の外では雨が静かに降っていた。",
      null,
    );
    expect(report.issues.filter((issue) => issue.severity === "error")).toEqual([]);
  });

  it("メタ発言の混入を検出する", () => {
    const report = analyzeQualityText("以下は本文です。\n彼は歩き出した。", null);
    expect(report.issues.some((issue) => issue.kind === "meta_commentary")).toBe(true);
  });

  it("Markdown の見出しを検出する", () => {
    const report = analyzeQualityText("# 第一章\n彼は歩き出した。", null);
    expect(report.issues.some((issue) => issue.kind === "markdown_artifact")).toBe(true);
  });

  it("「と」の数が合わないとき unbalanced_brackets を報告する", () => {
    const report = analyzeQualityText("「おはよう。彼は答えなかった。", null);
    expect(report.issues.some((issue) => issue.kind === "unbalanced_brackets")).toBe(true);
  });

  it("文末が同じ形で連続すると monotonous_endings を報告する", () => {
    const text = "彼は歩いた。彼は止まった。彼は振り返った。彼は走った。彼は叫んだ。";
    const report = analyzeQualityText(text, null);
    expect(report.issues.some((issue) => issue.kind === "monotonous_endings")).toBe(true);
  });

  it("目標文字数に対して短すぎると too_short を報告する", () => {
    const report = analyzeQualityText("短い文章。", 1000);
    expect(report.issues.some((issue) => issue.kind === "too_short")).toBe(true);
  });

  it("同じ文が繰り返されると repeated_sentence を報告する", () => {
    const text = "彼女は振り返った。彼女は微笑んだ。彼女は振り返った。";
    const report = analyzeQualityText(text, null);
    expect(report.issues.some((issue) => issue.kind === "repeated_sentence")).toBe(true);
  });
});
