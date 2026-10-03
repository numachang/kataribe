import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { BackendProvider } from "../../api/context";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { readText, textDocument } from "../../test/documents";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { useDocumentEditor } from "./useDocumentEditor";

beforeEach(resetAllStores);
afterEach(resetAllStores);

it("保存中に入力してすぐ別の文書へ切り替えても、前の文書の最後の入力が保存される", async () => {
  const inner = createMockBackend({ delayMs: 0 });
  useWorkspaceStore.getState().openWorkspace(await inner.openProject(SAMPLE_PROJECT_FOLDER));
  const styleBefore = await readText(inner, "style.md");

  // 最初の保存だけ、ディスクが遅いときのように止めておく
  let releaseFirstWrite!: () => void;
  const firstWriteGate = new Promise<void>((resolve) => {
    releaseFirstWrite = resolve;
  });
  let isFirstWrite = true;
  const backend = wrapBackend(inner, {
    async writeDocument(path, document, expectedHash) {
      if (isFirstWrite) {
        isFirstWrite = false;
        await firstWriteGate;
      }
      return inner.writeDocument(path, document, expectedHash);
    },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <BackendProvider backend={backend}>{children}</BackendProvider>
  );
  const { result } = renderHook(() => useDocumentEditor(), { wrapper });

  act(() => useWorkspaceStore.getState().openDocument("concept.md"));
  await waitFor(() => expect(result.current.path).toBe("concept.md"));
  act(() => {
    result.current.onDocumentChange(textDocument("一回目の編集"));
    result.current.saveNow();
  });
  // 1 回目の保存が終わる前に入力を続け、すぐに別の文書へ切り替える
  act(() => {
    result.current.onDocumentChange(textDocument("二回目の編集"));
    result.current.saveNow();
    useWorkspaceStore.getState().openDocument("style.md");
  });
  releaseFirstWrite();

  await waitFor(() => expect(result.current.path).toBe("style.md"));
  expect(await readText(inner, "concept.md")).toBe("二回目の編集");
  expect(await readText(inner, "style.md")).toBe(styleBefore);
});

it("保存に失敗した編集が残っているときは、別の文書へ切り替えずに知らせる", async () => {
  const inner = createMockBackend({ delayMs: 0 });
  useWorkspaceStore.getState().openWorkspace(await inner.openProject(SAMPLE_PROJECT_FOLDER));
  const backend = wrapBackend(inner, {
    async writeDocument(path, document, expectedHash) {
      if (path === "concept.md") {
        throw new Error("ディスクがいっぱいです");
      }
      return inner.writeDocument(path, document, expectedHash);
    },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <BackendProvider backend={backend}>{children}</BackendProvider>
  );
  const { result } = renderHook(() => useDocumentEditor(), { wrapper });
  act(() => useWorkspaceStore.getState().openDocument("concept.md"));
  await waitFor(() => expect(result.current.path).toBe("concept.md"));
  act(() => result.current.onDocumentChange(textDocument("保存できない大事な編集")));

  act(() => useWorkspaceStore.getState().openDocument("style.md"));

  await waitFor(() => expect(useWorkspaceStore.getState().currentPath).toBe("concept.md"));
  expect(result.current.path).toBe("concept.md");
  expect(result.current.document).toEqual(textDocument("保存できない大事な編集"));
  expect(
    useUiStore.getState().toasts.some((toast) => toast.message.includes("切り替えません")),
  ).toBe(true);
});
