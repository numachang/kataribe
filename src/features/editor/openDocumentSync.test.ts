import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { relocatedPath, rewritesPath, touchesPath } from "../../lib/changeSetPaths";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { editorBody, editText, loadIntoEditor, readText, textDocument } from "../../test/documents";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { documentSaveController } from "./documentSaveController";
import { writeBesideEditor } from "./openDocumentSync";

beforeEach(resetAllStores);
afterEach(resetAllStores);

const PATH = "concept.md";

/** サンプル作品を開き、`concept.md` をエディタで開いた状態にする。 */
async function openConceptInEditor(backend: Backend): Promise<string> {
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
  useWorkspaceStore.getState().openDocument(PATH);
  await loadIntoEditor(backend, PATH);
  return readText(backend, PATH);
}

/** エディタを通さずに `concept.md` を書き換える。 */
function rewriteConcept(backend: Backend, content: string) {
  return async () => {
    const { hash } = await backend.readDocument(PATH);
    return backend.writeDocument(PATH, textDocument(content), hash);
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

    expect(editorBody()).toBe("書き換えた企画");
  });

  it("項目に分けて開いている人物資料も、書き換えた項目と本文を読み直す", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    const characterPath = "characters/kirishima-rin.md";
    useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
    useWorkspaceStore.getState().openDocument(characterPath);
    await loadIntoEditor(backend, characterPath);

    await writeBesideEditor(backend, {
      touches: (path) => path === characterPath,
      write: async () => {
        const { document, hash } = await backend.readDocument(characterPath);
        if (document.kind !== "character") {
          throw new Error("人物資料として読めるはず");
        }
        return backend.writeDocument(
          characterPath,
          { ...document, meta: { ...document.meta, name: "霧島 凛子" }, body: "書き換えた本文" },
          hash,
        );
      },
      unsavedWorkMessage: "未保存",
    });

    const { document } = useEditorStore.getState();
    expect(document).toMatchObject({
      kind: "character",
      meta: { name: "霧島 凛子" },
      body: "書き換えた本文",
    });
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("保存できない編集が残っていれば、書き換えずに止める", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    const original = await openConceptInEditor(inner);
    const backend = wrapBackend(inner, {
      async writeDocument() {
        throw new Error("ディスクがいっぱいです");
      },
    });
    editText("保存できていない編集");
    const write = vi.fn(rewriteConcept(inner, "書き換えた企画"));

    await expect(
      writeBesideEditor(backend, {
        touches: (path) => path === PATH,
        write,
        unsavedWorkMessage: "保存できていない編集があります",
      }),
    ).rejects.toThrow("保存できていない編集があります");

    expect(write).not.toHaveBeenCalled();
    expect(await readText(inner, PATH)).toBe(original);
    expect(editorBody()).toBe("保存できていない編集");
  });

  it("書き換えの間にエディタへ入力されていたら、読み直さない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(backend);

    await writeBesideEditor(backend, {
      touches: (path) => path === PATH,
      write: async () => {
        await rewriteConcept(backend, "書き換えた企画")();
        editText("書き換えの間に入力した内容");
      },
      unsavedWorkMessage: "未保存",
    });

    expect(editorBody()).toBe("書き換えの間に入力した内容");
  });

  it("書き換えの間の入力が、書き換え前と同じ内容に戻っていても、読み直さない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    const original = await openConceptInEditor(backend);

    await writeBesideEditor(backend, {
      touches: (path) => path === PATH,
      write: async () => {
        await rewriteConcept(backend, "書き換えた企画")();
        editText("途中の入力");
        editText(original);
      },
      unsavedWorkMessage: "未保存",
    });

    expect(editorBody()).toBe(original);
  });

  it("読み直しに失敗しても、書き換えの結果を返し、失敗を知らせる", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(inner);
    let reads = 0;
    const backend = wrapBackend(inner, {
      async readDocument(path) {
        reads += 1;
        // 1 回目は書き換えの中の読み込み、2 回目が読み直し
        if (reads > 1) {
          throw new Error("一時的に読めません");
        }
        return inner.readDocument(path);
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

  it("開いている文書がゴミ箱へ移るなら、読み直さずに閉じて、そのことを知らせる", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(inner);
    const readDocument = vi.fn(inner.readDocument.bind(inner));
    const backend = wrapBackend(inner, { readDocument });

    await writeBesideEditor(backend, {
      touches: (path) => path === PATH,
      relocatedPath: (path) => (path === PATH ? null : undefined),
      write: async () => "ゴミ箱へ移した",
      unsavedWorkMessage: "未保存",
    });

    expect(readDocument).not.toHaveBeenCalled();
    expect(useEditorStore.getState().path).toBeNull();
    expect(useEditorStore.getState().document).toBeNull();
    expect(useWorkspaceStore.getState().currentPath).toBeNull();
    expect(useUiStore.getState().toasts.at(-1)?.message).toBe(
      "開いていた「concept.md」はゴミ箱へ移したので、閉じました。",
    );
  });

  it("ゴミ箱へ移るのが別の文書なら、開いている文書は閉じない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(backend);

    await writeBesideEditor(backend, {
      touches: (path) => path === "style.md",
      relocatedPath: (path) => (path === "style.md" ? null : undefined),
      write: async () => "ゴミ箱へ移した",
      unsavedWorkMessage: "未保存",
    });

    expect(useWorkspaceStore.getState().currentPath).toBe(PATH);
    expect(useEditorStore.getState().path).toBe(PATH);
  });

  it("ゴミ箱へ移す文書に保存できない編集が残っていれば、移さずに止める", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(inner);
    const backend = wrapBackend(inner, {
      async writeDocument() {
        throw new Error("ディスクがいっぱいです");
      },
    });
    editText("保存できていない編集");
    const write = vi.fn(async () => "ゴミ箱へ移した");

    await expect(
      writeBesideEditor(backend, {
        touches: (path) => path === PATH,
        relocatedPath: (path) => (path === PATH ? null : undefined),
        write,
        unsavedWorkMessage: "保存できていない編集があります",
      }),
    ).rejects.toThrow("保存できていない編集があります");

    expect(write).not.toHaveBeenCalled();
    expect(editorBody()).toBe("保存できていない編集");
  });

  it("書き換えない文書を開いているときは、読み直さない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openConceptInEditor(inner);
    const readDocument = vi.fn(inner.readDocument.bind(inner));
    const backend = wrapBackend(inner, { readDocument });

    await writeBesideEditor(backend, {
      touches: (path) => path === "style.md",
      write: async () => "書き換えた",
      unsavedWorkMessage: "未保存",
    });

    expect(readDocument).not.toHaveBeenCalled();
  });
});

