import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { editorBody, editText, loadIntoEditor, readText, textDocument } from "../../test/documents";
import { wrapBackend } from "../../test/wrapBackend";
import { documentSaveController } from "./documentSaveController";

// このモジュールは React の外で完結する保存ロジックなので、フックやコンポーネントを介さずに
// 直接テストする。currentPath の切り替えは useWorkspaceStore を経由せず、useEditorStore を
// 直接書き換えて模擬する（documentSaveController は useEditorStore しか見ていないため）。

/** writeDocument の完了を任意のタイミングまで遅らせられる Backend のラッパーを作る。 */
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
  const backend = wrapBackend(inner, {
    async writeDocument(path, document, expectedHash) {
      notifyStarted();
      await gate;
      return inner.writeDocument(path, document, expectedHash);
    },
  });
  return { backend, releaseWrites: release, writeStarted };
}

async function openProject(backend: Backend, folder = SAMPLE_PROJECT_FOLDER) {
  return backend.openProject(folder);
}

beforeEach(() => {
  documentSaveController.reset();
  useWorkspaceStore.getState().closeWorkspace();
  useEditorStore.getState().reset();
  useEditorStore.setState({ vertical: true, rubyPreview: false });
  useUiStore.setState({ toasts: [] });
});

afterEach(() => {
  documentSaveController.reset();
  useWorkspaceStore.getState().closeWorkspace();
  useEditorStore.getState().reset();
});

describe("flush", () => {
  it("保留中の自動保存をすぐに実行してから解決する（保存してから取り消す順序）", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    await loadIntoEditor(backend, "concept.md");

    editText("退避直前の編集");
    documentSaveController.notifyChange(backend);

    // タイマーが発火する前に flush する（アンマウント・作品を閉じる・ウィンドウを閉じるのシナリオ）。
    await documentSaveController.flush(backend);

    expect(await readText(backend, "concept.md")).toBe("退避直前の編集");
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("保存に失敗した編集は、flush のあとも「保存できていない」として残る", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    await loadIntoEditor(inner, "concept.md");
    const backend = wrapBackend(inner, {
      async writeDocument() {
        throw new Error("ディスクがいっぱいです");
      },
    });

    editText("保存できない編集");
    documentSaveController.notifyChange(backend);
    await documentSaveController.flush(backend);

    expect(documentSaveController.hasUnsavedWork()).toBe(true);
    expect(editorBody()).toBe("保存できない編集");
  });

  it("保存が済めば「保存できていない編集」は残らない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    await loadIntoEditor(backend, "concept.md");

    editText("保存できる編集");
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
    await loadIntoEditor(inner, "concept.md");

    const { backend, releaseWrites, writeStarted } = withDelayedWrite(inner);

    editText("1回目の編集");
    documentSaveController.saveNow(backend);
    await writeStarted;

    // 1 回目の書き込みが進行中に、さらに編集して 2 回目を要求する。
    editText("2回目の編集");
    documentSaveController.saveNow(backend);

    releaseWrites();
    await documentSaveController.flush(backend);

    expect(await readText(inner, "concept.md")).toBe("2回目の編集");
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("保存中に編集が進んだら、保存が成功しても未保存のままにする（内容が元に戻っていても）", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    const original = await loadIntoEditor(inner, "concept.md");
    const { backend, releaseWrites, writeStarted } = withDelayedWrite(inner);

    editText("保存する内容");
    documentSaveController.saveNow(backend);
    await writeStarted;
    // 保存中に別の内容へ変えてから、保存中の内容と同じ文字列へ戻す（文書の中身は一致するが、版は進んでいる）
    editText("途中の内容");
    editText("保存する内容");

    releaseWrites();
    await documentSaveController.flush(backend);

    expect(useEditorStore.getState().status).toBe("dirty");
    expect(useEditorStore.getState().savedHash).not.toBe(original.hash);
    // 次の保存は、返ってきたハッシュを前提にするので競合しない
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    expect(documentSaveController.getConflict()).toBeNull();
    expect(useEditorStore.getState().status).toBe("clean");
  });
});

