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
  const overview = await backend.createProject("C:\\projects\\test-pipeline", {
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

describe("工程タブから生成する", () => {
  it("次の工程を実行すると、ストリーミング表示のあとに変更案を適用できる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 5 });
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    expect(await screen.findByText("生成できます")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));

    expect(await screen.findByText("生成しています…")).toBeInTheDocument();

    await screen.findByText("企画を生成しました", {}, { timeout: 3000 });
    expect(screen.getByRole("button", { name: "適用" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "適用" }));

    await waitFor(() => {
      expect(screen.getByText("完了")).toBeInTheDocument();
    });
  });
});

describe("生成の中止", () => {
  it("中止ボタンを押すと、生成を終わらせずに工程タブへ戻る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 80 });
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "次の工程を実行" }));
    await screen.findByText("生成しています…");

    await user.click(screen.getByRole("button", { name: "中止" }));

    await waitFor(() => {
      expect(screen.queryByText("生成しています…")).not.toBeInTheDocument();
    });
    expect(await screen.findByRole("button", { name: "次の工程を実行" })).toBeInTheDocument();
    expect(screen.getByText("生成できます")).toBeInTheDocument();
  });
});

describe("自動で進める", () => {
  it("止めるまで次々に ready な工程を生成・適用し続け、止めるとそこで終わる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 20 });
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "自動で進める" }));

    await waitFor(
      async () => {
        const pipelineSoFar = await backend.pipeline();
        expect(pipelineSoFar.find((step) => step.task.kind === "concept")?.state).toBe("done");
      },
      { timeout: 3000 },
    );

    await user.click(await screen.findByRole("button", { name: "自動で進めるのを止める" }));

    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: "自動で進めるのを止める" }),
      ).not.toBeInTheDocument();
    });
    expect(await screen.findByRole("button", { name: "自動で進める" })).toBeInTheDocument();

    const pipeline = await backend.pipeline();
    const concept = pipeline.find((step) => step.task.kind === "concept");
    expect(concept?.state).toBe("done");
  });
});
