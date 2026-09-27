import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { documentSaveController } from "./documentSaveController";

// このモジュールは React の外で完結する保存ロジックなので、フックやコンポーネントを介さずに
// 直接テストする。currentPath の切り替えは useWorkspaceStore を経由せず、useEditorStore を
// 直接書き換えて模擬する（documentSaveController は useEditorStore しか見ていないため）。

/** writeFile の完了を任意のタイミングまで遅らせられる Backend のラッパーを作る。 */
function withDelayedWrite(inner: Backend): {
  backend: Backend;
  releaseWrites: () => void;
  writeStarted: Promise<void>;
} {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let notifyStarted!: () => void;
  const writeStarted = new Promise<void>((resolve) => {
    notifyStarted = resolve;
  });
  const backend: Backend = {
    ...inner,
    async writeFile(path, content, expectedHash) {
      notifyStarted();
      await gate;
      return inner.writeFile(path, content, expectedHash);
    },
  };
  return { backend, releaseWrites: release, writeStarted };
}

async function openProject(backend: Backend, folder = SAMPLE_PROJECT_FOLDER) {
  return backend.openProject(folder);
}

beforeEach(() => {
  documentSaveController.reset();
  useEditorStore.getState().reset();
  useEditorStore.setState({ vertical: true, rubyPreview: false });
  useUiStore.setState({ toasts: [] });
});

afterEach(() => {
  documentSaveController.reset();
  useEditorStore.getState().reset();
});

describe("flush", () => {
  it("保留中の自動保存をすぐに実行してから解決する（保存してから取り消す順序）", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await backend.readFile("concept.md");
    useEditorStore.getState().loadDocument("concept.md", original.content, original.hash);

    useEditorStore.getState().updateContent("退避直前の編集");
    documentSaveController.notifyChange(backend);

    // タイマーが発火する前に flush する（アンマウント・作品を閉じる・ウィンドウを閉じるのシナリオ）。
    await documentSaveController.flush(backend);

    const saved = await backend.readFile("concept.md");
    expect(saved.content).toBe("退避直前の編集");
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("保存に失敗した編集は、flush のあとも「保存できていない」として残る", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    const original = await inner.readFile("concept.md");
    useEditorStore.getState().loadDocument("concept.md", original.content, original.hash);
    const backend: Backend = {
      ...inner,
      async writeFile() {
        throw new Error("ディスクがいっぱいです");
      },
    };

    useEditorStore.getState().updateContent("保存できない編集");
    documentSaveController.notifyChange(backend);
    await documentSaveController.flush(backend);

    expect(documentSaveController.hasUnsavedWork()).toBe(true);
    expect(useEditorStore.getState().content).toBe("保存できない編集");
  });

  it("保存が済めば「保存できていない編集」は残らない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await backend.readFile("concept.md");
    useEditorStore.getState().loadDocument("concept.md", original.content, original.hash);

    useEditorStore.getState().updateContent("保存できる編集");
    documentSaveController.notifyChange(backend);
    await documentSaveController.flush(backend);

    expect(documentSaveController.hasUnsavedWork()).toBe(false);
  });

  it("何も保留していなければ即座に解決する", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await expect(documentSaveController.flush(backend)).resolves.toBeUndefined();
  });
});

describe("保存の直列化", () => {
  it("実行中に saveNow が来たら、終わったあとに最新の内容でもう一度保存する", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    const original = await inner.readFile("concept.md");
    useEditorStore.getState().loadDocument("concept.md", original.content, original.hash);

    const { backend, releaseWrites, writeStarted } = withDelayedWrite(inner);

    useEditorStore.getState().updateContent("1回目の編集");
    documentSaveController.saveNow(backend);
    await writeStarted;

    // 1 回目の書き込みが進行中に、さらに編集して 2 回目を要求する。
    useEditorStore.getState().updateContent("2回目の編集");
    documentSaveController.saveNow(backend);

    releaseWrites();
    await documentSaveController.flush(backend);

    const saved = await inner.readFile("concept.md");
    expect(saved.content).toBe("2回目の編集");
    expect(useEditorStore.getState().status).toBe("clean");
  });
});