describe("人物資料・章立てを保存したあと", () => {
  async function openWorkspaceFor(backend: Backend) {
    useWorkspaceStore.getState().openWorkspace(await openProject(backend));
    await useWorkspaceStore.getState().refreshPipeline(backend);
  }

  function characterLabels(): string[] {
    const section = useWorkspaceStore
      .getState()
      .overview?.sections.find((candidate) => candidate.kind === "characters");
    return section?.entries.map((entry) => entry.label) ?? [];
  }

  it("目次の人物名が、保存した名前に変わる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openWorkspaceFor(backend);
    const file = await loadIntoEditor(backend, "characters/kirishima-rin.md");
    if (file.document.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }

    useEditorStore.getState().updateDocument({
      ...file.document,
      meta: { ...file.document.meta, name: "霧島 凛子" },
    });
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    await vi.waitFor(() => expect(characterLabels()).toContain("霧島 凛子"));
  });

  it("YAML が壊れていて文字列で開いた人物資料も、直して保存すれば目次が変わる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openWorkspaceFor(backend);
    const { hash } = await backend.readDocument("characters/kirishima-rin.md");
    useEditorStore.getState().loadDocument("characters/kirishima-rin.md", {
      document: textDocument("---\nname: [\n---\n"),
      hash,
      parse_error: "characters/kirishima-rin.md の 2 行目を読めません",
    });

    editText("---\nname: 霧島 凛子\nrole: 主人公\nsummary: 概要\n---\n本文\n");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    await vi.waitFor(() => expect(characterLabels()).toContain("霧島 凛子"));
  });

  /** 目次と工程の読み直しの回数を数える Backend。 */
  function countingBackend(inner: Backend, overrides: Partial<Backend> = {}) {
    const calls = { overview: 0, pipeline: 0 };
    const backend = wrapBackend(inner, {
      async overview() {
        calls.overview += 1;
        return inner.overview();
      },
      async pipeline() {
        calls.pipeline += 1;
        return inner.pipeline();
      },
      ...overrides,
    });
    return { backend, calls };
  }

  async function openCharacter(backend: Backend) {
    const file = await loadIntoEditor(backend, "characters/kirishima-rin.md");
    if (file.document.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }
    return file.document;
  }

  /** 読み直しは保存の外で行うので、始まっていれば済むだけの時間を置いてから確かめる。 */
  async function letRefreshRun(): Promise<void> {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  it("本文だけを直して保存しても、目次と工程は読み直さない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openWorkspaceFor(inner);
    const { backend, calls } = countingBackend(inner);
    calls.pipeline = 0;
    const character = await openCharacter(backend);

    useEditorStore.getState().updateDocument({ ...character, body: "本文だけ直した" });
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    await letRefreshRun();

    expect(useEditorStore.getState().status).toBe("clean");
    expect(calls).toEqual({ overview: 0, pipeline: 0 });
  });

  it("名前を直して保存すると、目次と工程を読み直す。続けて本文だけ直しても、もう読み直さない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openWorkspaceFor(inner);
    const { backend, calls } = countingBackend(inner);
    calls.pipeline = 0;
    const character = await openCharacter(backend);

    const renamed = { ...character, meta: { ...character.meta, name: "霧島 凛子" } };
    useEditorStore.getState().updateDocument(renamed);
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    await vi.waitFor(() => expect(calls).toEqual({ overview: 1, pipeline: 1 }));

    useEditorStore.getState().updateDocument({ ...renamed, body: "本文だけ直した" });
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    await letRefreshRun();

    expect(calls).toEqual({ overview: 1, pipeline: 1 });
  });

  it("目次の読み直しが終わらなくても、flush は待たされない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openWorkspaceFor(inner);
    const backend = wrapBackend(inner, { overview: () => new Promise(() => {}) });
    const character = await openCharacter(backend);

    useEditorStore.getState().updateDocument({
      ...character,
      meta: { ...character.meta, name: "霧島 凛子" },
    });
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    expect(useEditorStore.getState().status).toBe("clean");
    expect(documentSaveController.hasUnsavedWork()).toBe(false);
  });

  it("手で書く文書（企画など）を保存しても、目次は読み直さない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openWorkspaceFor(inner);
    let overviewCalls = 0;
    const backend = wrapBackend(inner, {
      async overview() {
        overviewCalls += 1;
        return inner.overview();
      },
    });
    await loadIntoEditor(backend, "concept.md");

    editText("書き直した企画");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    expect(overviewCalls).toBe(0);
  });

  it("目次を読み直せなくても、保存は済んでいて、知らせるだけにする", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openWorkspaceFor(inner);
    const backend = wrapBackend(inner, {
      async overview() {
        throw new Error("読み直せない");
      },
    });
    const file = await loadIntoEditor(backend, "characters/kirishima-rin.md");
    if (file.document.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }

    useEditorStore.getState().updateDocument({
      ...file.document,
      meta: { ...file.document.meta, name: "霧島 凛子" },
    });
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    expect(useEditorStore.getState().status).toBe("clean");
    await vi.waitFor(() =>
      expect(useUiStore.getState().toasts.some((toast) => toast.kind === "error")).toBe(true),
    );
  });
});

