import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { BackendProvider } from "../../api/context";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { useDocumentEditor } from "./useDocumentEditor";

beforeEach(resetAllStores);
afterEach(resetAllStores);

it("保存中に入力してすぐ別の文書へ切り替えても、前の文書の最後の入力が保存される", async () => {
  const inner = createMockBackend({ delayMs: 0 });
  useWorkspaceStore.getState().openWorkspace(await inner.openProject(SAMPLE_PROJECT_FOLDER));
  const styleBefore = await inner.readFile("style.md");

  // 最初の保存だけ、ディスクが遅いときのように止めておく
  let releaseFirstWrite!: () => void;
  const firstWriteGate = new Promise<void>((resolve) => {
    releaseFirstWrite = resolve;
  });
  let isFirstWrite = true;
  const backend = wrapBackend(inner, {
    async writeFile(path, content, expectedHash) {
      if (isFirstWrite) {
        isFirstWrite = false;
        await firstWriteGate;
      }
      return inner.writeFile(path, content, expectedHash);
    },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <BackendProvider backend={backend}>{children}</BackendProvider>
  );
  const { result } = renderHook(() => useDocumentEditor(), { wrapper });

  act(() => useWorkspaceStore.getState().openDocument("concept.md"));
  await waitFor(() => expect(result.current.path).toBe("concept.md"));
  act(() => {
    result.current.onContentChange("一回目の編集");
    result.current.saveNow();
  });
  // 1 回目の保存が終わる前に入力を続け、すぐに別の文書へ切り替える
  act(() => {
    result.current.onContentChange("二回目の編集");
    result.current.saveNow();
    useWorkspaceStore.getState().openDocument("style.md");
  });
  releaseFirstWrite();

  await waitFor(() => expect(result.current.path).toBe("style.md"));
  expect((await inner.readFile("concept.md")).content).toBe("二回目の編集");
  expect((await inner.readFile("style.md")).content).toBe(styleBefore.content);
});
