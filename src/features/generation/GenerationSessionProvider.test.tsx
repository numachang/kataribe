import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "../../App";
import { createMockBackend } from "../../api/mock";
import type { NewProject } from "../../api/types";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { renderWithBackend } from "../../test/renderWithBackend";
import { resetAllStores } from "../../test/resetStores";

beforeEach(resetAllStores);
afterEach(resetAllStores);

function newProject(title: string): NewProject {
  return {
    title,
    author: null,
    genre: "fantasy",
    genre_note: null,
    rating: "general",
    target_length: 10000,
    idea: "旅する少女の物語",
  };
}

describe("自動で進めている最中に作品を閉じる", () => {
  it("ループが止まり、別の作品を開いても生成が引き継がれない", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 20 });

    const overviewA = await backend.createProject("C:\\projects\\A", newProject("作品A"));
    act(() => useWorkspaceStore.getState().openWorkspace(overviewA));
    renderWithBackend(<App />, backend);

    await user.click(await screen.findByRole("button", { name: "自動で進める" }));
    // 生成が始まっているうちに、作品を閉じる。
    await screen.findByText("生成しています…");
    await user.click(screen.getByRole("button", { name: "作品を閉じる" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "新しい作品" })).toBeInTheDocument();
    });

    const overviewB = await backend.createProject("C:\\projects\\B", newProject("作品B"));
    act(() => useWorkspaceStore.getState().openWorkspace(overviewB));

    // 作品 A 側で走っていたはずの自動生成ループが、作品 B の工程を勝手に進めていないことを確かめる。
    // (少し待っても B 側の企画が生成されていなければ、A のループは正しく止まっている。)
    await new Promise((resolve) => setTimeout(resolve, 200));
    const pipelineB = await backend.pipeline();
    expect(pipelineB.find((step) => step.task.kind === "concept")?.state).toBe("ready");
  });
});
