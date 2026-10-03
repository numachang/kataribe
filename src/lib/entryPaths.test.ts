import { describe, expect, it } from "vitest";
import {
  additionalWorldDocumentNameOfPath,
  characterIdOfPath,
  isAdditionalWorldDocumentPath,
  isCharacterDocumentPath,
} from "./entryPaths";

describe("characterIdOfPath", () => {
  it("人物資料のパスから id を取り出す", () => {
    expect(characterIdOfPath("characters/kirishima-rin.md")).toBe("kirishima-rin");
  });

  it("ファイル名が人物 ID の規則に合わない資料は null（生成の対象にできない）", () => {
    expect(characterIdOfPath("characters/Rin.md")).toBeNull();
    expect(characterIdOfPath("characters/凛.md")).toBeNull();
  });

  it("人物資料でないパスは null", () => {
    expect(characterIdOfPath("world/overview.md")).toBeNull();
    expect(characterIdOfPath("characters/kirishima-rin.txt")).toBeNull();
  });
});

describe("isCharacterDocumentPath", () => {
  it("characters/ 直下の Markdown は、ファイル名が ID の規則に合わなくても人物資料", () => {
    expect(isCharacterDocumentPath("characters/kirishima-rin.md")).toBe(true);
    expect(isCharacterDocumentPath("characters/Rin.md")).toBe(true);
    expect(isCharacterDocumentPath("characters/凛.md")).toBe(true);
  });

  it("サブフォルダの下・ほかの拡張子・ほかの場所は違う", () => {
    expect(isCharacterDocumentPath("characters/sub/rin.md")).toBe(false);
    expect(isCharacterDocumentPath("characters/rin.txt")).toBe(false);
    expect(isCharacterDocumentPath("world/rin.md")).toBe(false);
  });
});

describe("isAdditionalWorldDocumentPath", () => {
  it("world/ 直下の Markdown は足した資料", () => {
    expect(isAdditionalWorldDocumentPath("world/glossary.md")).toBe(true);
    expect(isAdditionalWorldDocumentPath("world/用語集.md")).toBe(true);
  });

  it("世界観の概要と、サブフォルダの下・ほかの場所のファイルは違う", () => {
    expect(isAdditionalWorldDocumentPath("world/overview.md")).toBe(false);
    expect(isAdditionalWorldDocumentPath("world/Overview.md")).toBe(false);
    expect(isAdditionalWorldDocumentPath("world/sub/x.md")).toBe(false);
    expect(isAdditionalWorldDocumentPath("concept.md")).toBe(false);
  });
});

describe("additionalWorldDocumentNameOfPath", () => {
  it("足した資料のパスから name を取り出す", () => {
    expect(additionalWorldDocumentNameOfPath("world/glossary.md")).toBe("glossary");
  });

  it("概要・サブフォルダの下・ほかの場所は null", () => {
    expect(additionalWorldDocumentNameOfPath("world/overview.md")).toBeNull();
    expect(additionalWorldDocumentNameOfPath("world/Overview.md")).toBeNull();
    expect(additionalWorldDocumentNameOfPath("world/sub/glossary.md")).toBeNull();
    expect(additionalWorldDocumentNameOfPath("characters/glossary.md")).toBeNull();
  });
});
