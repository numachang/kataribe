import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Backend } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { renderWithBackend } from "../../../test/renderWithBackend";
import { resetAllStores } from "../../../test/resetStores";
import { WorkspaceScreen } from "../WorkspaceScreen";

beforeEach(resetAllStores);
afterEach(resetAllStores);

async function createEmptyProject(backend: Backend): Promise<void> {
  const overview = await backend.createProject("C:\\projects\\test-thinking", {
    title: "テスト作品",
    author: null,
    genre: "fantasy",
    genre_note: null,
    rating: "general",
    target_length: 10000,
    idea: "旅する少女の物語",
  });
  useWorkspaceStore.getState().openWorkspace(overview);
  await useWorkspaceStore.getState().refreshPipeline(backend);
}

describe("生成中の「考え中…」表示", () => {
  it("思考の断片だけが届いている間は「考え中…」と分かり、本文が流れ始めると普通の見出しに戻る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 30 });
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "次の工程を実行" }));

    expect(
      await screen.findByText("考え中…（思考過程）", {}, { timeout: 2000 }),
    ).toBeInTheDocument();

    await waitFor(
      () => {
        expect(screen.getByText("思考過程")).toBeInTheDocument();
      },
      { timeout: 2000 },
    );
  });
});