describe("無関係な文書への切り替え後に保存が完了したとき", () => {
  it("保存に成功しても、別の文書に切り替わっていればその文書の状態を書き換えない", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    await loadIntoEditor(inner, "concept.md");

    const { backend, releaseWrites, writeStarted } = withDelayedWrite(inner);
    editText("concept の編集");
    documentSaveController.notifyChange(backend);
    documentSaveController.saveNow(backend);
    await writeStarted;

    // concept.md の保存がまだ終わっていないうちに、別の文書 (style.md) に切り替わる。
    const originalStyle = await loadIntoEditor(inner, "style.md");

    releaseWrites();
    await documentSaveController.flush(backend);

    // style.md 側の表示状態は、concept.md の保存結果で上書きされていない。
    expect(useEditorStore.getState().path).toBe("style.md");
    expect(useEditorStore.getState().document).toEqual(originalStyle.document);
    expect(useEditorStore.getState().status).toBe("clean");
    expect(useEditorStore.getState().savedHash).toBe(originalStyle.hash);

    // ディスクへの書き込み自体は成功している。
    expect(await readText(inner, "concept.md")).toBe("concept の編集");
  });

  it("保存が外部の変更と競合しても、切り替え後の文書とは別に競合として残る", async () => {
    const inner = createMockBackend({ delayMs: 0 });
    await openProject(inner);
    const originalConcept = await loadIntoEditor(inner, "concept.md");

    const { backend, releaseWrites, writeStarted } = withDelayedWrite(inner);
    editText("concept のローカル編集");
    documentSaveController.saveNow(backend);
    await writeStarted;

    // 保存が進行中に、外部で concept.md が書き換えられる。
    await inner.writeDocument("concept.md", textDocument("外部での書き換え"), originalConcept.hash);

    // さらに、画面側は style.md に切り替わる。
    await loadIntoEditor(inner, "style.md");

    releaseWrites();
    await documentSaveController.flush(backend);

    // 競合は concept.md のものとして残り、style.md の表示は乱れない。
    const conflict = documentSaveController.getConflict();
    expect(conflict?.path).toBe("concept.md");
    expect(conflict?.document).toEqual(textDocument("concept のローカル編集"));
    expect(useEditorStore.getState().path).toBe("style.md");
    expect(useEditorStore.getState().status).toBe("clean");
  });
});

describe("競合の解決", () => {
  it("上書きすると、外部の変更を退けてローカルの内容が残る", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await loadIntoEditor(backend, "concept.md");
    await backend.writeDocument("concept.md", textDocument("外部での書き換え"), original.hash);

    editText("ローカルの編集");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    expect(documentSaveController.getConflict()).not.toBeNull();

    await documentSaveController.resolveConflictByOverwriting(backend);

    expect(documentSaveController.getConflict()).toBeNull();
    expect(await readText(backend, "concept.md")).toBe("ローカルの編集");
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
    useEditorStore.getState().loadDocument("concept.md", {
      document: textDocument("以前の内容"),
      hash: "stale-hash",
      parse_error: null,
    });
    editText("復元したい内容");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    expect(documentSaveController.getConflict()?.path).toBe("concept.md");

    await documentSaveController.resolveConflictByOverwriting(backend);

    expect(documentSaveController.getConflict()).toBeNull();
    expect(await readText(backend, "concept.md")).toBe("復元したい内容");
  });

  it("再読み込みすると、外部の内容に置き換わる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await loadIntoEditor(backend, "concept.md");
    await backend.writeDocument("concept.md", textDocument("外部での書き換え"), original.hash);

    editText("ローカルの編集");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);

    await documentSaveController.resolveConflictByReloading(backend);

    expect(documentSaveController.getConflict()).toBeNull();
    expect(editorBody()).toBe("外部での書き換え");
    expect(useEditorStore.getState().status).toBe("clean");
  });

  it("何も選ばずに閉じると、競合は消えるが例外にはならず、利用者に知らせる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openProject(backend);
    const original = await loadIntoEditor(backend, "concept.md");
    await backend.writeDocument("concept.md", textDocument("外部での書き換え"), original.hash);

    editText("ローカルの編集");
    documentSaveController.saveNow(backend);
    await documentSaveController.flush(backend);
    expect(documentSaveController.getConflict()).not.toBeNull();

    documentSaveController.dismissConflict();

    expect(documentSaveController.getConflict()).toBeNull();
    expect(useUiStore.getState().toasts.some((toast) => toast.kind === "error")).toBe(true);
  });
});
