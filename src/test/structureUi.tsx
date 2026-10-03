import { screen } from "@testing-library/react";
import type userEvent from "@testing-library/user-event";
import type { Backend } from "../api/backend";
import { SAMPLE_PROJECT_FOLDER } from "../api/mock/sampleProject";
import { WorkspaceScreen } from "../components/workspace/WorkspaceScreen";
import { useUiStore } from "../store/uiStore";
import { useWorkspaceStore } from "../store/workspaceStore";
import { renderWithBackend } from "./renderWithBackend";

// 構成の操作（目次の追加・削除）の画面のテストが共有する部品。

/** サンプル作品を開き、作業画面を描画する。 */
export async function renderWorkspace(backend: Backend): Promise<void> {
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
  renderWithBackend(<WorkspaceScreen />, backend);
}

/**
 * 目次の行の操作メニューを開いて、項目を選ぶ。
 * 章立ての行（プロットの節）は、本文の章見出しと同じ題なので、`kind` に「章立ての」を渡して見分ける。
 */
export async function openRowMenu(
  user: ReturnType<typeof userEvent.setup>,
  label: string,
  item: string,
  kind = "",
): Promise<void> {
  await user.click(screen.getByRole("button", { name: `「${label}」の${kind}操作` }));
  await user.click(screen.getByRole("menuitem", { name: item }));
}

/** 知らせ（トースト）に、この文を含むものが出ているか。 */
export function hasToast(text: string): boolean {
  return useUiStore.getState().toasts.some((toast) => toast.message.includes(text));
}
