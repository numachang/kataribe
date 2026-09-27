import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { renderWithBackend } from "../../test/renderWithBackend";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { WorkspaceScreen } from "./WorkspaceScreen";

// ウィンドウを閉じる操作は OS 側から来るので、作業画面が登録する「閉じる前の処理」を横取りして呼ぶ
const windowClose = vi.hoisted(() => ({
  prepareToClose: null as (() => Promise<boolean>) | null,
}));
vi.mock("../../api/windowLifecycle", () => ({
  createWindowLifecycle: () => ({
    interceptClose(prepareToClose: () => Promise<boolean>) {
      windowClose.prepareToClose = prepareToClose;
      return () => {
        windowClose.prepareToClose = null;
      };
    },
  }),
}));

beforeEach(resetAllStores);
afterEach(resetAllStores);

function backendWithFailingWrites(): Backend {
  const inner = createMockBackend({ delayMs: 0 });
  return wrapBackend(inner, {
    async writeFile() {
      throw new Error("ディスクがいっぱいです");
    },
  });
}

/** サンプル作品を開き、企画（concept.md）に入力した状態にする。 */
async function editConcept(backend: Backend, text: string): Promise<void> {
  const user = userEvent.setup();
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
  renderWithBackend(<WorkspaceScreen />, backend);
  await user.click(await screen.findByRole("button", { name: /^企画/ }));
  const textarea = await screen.findByRole("textbox", { name: "concept.md" });
  fireEvent.change(textarea, { target: { value: text } });
}

function hasToast(text: string): boolean {
  return useUiStore.getState().toasts.some((toast) => toast.message.includes(text));
}

describe("保存できていない編集があるときは閉じない", () => {
  it("「作品を閉じる」を押しても作品を閉じず、理由を知らせる", async () => {
    const user = userEvent.setup();
    await editConcept(backendWithFailingWrites(), "保存できない大事な編集");

    await user.click(screen.getByRole("button", { name: "作品を閉じる" }));

    await waitFor(() => expect(hasToast("作品を閉じませんでした")).toBe(true));
    expect(useWorkspaceStore.getState().overview).not.toBeNull();
    expect(screen.getByRole("textbox", { name: "concept.md" })).toHaveValue(
      "保存できない大事な編集",
    );
  });

  it("ウィンドウを閉じようとしても閉じず、理由を知らせる", async () => {
    await editConcept(backendWithFailingWrites(), "保存できない大事な編集");

    let canClose = true;
    await act(async () => {
      canClose = (await windowClose.prepareToClose?.()) ?? true;
    });

    expect(canClose).toBe(false);
    expect(hasToast("ウィンドウを閉じませんでした")).toBe(true);
  });

  it("保存できれば、ウィンドウを閉じる前に保存してから閉じてよいと答える", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await editConcept(backend, "閉じる直前の編集");

    let canClose = false;
    await act(async () => {
      canClose = (await windowClose.prepareToClose?.()) ?? false;
    });

    expect(canClose).toBe(true);
    expect((await backend.readFile("concept.md")).content).toBe("閉じる直前の編集");
  });
});
