import { describe, expect, it } from "vitest";
import { parseRubySegments, stripRubyNotation } from "./ruby";

describe("parseRubySegments", () => {
  it("空文字列には何も返さない", () => {
    expect(parseRubySegments("")).toEqual([]);
  });

  it("記法のない文章はひとつの text セグメントになる", () => {
    expect(parseRubySegments("今日はいい天気だ。")).toEqual([
      { kind: "text", text: "今日はいい天気だ。" },
    ]);
  });

  it("漢字の連続に続く《よみ》を自動ルビとして解析する", () => {
    expect(parseRubySegments("霧島《きりしま》凛")).toEqual([
      { kind: "ruby", base: "霧島", reading: "きりしま" },
      { kind: "text", text: "凛" },
    ]);
  });

  it("｜親《よみ》で親文字を明示できる", () => {
    expect(parseRubySegments("｜わたし《ヒロイン》は驚いた")).toEqual([
      { kind: "ruby", base: "わたし", reading: "ヒロイン" },
      { kind: "text", text: "は驚いた" },
    ]);
  });

  it("半角の | でも明示ルビとして解析する", () => {
    expect(parseRubySegments("|海月《くらげ》")).toEqual([
      { kind: "ruby", base: "海月", reading: "くらげ" },
    ]);
  });

  it("《《傍点》》を傍点セグメントとして解析する", () => {
    expect(parseRubySegments("それは《《嘘》》だ")).toEqual([
      { kind: "text", text: "それは" },
      { kind: "emphasis", text: "嘘" },
      { kind: "text", text: "だ" },
    ]);
  });

  it("｜《 は記法を解除し、《 をそのまま出力する", () => {
    expect(parseRubySegments("｜《これは引用》")).toEqual([
      { kind: "text", text: "《これは引用》" },
    ]);
  });

  it("複数の記法が混在していても順番どおりに解析する", () => {
    const segments = parseRubySegments("霧島《きりしま》は「《《嘘》》」と言った。");
    expect(segments).toEqual([
      { kind: "ruby", base: "霧島", reading: "きりしま" },
      { kind: "text", text: "は「" },
      { kind: "emphasis", text: "嘘" },
      { kind: "text", text: "」と言った。" },
    ]);
  });
});

describe("stripRubyNotation", () => {
  it("読みを取り除き、親文字と地の文だけを残す", () => {
    expect(stripRubyNotation("霧島《きりしま》は｜わたし《ヒロイン》と名乗った。")).toBe(
      "霧島はわたしと名乗った。",
    );
  });

  it("傍点の文字はそのまま残す", () => {
    expect(stripRubyNotation("それは《《嘘》》だ")).toBe("それは嘘だ");
  });
});
