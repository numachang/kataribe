import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { writeBesideEditor } from "./openDocumentSync";

beforeEach(resetAllStores);
afterEach(resetAllStores);

const PATH = "concept.md";

/** サンプル作品を開き、`concept.md` をエディタで開いた状態にする。 */
async function openConceptInEditor(backend: Backend): Promise<string> {
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
  const file = await backend.readFile(PATH);
  useWorkspaceStore.getState().openDocument(PATH);
  useEditorStore.getState().loadDocument(PATH, file.content, file.hash);
  return file.content;
}

/** エディタを通さずに `concept.md` を書き換える。 */
function rewriteConcept(backend: Backend, content: string) {
  return async () => {
    const { hash } = await backend.readFile(PATH);
    return backend.writeFile(PATH, content, hash);
  };
}

describe("writeBesideEditor", () => {
  it("書き換えた文書を開いていれば、書き換えた内容を読み直す", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(backend);

    await writeBesideEditor(backend, {
      touches: (path) => path === PATH,
      write: rewriteConcept(backend, "書き換えた企画"),
      unsavedWorkMessage: "未保存",
    });

    expect(useEditorStore.getState().content).toBe("書き換えた企画");
  });

  it("保存できない編集が残っていれば、書き換えずに止める", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    const original = await openConceptInEditor(inner);
    const backend = wrapBackend(inner, {
      async writeFile() {
        throw new Error("ディスクがいっぱいです");
      },
    });
    useEditorStore.getState().updateContent("保存できていない編集");
    const write = vi.fn(rewriteConcept(inner, "書き換えた企画"));

    await expect(
      writeBesideEditor(backend, {
        touches: (path) => path === PATH,
        write,
        unsavedWorkMessage: "保存できていない編集があります",
      }),
    ).rejects.toThrow("保存できていない編集があります");

    expect(write).not.toHaveBeenCalled();
    expect((await inner.readFile(PATH)).content).toBe(original);
    expect(useEditorStore.getState().content).toBe("保存できていない編集");
  });

  it("書き換えの間にエディタへ入力されていたら、読み直さない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(backend);

    await writeBesideEditor(backend, {
      touches: (path) => path === PATH,
      write: async () => {
        await rewriteConcept(backend, "書き換えた企画")();
        useEditorStore.getState().updateContent("書き換えの間に入力した内容");
      },
      unsavedWorkMessage: "未保存",
    });

    expect(useEditorStore.getState().content).toBe("書き換えの間に入力した内容");
  });

  it("読み直しに失敗しても、書き換えの結果を返し、失敗を知らせる", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(inner);
    let reads = 0;
    const backend = wrapBackend(inner, {
      async readFile(path) {
        reads += 1;
        // 1 回目は書き換えの中の読み込み、2 回目が読み直し
        if (reads > 1) {
          throw new Error("一時的に読めません");
        }
        return inner.readFile(path);
      },
    });

    const result = await writeBesideEditor(backend, {
      touches: (path) => path === PATH,
      write: rewriteConcept(backend, "書き換えた企画"),
      unsavedWorkMessage: "未保存",
    });

    expect(typeof result).toBe("string");
    expect(useUiStore.getState().toasts.at(-1)?.message).toContain("一時的に読めません");
  });

  it("書き換えない文書を開いているときは、読み直さない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(inner);
    const readFile = vi.fn(inner.readFile.bind(inner));
    const backend = wrapBackend(inner, { readFile });

    await writeBesideEditor(backend, {
      touches: (path) => path === "style.md",
      write: async () => "書き換えた",
      unsavedWorkMessage: "未保存",
    });

    expect(readFile).not.toHaveBeenCalled();
  });
});
