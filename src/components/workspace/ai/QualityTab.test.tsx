import { screen } from "@testing-library/react";
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

describe("品質タブ", () => {
  it("開いている本文の指標と問題点を表示する", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: /^招かれざる客/ }));
    await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" });

    await user.click(screen.getByRole("tab", { name: "品質" }));

    expect(await screen.findByText("会話率")).toBeInTheDocument();
    expect(screen.getByText("漢字率")).toBeInTheDocument();
    expect(screen.getByText("平均文長")).toBeInTheDocument();
  });

  it("文書を選んでいないときは案内を表示する", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(screen.getByRole("tab", { name: "品質" }));

    expect(document.querySelector(".quality-tab__empty")).toHaveTextContent(
      "左の目次から文書を選んでください。",
    );
  });
});
