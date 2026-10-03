import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { findOverviewEntry } from "../../lib/overviewTree";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { readText, textDocument } from "../../test/documents";
import { renderWithBackend } from "../../test/renderWithBackend";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { WorkspaceScreen } from "./WorkspaceScreen";

beforeEach(resetAllStores);
afterEach(() => {
  resetAllStores();
  vi.useRealTimers();
});

async function openSampleProject(backend: Backend): Promise<void> {
  const overview = await backend.openProject(SAMPLE_PROJECT_FOLDER);
  useWorkspaceStore.getState().openWorkspace(overview);
}

function getConceptTextarea(): HTMLTextAreaElement {
  return screen.getByRole("textbox", { name: "concept.md" }) as HTMLTextAreaElement;
}

describe("目次から文書を開いて編集する", () => {
  it("目次のファイルをクリックすると内容が読み込まれる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));

    const textarea = (await screen.findByRole("textbox", {
      name: "concept.md",
    })) as HTMLTextAreaElement;
    expect(textarea.value).toContain("月霧の館");
    expect(screen.getByText("保存済み")).toBeInTheDocument();
  });

  it("入力が止まって 1 秒後に自動保存される", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    vi.useFakeTimers();
    fireEvent.change(getConceptTextarea(), { target: { value: "書き直した企画本文" } });
    expect(screen.getByText("未保存の変更があります")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    expect(screen.getByText("保存済み")).toBeInTheDocument();
    expect(await readText(backend, "concept.md")).toBe("書き直した企画本文");
  });

  it("Ctrl+S で即座に保存する", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    fireEvent.change(getConceptTextarea(), { target: { value: "Ctrl+S で保存した本文" } });
    fireEvent.keyDown(getConceptTextarea(), { key: "s", ctrlKey: true });

    await screen.findByText("保存済み");
    expect(await readText(backend, "concept.md")).toBe("Ctrl+S で保存した本文");
  });
});

describe("競合ダイアログ", () => {
  it("保存しようとした文書が外部で変更されていると、選び直させる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    const original = await backend.readDocument("concept.md");
    await backend.writeDocument(
      "concept.md",
      textDocument("外部で書き換えられた内容"),
      original.hash,
    );

    vi.useFakeTimers();
    fireEvent.change(getConceptTextarea(), { target: { value: "画面上での編集内容" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    expect(screen.getByText("外部で変更されています")).toBeInTheDocument();

    vi.useRealTimers();
    await user.click(screen.getByRole("button", { name: "上書きする" }));

    expect(await readText(backend, "concept.md")).toBe("画面上での編集内容");
  });

  it("「再読み込み」を選ぶと外部の内容に置き換わる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    const original = await backend.readDocument("concept.md");
    await backend.writeDocument(
      "concept.md",
      textDocument("外部で書き換えられた内容"),
      original.hash,
    );

    vi.useFakeTimers();
    fireEvent.change(getConceptTextarea(), { target: { value: "画面上での編集内容" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    vi.useRealTimers();

    await user.click(await screen.findByRole("button", { name: "再読み込み" }));

    expect(await screen.findByText("保存済み")).toBeInTheDocument();
    expect(getConceptTextarea().value).toBe("外部で書き換えられた内容");
  });
});

describe("作品を閉じる・アンマウント時の保存", () => {
  it("直前の編集（1 秒以内）も保存してから作品を閉じる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    // 自動保存のタイマー（1 秒）が発火する前に、すぐ「作品を閉じる」を押す。
    fireEvent.change(getConceptTextarea(), { target: { value: "閉じる直前の編集" } });
    await user.click(screen.getByRole("button", { name: "作品を閉じる" }));

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "作品を閉じる" })).not.toBeInTheDocument();
    });
    // 作品を閉じたあとも、保存自体はディスク（偽バックエンドの内部状態）に残っている。
    await backend.openProject(SAMPLE_PROJECT_FOLDER);
    expect(await readText(backend, "concept.md")).toBe("閉じる直前の編集");
  });
});

