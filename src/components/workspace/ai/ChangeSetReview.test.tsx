import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Backend } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { renderWithBackend } from "../../../test/renderWithBackend";
import { resetAllStores } from "../../../test/resetStores";
import { wrapBackend } from "../../../test/wrapBackend";
import { WorkspaceScreen } from "../WorkspaceScreen";

beforeEach(resetAllStores);
afterEach(resetAllStores);

async function createEmptyProject(backend: Backend): Promise<void> {
  const overview = await backend.createProject("C:\\projects\\test-apply-failure", {
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

/** 最初の 1 回だけ applyChangeSet を失敗させ、以降は本来のバックエンドに委ねる。 */
function withFailingFirstApply(inner: Backend): Backend {
  let calls = 0;
  return wrapBackend(inner, {
    async applyChangeSet(changeSet) {
      calls += 1;
      if (calls === 1) {
        throw new Error("ディスクへの書き込みに失敗しました。");
      }
      return inner.applyChangeSet(changeSet);
    },
  });
}

describe("変更案の適用に失敗したとき", () => {
  it("reviewing のまま変更案が残り、下にエラーを表示する", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 5 });
    const backend = withFailingFirstApply(inner);
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByText("企画を生成しました", {}, { timeout: 3000 });

    await user.click(screen.getByRole("button", { name: "適用" }));

    expect(await screen.findByText("ディスクへの書き込みに失敗しました。")).toBeInTheDocument();
    // 変更案自体は見直せるように残っている（工程タブに切り替わっていない）。
    expect(screen.getByText("企画を生成しました")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "適用" })).toBeInTheDocument();
  });

  it("適用中はボタンが無効になる", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 5 });
    let releaseApply!: () => void;
    const gate = new Promise<void>((resolve) => {
      releaseApply = resolve;
    });
    const backend = wrapBackend(inner, {
      async applyChangeSet(changeSet) {
        await gate;
        return inner.applyChangeSet(changeSet);
      },
    });
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByText("企画を生成しました", {}, { timeout: 3000 });

    const applyButton = screen.getByRole("button", { name: "適用" });
    await user.click(applyButton);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "適用しています…" })).toBeDisabled();
    });
    expect(screen.getByRole("button", { name: "破棄" })).toBeDisabled();

    releaseApply();
    await waitFor(() => {
      expect(screen.queryByText("企画を生成しました")).not.toBeInTheDocument();
    });
  });
});

describe("自動で進めるループと適用の失敗", () => {
  it("適用に失敗したら、同じ工程を繰り返さずに止まる", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 5 });
    const backend = withFailingFirstApply(inner);
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "自動で進める" }));

    await waitFor(() => {
      expect(screen.getByText("ディスクへの書き込みに失敗しました。")).toBeInTheDocument();
    });

    // 止まったあとも「企画」は未完了のまま（適用が失敗しているので）。
    const pipeline = await inner.pipeline();
    expect(pipeline.find((step) => step.task.kind === "concept")?.state).toBe("ready");

    // 自動で進めるは終わっている（「止める」ボタンは消えている）。
    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: "自動で進めるのを止める" }),
      ).not.toBeInTheDocument();
    });
  });
});
