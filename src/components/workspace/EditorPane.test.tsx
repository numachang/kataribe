import { act, fireEvent, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { renderWithBackend } from "../../test/renderWithBackend";
import { resetAllStores } from "../../test/resetStores";
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
    const saved = await backend.readFile("concept.md");
    expect(saved.content).toBe("書き直した企画本文");
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
    const saved = await backend.readFile("concept.md");
    expect(saved.content).toBe("Ctrl+S で保存した本文");
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

    const original = await backend.readFile("concept.md");
    await backend.writeFile("concept.md", "外部で書き換えられた内容", original.hash);

    vi.useFakeTimers();
    fireEvent.change(getConceptTextarea(), { target: { value: "画面上での編集内容" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    expect(screen.getByText("外部で変更されています")).toBeInTheDocument();

    vi.useRealTimers();
    await user.click(screen.getByRole("button", { name: "上書きする" }));

    const overwritten = await backend.readFile("concept.md");
    expect(overwritten.content).toBe("画面上での編集内容");
  });

  it("「再読み込み」を選ぶと外部の内容に置き換わる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^企画/ }));
    await screen.findByRole("textbox", { name: "concept.md" });

    const original = await backend.readFile("concept.md");
    await backend.writeFile("concept.md", "外部で書き換えられた内容", original.hash);

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