describe("未生成の文書を選んだとき", () => {
  it("エディタは前の文書を表示し続けず、専用の空の状態を出す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    // 「消えた甥」はサンプル作品内でまだ本文が生成されていないシーン（exists: false）。
    await user.click(await screen.findByRole("button", { name: /^消えた甥/ }));

    expect(await screen.findByText("この文書はまだ生成されていません。")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "concept.md" })).not.toBeInTheDocument();
  });

  it("本文のシーンなら「空の本文から書き始める」が出て、押すと空のエディタで書き始められる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^消えた甥/ }));
    await user.click(await screen.findByRole("button", { name: "空の本文から書き始める" }));

    const textarea = await screen.findByRole("textbox", { name: "manuscript/01/s03.txt" });
    expect(textarea).toHaveValue("");
    expect(screen.queryByText("この文書はまだ生成されていません。")).not.toBeInTheDocument();
    // 目次では「まだ無い」印が外れ、工程でも本文は済みになる
    const entry = findOverviewEntry(useWorkspaceStore.getState().overview, "manuscript/01/s03.txt");
    expect(entry?.exists).toBe(true);
    const pipeline = await backend.pipeline();
    expect(pipeline.find((step) => step.label === "雨の匂い 消えた甥")?.state).toBe("done");

    await user.type(textarea, "霧が出ていた。");
    fireEvent.keyDown(textarea, { key: "s", ctrlKey: true });
    await waitFor(async () =>
      expect(await readText(backend, "manuscript/01/s03.txt")).toBe("霧が出ていた。"),
    );
  });

  it("書き始めるボタンは、本文のシーン以外には出さない", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    const overview = await backend.createProject("C:projectsempty", {
      title: "空の作品",
      author: null,
      genre: "mystery",
      genre_note: null,
      rating: "general",
      target_length: 10000,
      idea: "雨の夜の探偵の物語",
    });
    useWorkspaceStore.getState().openWorkspace(overview);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));

    expect(await screen.findByText("この文書はまだ生成されていません。")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "空の本文から書き始める" }),
    ).not.toBeInTheDocument();
  });

  it("作れなかったときは、理由を知らせ、エディタは出さない", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const backend = wrapBackend(inner, {
      async writeDocument() {
        throw new Error("ディスクがいっぱいです");
      },
    });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^消えた甥/ }));
    await user.click(await screen.findByRole("button", { name: "空の本文から書き始める" }));

    await waitFor(() =>
      expect(
        useUiStore
          .getState()
          .toasts.some((toast) => toast.message.includes("ディスクがいっぱいです")),
      ).toBe(true),
    );
    expect(screen.getByText("この文書はまだ生成されていません。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "空の本文から書き始める" })).toBeEnabled();
  });
});

describe("競合ダイアログを閉じる", () => {
  it("どちらも選ばずに閉じられる（詰まない）", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    const original = await backend.readDocument("concept.md");
    await backend.writeDocument(
      "concept.md",
      textDocument("外部で書き換えられた内容"),
      original.hash,
    );

    vi.useFakeTimers();
    fireEvent.change(getConceptTextarea(), { target: { value: "画面上での編集内容" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    vi.useRealTimers();

    expect(await screen.findByText("外部で変更されています")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "閉じる" }));

    expect(screen.queryByText("外部で変更されています")).not.toBeInTheDocument();
  });
});

describe("縦書き・横書きの切り替え", () => {
  it("ボタンを押すと縦書きの表示に切り替わる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    const textarea = await screen.findByRole("textbox", { name: "concept.md" });
    // 既定は縦書き。
    expect(textarea.className).toContain("editor-pane__textarea--vertical");

    await user.click(screen.getByRole("button", { name: "横書きにする" }));
    expect(textarea.className).not.toContain("editor-pane__textarea--vertical");

    await user.click(screen.getByRole("button", { name: "縦書きにする" }));
    expect(textarea.className).toContain("editor-pane__textarea--vertical");
  });
});
