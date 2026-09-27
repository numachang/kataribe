import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Backend } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../../api/mock/sampleProject";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { renderWithBackend } from "../../../test/renderWithBackend";
import { resetAllStores } from "../../../test/resetStores";
import { WorkspaceScreen } from "../WorkspaceScreen";

beforeEach(resetAllStores);
afterEach(resetAllStores);

async function openSampleProject(backend: Backend): Promise<void> {
  const overview = await backend.openProject(SAMPLE_PROJECT_FOLDER);
  useWorkspaceStore.getState().openWorkspace(overview);
}

describe("この文書タブから書き直す", () => {
  it("指示を入力して書き直すと、変更案を適用できる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /招かれざる客/ }));
    await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" });

    await user.click(screen.getByRole("tab", { name: "この文書" }));

    const instruction = await screen.findByPlaceholderText("例: もっと不穏な雰囲気にしてください");
    await user.type(instruction, "もっと不穏な雰囲気にして");
    await user.click(screen.getByRole("button", { name: /書き直す/ }));

    await screen.findByText("指示に沿って書き直しました", {}, { timeout: 3000 });
    const before = await backend.readFile("manuscript/01/s01.txt");

    await user.click(screen.getByRole("button", { name: "適用" }));

    await waitFor(async () => {
      const after = await backend.readFile("manuscript/01/s01.txt");
      expect(after.content).not.toBe(before.content);
    });
  });
});
