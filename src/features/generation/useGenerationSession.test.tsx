import { act, renderHook, waitFor } from "@testing-library/react";
import { type ReactNode, StrictMode } from "react";
import { afterEach, beforeEach, expect, it } from "vitest";
import type { Backend } from "../../api/backend";
import { BackendProvider } from "../../api/context";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import type { NewProject, Task } from "../../api/types";
import { useEditorStore } from "../../store/editorStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { documentSaveController } from "../editor/documentSaveController";
import { useDocumentEditor } from "../editor/useDocumentEditor";
import { useGenerationSession } from "./useGenerationSession";

beforeEach(resetAllStores);
afterEach(resetAllStores);

const REVISE_CONCEPT: Task = { kind: "revise", path: "concept.md", instruction: "短く" };

function renderEditorAndSession(backend: Backend) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <BackendProvider backend={backend}>{children}</BackendProvider>
  );
  return renderHook(() => ({ editor: useDocumentEditor(), session: useGenerationSession() }), {
    wrapper,
  });
}

/** サンプル作品の concept.md を開き、その書き直しの変更案が見直し待ちになった状態にする。 */
async function reviewRevisionOfOpenConcept(backend: Backend) {
  const rendered = renderEditorAndSession(backend);
  act(() => useWorkspaceStore.getState().openDocument("concept.md"));
  await waitFor(() => expect(rendered.result.current.editor.path).toBe("concept.md"));
  act(() => rendered.result.current.session.start(REVISE_CONCEPT));
  await waitFor(() => expect(rendered.result.current.session.phase).toBe("reviewing"));
  return rendered;
}

it("開いている文書の保存に失敗しているときは、その文書への変更案を適用しない", async () => {
  const inner = createMockBackend({ delayMs: 0 });
  useWorkspaceStore.getState().openWorkspace(await inner.openProject(SAMPLE_PROJECT_FOLDER));
  const conceptBefore = await inner.readFile("concept.md");
  const backend = wrapBackend(inner, {
    async writeFile() {
      throw new Error("ディスクがいっぱいです");
    },
  });
  const { result } = await reviewRevisionOfOpenConcept(backend);

  act(() => result.current.editor.onContentChange("保存できていない大事な編集"));
  await act(async () => {
    await result.current.session.apply();
  });

  expect(result.current.session.phase).toBe("reviewing");
  expect(result.current.session.applyErrorMessage).toContain("保存できていない編集");
  expect(useEditorStore.getState().content).toBe("保存できていない大事な編集");
  expect((await inner.readFile("concept.md")).content).toBe(conceptBefore.content);
});

it("適用の間にエディタへ入力した文字は、適用後の読み直しで消さず、次の保存で競合として知らせる", async () => {
  const inner = createMockBackend({ delayMs: 0 });
  useWorkspaceStore.getState().openWorkspace(await inner.openProject(SAMPLE_PROJECT_FOLDER));
  let releaseApply!: () => void;
  const applyGate = new Promise<void>((resolve) => {
    releaseApply = resolve;
  });
  let notifyApplyStarted!: () => void;
  const applyStarted = new Promise<void>((resolve) => {
    notifyApplyStarted = resolve;
  });
  const backend = wrapBackend(inner, {
    async applyChangeSet(changeSet) {
      notifyApplyStarted();
      await applyGate;
      return inner.applyChangeSet(changeSet);
    },
  });
  const { result } = await reviewRevisionOfOpenConcept(backend);

  let applying!: Promise<void>;
  act(() => {
    applying = result.current.session.apply();
  });
  await applyStarted;
  act(() => result.current.editor.onContentChange("適用中に打った文章"));
  releaseApply();
  await act(async () => {
    await applying;
  });

  expect(useEditorStore.getState().content).toBe("適用中に打った文章");
  // 基準のハッシュは適用前のままなので、次の保存は競合になり、利用者が選べる
  act(() => result.current.editor.saveNow());
  await waitFor(() => expect(documentSaveController.getConflict()?.path).toBe("concept.md"));
  expect(documentSaveController.hasUnsavedWork()).toBe(true);
});

it("アンマウントすると、実行中の生成を中止し、そのあと変更案を適用しない", async () => {
  const newProject: NewProject = {
    title: "作品A",
    author: null,
    genre: "fantasy",
    genre_note: null,
    rating: "general",
    target_length: 10000,
    idea: "旅する少女の物語",
  };
  const inner = createMockBackend({ delayMs: 20 });
  useWorkspaceStore
    .getState()
    .openWorkspace(await inner.createProject("C:\\projects\\A", newProject));
  const calls: string[] = [];
  const backend = wrapBackend(inner, {
    async applyChangeSet(changeSet) {
      calls.push("apply");
      return inner.applyChangeSet(changeSet);
    },
    async cancelGeneration(jobId) {
      calls.push("cancel");
      return inner.cancelGeneration(jobId);
    },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <BackendProvider backend={backend}>{children}</BackendProvider>
  );
  const { result, unmount } = renderHook(() => useGenerationSession(), { wrapper });

  act(() => result.current.runAutoAdvance());
  await waitFor(() => expect(result.current.phase).toBe("running"));
  // 作品は開いたまま（Backend は中止しない）、画面だけが消えた場合でも止まることを確かめる
  unmount();
  await new Promise((resolve) => setTimeout(resolve, 1500));

  expect(calls).toContain("cancel");
  expect(calls).not.toContain("apply");
  const concept = (await inner.pipeline()).find((step) => step.task.kind === "concept");
  expect(concept?.state).toBe("ready");
});

it("StrictMode（開発時）でも、生成した変更案が見直し待ちになる", async () => {
  const backend = createMockBackend({ delayMs: 0 });
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
  const wrapper = ({ children }: { children: ReactNode }) => (
    <StrictMode>
      <BackendProvider backend={backend}>{children}</BackendProvider>
    </StrictMode>
  );
  const { result } = renderHook(() => useGenerationSession(), { wrapper });

  act(() => result.current.start(REVISE_CONCEPT));

  await waitFor(() => expect(result.current.phase).toBe("reviewing"));
  expect(result.current.changeSet?.files.length).toBeGreaterThan(0);
});
