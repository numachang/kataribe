import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, it } from "vitest";
import type { Backend } from "../../api/backend";
import { BackendProvider } from "../../api/context";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import type { ChangeSet, NewProject, Task } from "../../api/types";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { useGenerationSession } from "./useGenerationSession";

// 指示から作って足した人物・資料を適用したあとに、その文書を開くこと。

beforeEach(resetAllStores);
afterEach(resetAllStores);

const ADD_CHARACTER: Task = { kind: "add_character", instruction: "主人公の幼なじみ" };

function renderSession(backend: Backend) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <BackendProvider backend={backend}>{children}</BackendProvider>
  );
  return renderHook(() => useGenerationSession(), { wrapper });
}

async function openSample(backend: Backend): Promise<void> {
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
}

it("人物を足す変更案を適用すると、作った人物資料を開く", async () => {
  const backend = createMockBackend({ delayMs: 0 });
  await openSample(backend);
  const { result } = renderSession(backend);

  act(() => result.current.start(ADD_CHARACTER));
  await waitFor(() => expect(result.current.phase).toBe("reviewing"));
  await act(async () => {
    await result.current.apply();
  });

  expect(result.current.phase).toBe("idle");
  expect(useWorkspaceStore.getState().currentPath).toBe("characters/character.md");
});

it("世界観の資料を足す変更案を適用すると、作った資料を開く", async () => {
  const backend = createMockBackend({ delayMs: 0 });
  await openSample(backend);
  const { result } = renderSession(backend);

  act(() =>
    result.current.start({ kind: "add_world_document", name: "weather", instruction: "天気" }),
  );
  await waitFor(() => expect(result.current.phase).toBe("reviewing"));
  await act(async () => {
    await result.current.apply();
  });

  expect(useWorkspaceStore.getState().currentPath).toBe("world/weather.md");
});

it("適用に失敗したときは、作った文書を開かず、変更案を残す", async () => {
  const inner = createMockBackend({ delayMs: 0 });
  await openSample(inner);
  const backend = wrapBackend(inner, {
    async applyChangeSet() {
      throw new Error("ディスクがいっぱいです");
    },
  });
  const { result } = renderSession(backend);

  act(() => result.current.start(ADD_CHARACTER));
  await waitFor(() => expect(result.current.phase).toBe("reviewing"));
  await act(async () => {
    await result.current.apply();
  });

  expect(result.current.phase).toBe("reviewing");
  expect(useWorkspaceStore.getState().currentPath).toBeNull();
});

it("開いている文書が改名される変更案では、作った文書を開かず、改名された文書のまま続ける", async () => {
  const inner = createMockBackend({ delayMs: 0 });
  await openSample(inner);
  const renaming: ChangeSet = {
    summary: "人物「高橋 美咲」を生成しました（characters/character.md）。",
    project_root: SAMPLE_PROJECT_FOLDER,
    files: [
      { kind: "move", from: "plot/chapters/02.md", to: "plot/chapters/03.md" },
      {
        kind: "write",
        path: "characters/character.md",
        content: "内容",
        previous: null,
        base_hash: null,
      },
    ],
  };
  const backend = wrapBackend(inner, {
    generate: async () => renaming,
    // 改名の中身は偽バックエンドの適用に任せず、画面の振る舞いだけを確かめる
    applyChangeSet: async () => inner.overview(),
  });
  useWorkspaceStore.getState().openDocument("plot/chapters/02.md");
  const { result } = renderSession(backend);

  act(() => result.current.start(ADD_CHARACTER));
  await waitFor(() => expect(result.current.phase).toBe("reviewing"));
  await act(async () => {
    await result.current.apply();
  });

  expect(useWorkspaceStore.getState().currentPath).toBe("plot/chapters/03.md");
});

it("自動で進めるときは、工程が文書を作っても、文書を開かない", async () => {
  const backend = createMockBackend({ delayMs: 0 });
  const newProject: NewProject = {
    title: "作品A",
    author: null,
    genre: "fantasy",
    genre_note: null,
    rating: "general",
    target_length: 10000,
    idea: "旅する少女の物語",
  };
  useWorkspaceStore
    .getState()
    .openWorkspace(await backend.createProject("C:\\projects\\A", newProject));
  const { result } = renderSession(backend);

  act(() => result.current.runAutoAdvance());
  await waitFor(() => expect(result.current.autoAdvancing).toBe(true));
  await waitFor(() => expect(result.current.autoAdvancing).toBe(false), { timeout: 10000 });

  const pipeline = await backend.pipeline();
  expect(pipeline.filter((step) => step.state === "done").length).toBeGreaterThan(0);
  expect(useWorkspaceStore.getState().currentPath).toBeNull();
});