describe("無関係な文書への切り替え後に保存が完了したとき", () => {
  it("保存に成功しても、別の文書に切り替わっていればその文書の状態を書き換えない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    const originalConcept = await inner.readFile("concept.md");
    useEditorStore
      .getState()
      .loadDocument("concept.md", originalConcept.content, originalConcept.hash);

    const { backend, releaseWrites, writeStarted } = withDelayedWrite(inner);
    useEditorStore.getState().updateContent("concept の編集");
    documentSaveController.notifyChange(backend);
    documentSaveController.saveNow(backend);
    await writeStarted;

    // concept.md の保存がまだ終わっていないうちに、別の文書 (style.md) に切り替わる。
    const originalStyle = await inner.readFile("style.md");
    useEditorStore.getState().loadDocument("style.md", originalStyle.content, originalStyle.hash);

    releaseWrites();
    await documentSaveController.flush(backend);

    // style.md 側の表示状態は、concept.md の保存結果で上書きされていない。
    expect(useEditorStore.getState().path).toBe("style.md");
    expect(useEditorStore.getState().content).toBe(originalStyle.content);
    expect(useEditorStore.getState().status).toBe("clean");
    expect(useEditorStore.getState().savedHash).toBe(originalStyle.hash);

    // ディスクへの書き込み自体は成功している。
    const savedConcept = await inner.readFile("concept.md");
    expect(savedConcept.content).toBe("concept の編集");
  });

  it("保存が外部の変更と競合しても、切り替え後の文書とは別に競合として残る", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    const originalConcept = await inner.readFile("concept.md");
    useEditorStore
      .getState()
      .loadDocument("concept.md", originalConcept.content, originalConcept.hash);

    const { backend, releaseWrites, writeStarted } = withDelayedWrite(inner);
    useEditorStore.getState().updateContent("concept のローカル編集");
    documentSaveController.saveNow(backend);
    await writeStarted;

    // 保存が進行中に、外部で concept.md が書き換えられる。
    await inner.writeFile("concept.md", "外部での書き換え", originalConcept.hash);

    // さらに、画面側は style.md に切り替わる。
    const originalStyle = await inner.readFile("style.md");
    useEditorStore.getState().loadDocument("style.md", originalStyle.content, originalStyle.hash);

    releaseWrites();
    await documentSaveController.flush(backend);

    // 競合は concept.md のものとして残り、style.md の表示は乱れない。
    const conflict = documentSaveController.getConflict();
    expect(conflict?.path).toBe("concept.md");
    expect(conflict?.content).toBe("concept のローカル編集");
    expect(useEditorStore.getState().path).toBe("style.md");
    expect(useEditorStore.getState().status).toBe("clean");
  });
});

describe("競合の解決", () => {
  it("上書きすると、外部の変更を退けてローカルの内容が残る", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await backend.readFile("concept.md");
    useEditorStore.getState().loadDocument("concept.md", original.content, original.hash);
    await backend.writeFile("concept.md", "外部での書き換え", original.hash);

    useEditorStore.getState().updateContent("ローカルの編集");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    expect(documentSaveController.getConflict()).not.toBeNull();

    await documentSaveController.resolveConflictByOverwriting(backend);

    expect(documentSaveController.getConflict()).toBeNull();
    const saved = await backend.readFile("concept.md");
    expect(saved.content).toBe("ローカルの編集");
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("外部でファイルが削除されていても、上書きは新規作成として扱う", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    // concept.md がまだ存在しない（= 外部で削除された、と同じ状態）作品を用意する。
    await backend.createProject("C:\\projects\\empty", {
      title: "テスト作品",
      author: null,
      genre: "fantasy",
      genre_note: null,
      rating: "general",
      target_length: 10000,
      idea: "種",
    });

    // 画面側は、以前読み込んだ古いハッシュを基準に持っている想定にする。
    useEditorStore.getState().loadDocument("concept.md", "以前の内容", "stale-hash");
    useEditorStore.getState().updateContent("復元したい内容");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    expect(documentSaveController.getConflict()?.path).toBe("concept.md");

    await documentSaveController.resolveConflictByOverwriting(backend);

    expect(documentSaveController.getConflict()).toBeNull();
    const created = await backend.readFile("concept.md");
    expect(created.content).toBe("復元したい内容");
  });

  it("再読み込みすると、外部の内容に置き換わる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await backend.readFile("concept.md");
    useEditorStore.getState().loadDocument("concept.md", original.content, original.hash);
    await backend.writeFile("concept.md", "外部での書き換え", original.hash);

    useEditorStore.getState().updateContent("ローカルの編集");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    await documentSaveController.resolveConflictByReloading(backend);

    expect(documentSaveController.getConflict()).toBeNull();
    expect(useEditorStore.getState().content).toBe("外部での書き換え");
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("何も選ばずに閉じると、競合は消えるが例外にはならず、利用者に知らせる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await backend.readFile("concept.md");
    useEditorStore.getState().loadDocument("concept.md", original.content, original.hash);
    await backend.writeFile("concept.md", "外部での書き換え", original.hash);

    useEditorStore.getState().updateContent("ローカルの編集");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    expect(documentSaveController.getConflict()).not.toBeNull();

    documentSaveController.dismissConflict();

    expect(documentSaveController.getConflict()).toBeNull();
    expect(useUiStore.getState().toasts.some((toast) => toast.kind === "error")).toBe(true);
  });
});
