import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { createMockBackend } from "./api/mock";
import { renderWithBackend } from "./test/renderWithBackend";
import { resetAllStores } from "./test/resetStores";

beforeEach(resetAllStores);
afterEach(resetAllStores);

describe("新しい作品の作成", () => {
  it("題名を入力してフォルダを選び、作成すると作業画面に入る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({
      delayMs: 0,
      pickFolder: async () => "C:\\projects\\新しい物語",
    });
    renderWithBackend(<App />, backend);

    await user.click(await screen.findByRole("button", { name: "新しい作品" }));

    await user.type(screen.getByPlaceholderText("作品の題名"), "夜明けの街で");
    await user.click(screen.getByRole("button", { name: "フォルダを選ぶ" }));
    await screen.findByText("C:\\projects\\新しい物語");

    await user.click(screen.getByRole("button", { name: "作成する" }));

    expect(await screen.findByRole("button", { name: "作品を閉じる" })).toBeInTheDocument();
    expect(screen.getAllByText("夜明けの街で").length).toBeGreaterThan(0);
  });

  it("題名が未入力のままでは作成できない", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({
      delayMs: 0,
      pickFolder: async () => "C:\\projects\\新しい物語",
    });
    renderWithBackend(<App />, backend);

    await user.click(await screen.findByRole("button", { name: "新しい作品" }));
    await user.click(screen.getByRole("button", { name: "フォルダを選ぶ" }));
    await screen.findByText("C:\\projects\\新しい物語");

    expect(screen.getByRole("button", { name: "作成する" })).toBeDisabled();
  });
});

describe("最近の作品を開く", () => {
  it("最近の作品の一覧から選ぶと作業画面に入る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    renderWithBackend(<App />, backend);

    const recentButton = await screen.findByRole("button", { name: /月霧の館/ });
    await user.click(recentButton);

    expect(await screen.findByRole("button", { name: "作品を閉じる" })).toBeInTheDocument();
  });

  it("作品を閉じると開始画面に戻る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    renderWithBackend(<App />, backend);

    await user.click(await screen.findByRole("button", { name: /月霧の館/ }));
    await screen.findByRole("button", { name: "作品を閉じる" });

    await user.click(screen.getByRole("button", { name: "作品を閉じる" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "新しい作品" })).toBeInTheDocument();
    });
  });
});
