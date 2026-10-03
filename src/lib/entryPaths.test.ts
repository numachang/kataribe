import { describe, expect, it } from "vitest";
import { characterIdOfPath, isAdditionalWorldDocumentPath } from "./entryPaths";

describe("characterIdOfPath", () => {
  it("人物資料のパスから id を取り出す", () => {
    expect(characterIdOfPath("characters/kirishima-rin.md")).toBe("kirishima-rin");
  });

  it("人物資料でないパスは null", () => {
    expect(characterIdOfPath("world/overview.md")).toBeNull();
    expect(characterIdOfPath("characters/kirishima-rin.txt")).toBeNull();
  });
});

describe("isAdditionalWorldDocumentPath", () => {
  it("world/ 直下の Markdown は足した資料", () => {
    expect(isAdditionalWorldDocumentPath("world/glossary.md")).toBe(true);
    expect(isAdditionalWorldDocumentPath("world/用語集.md")).toBe(true);
  });

  it("世界観の概要と、サブフォルダの下・ほかの場所のファイルは違う", () => {
    expect(isAdditionalWorldDocumentPath("world/overview.md")).toBe(false);
    expect(isAdditionalWorldDocumentPath("world/sub/x.md")).toBe(false);
    expect(isAdditionalWorldDocumentPath("concept.md")).toBe(false);
  });
});
