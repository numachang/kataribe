import { beforeEach, describe, expect, it } from "vitest";
import type { PipelineStep, ProjectOverview } from "../api/types";
import { createStubBackend } from "../test/fakeBackend";
import { useWorkspaceStore } from "./workspaceStore";

const OVERVIEW: ProjectOverview = {
  root: "C:\\projects\\test",
  title: "テスト作品",
  target_length: 1000,
  total_chars: 0,
  sections: [],
};

const PIPELINE: PipelineStep[] = [
  { task: { kind: "concept" }, label: "企画", state: "ready", blocked_by: null },
];

function reset(): void {
  useWorkspaceStore.getState().closeWorkspace();
}

describe("useWorkspaceStore", () => {
  beforeEach(reset);

  it("openWorkspace は目次を保持し、開いている文書を持たない", () => {
    useWorkspaceStore.getState().openWorkspace(OVERVIEW);
    expect(useWorkspaceStore.getState().overview).toEqual(OVERVIEW);
    expect(useWorkspaceStore.getState().currentPath).toBeNull();
  });

  it("openDocument で選択中のパスが変わる", () => {
    useWorkspaceStore.getState().openWorkspace(OVERVIEW);
    useWorkspaceStore.getState().openDocument("concept.md");
    expect(useWorkspaceStore.getState().currentPath).toBe("concept.md");
  });

  it("closeWorkspace ですべてリセットされる", () => {
    useWorkspaceStore.getState().openWorkspace(OVERVIEW);
    useWorkspaceStore.getState().openDocument("concept.md");
    useWorkspaceStore.getState().closeWorkspace();

    expect(useWorkspaceStore.getState().overview).toBeNull();
    expect(useWorkspaceStore.getState().currentPath).toBeNull();
    expect(useWorkspaceStore.getState().pipeline).toEqual([]);
  });

  it("refreshOverview は Backend から取得した目次で置き換える", async () => {
    const backend = createStubBackend({ overview: async () => OVERVIEW });
    await useWorkspaceStore.getState().refreshOverview(backend);
    expect(useWorkspaceStore.getState().overview).toEqual(OVERVIEW);
  });

  it("refreshPipeline は Backend から取得した工程で置き換える", async () => {
    const backend = createStubBackend({ pipeline: async () => PIPELINE });
    await useWorkspaceStore.getState().refreshPipeline(backend);
    expect(useWorkspaceStore.getState().pipeline).toEqual(PIPELINE);
  });
});
