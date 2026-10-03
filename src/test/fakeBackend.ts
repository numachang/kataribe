import { vi } from "vitest";
import type { Backend } from "../api/backend";

function notImplemented(name: string) {
  return vi.fn(async () => {
    throw new Error(`テストスタブ: ${name} は呼ばれない想定です`);
  });
}

/**
 * テスト用の Backend スタブを作る。
 * 渡した `overrides` だけが実際に呼ばれる想定で、それ以外を呼ぶとテストが失敗する。
 */
export function createStubBackend(overrides: Partial<Backend> = {}): Backend {
  const base: Backend = {
    loadSettings: notImplemented("loadSettings"),
    saveSettings: notImplemented("saveSettings"),
    loadProjectSettings: notImplemented("loadProjectSettings"),
    saveProjectSettings: notImplemented("saveProjectSettings"),
    setApiKey: notImplemented("setApiKey"),
    hasApiKey: notImplemented("hasApiKey"),
    listModels: notImplemented("listModels"),
    listGenres: notImplemented("listGenres"),
    pickFolder: notImplemented("pickFolder"),
    createProject: notImplemented("createProject"),
    openProject: notImplemented("openProject"),
    closeProject: notImplemented("closeProject"),
    overview: notImplemented("overview"),
    pipeline: notImplemented("pipeline"),
    readDocument: notImplemented("readDocument"),
    writeDocument: notImplemented("writeDocument"),
    parseDocument: notImplemented("parseDocument"),
    textStats: notImplemented("textStats"),
    parseRuby: notImplemented("parseRuby"),
    analyzeQuality: notImplemented("analyzeQuality"),
    generate: notImplemented("generate"),
    cancelGeneration: notImplemented("cancelGeneration"),
    applyChangeSet: notImplemented("applyChangeSet"),
  };
  return { ...base, ...overrides };
}