describe("writeBesideEditor（章の番号の振り直しで、開いている文書が改名されるとき）", () => {
  const OLD_PATH = "manuscript/01/s01.txt";
  const NEW_PATH = "manuscript/02/s01.txt";

  /** 第 1 章の前に章を足す（第 1 章の本文のフォルダが manuscript/02 へ移る）。 */
  async function addChapterBeforeFirst(backend: Backend) {
    const { change_set: changeSet } = await backend.planStructureEdit({
      kind: "add_chapter",
      before: "01",
      title: "序章",
      storyline: "",
    });
    return {
      touches: (path: string) => touchesPath(changeSet, path),
      rewrites: (path: string) => rewritesPath(changeSet, path),
      relocatedPath: (path: string) => relocatedPath(changeSet, path),
      write: () => backend.applyChangeSet(changeSet),
      unsavedWorkMessage: "未保存",
    };
  }

  async function openSceneInEditor(backend: Backend): Promise<void> {
    useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
    useWorkspaceStore.getState().openDocument(OLD_PATH);
    await loadIntoEditor(backend, OLD_PATH);
  }

  it("読み直さずに、エディタと目次の選択を新しいパスへ付け替える", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openSceneInEditor(inner);
    const originalBody = editorBody();
    const readDocument = vi.fn(inner.readDocument.bind(inner));
    const backend = wrapBackend(inner, { readDocument });

    await writeBesideEditor(backend, await addChapterBeforeFirst(backend));

    expect(readDocument).not.toHaveBeenCalled();
    expect(useEditorStore.getState().path).toBe(NEW_PATH);
    expect(useWorkspaceStore.getState().currentPath).toBe(NEW_PATH);
    expect(editorBody()).toBe(originalBody);
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("続けて編集して保存すると、新しいパスへ書き、古いパスには書かない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSceneInEditor(backend);
    await writeBesideEditor(backend, await addChapterBeforeFirst(backend));

    editText("改名のあとに書き足した本文");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    expect(await readText(backend, NEW_PATH)).toBe("改名のあとに書き足した本文");
    await expect(backend.readDocument(OLD_PATH)).rejects.toMatchObject({ kind: "not_found" });
    expect(useEditorStore.getState().status).toBe("clean");
    expect(documentSaveController.getConflict()).toBeNull();
  });

  it("保存できない編集が残っていれば、改名もせずに止める", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openSceneInEditor(inner);
    const backend = wrapBackend(inner, {
      async writeDocument() {
        throw new Error("ディスクがいっぱいです");
      },
    });
    editText("保存できていない編集");
    const options = await addChapterBeforeFirst(inner);
    const write = vi.fn(options.write);

    await expect(writeBesideEditor(backend, { ...options, write })).rejects.toThrow("未保存");

    expect(write).not.toHaveBeenCalled();
    expect(useEditorStore.getState().path).toBe(OLD_PATH);
  });

  it("改名されない文書を開いているときは、そのまま", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
    useWorkspaceStore.getState().openDocument(PATH);
    await loadIntoEditor(backend, PATH);

    await writeBesideEditor(backend, await addChapterBeforeFirst(backend));

    expect(useEditorStore.getState().path).toBe(PATH);
    expect(useWorkspaceStore.getState().currentPath).toBe(PATH);
  });
});
