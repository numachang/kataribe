import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createMockBackend } from "../../api/mock";
import { useSettingsStore } from "../../store/settingsStore";
import { renderWithBackend } from "../../test/renderWithBackend";
import { resetAllStores } from "../../test/resetStores";
import { ToastHost } from "../Toast/ToastHost";
import { SettingsDialog } from "./SettingsDialog";

beforeEach(resetAllStores);
afterEach(resetAllStores);

describe("設定の保存", () => {
  it("LLM の接続先を変えて保存すると、次に読み込んだときも反映されている", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await useSettingsStore.getState().load(backend);
    renderWithBackend(<SettingsDialog onClose={() => {}} />, backend);

    const urlInput = await screen.findByDisplayValue("http://localhost:1234/v1");
    fireEvent.change(urlInput, { target: { value: "http://localhost:9999/v1" } });

    const verticalCheckbox = screen.getByRole("checkbox", { name: "既定で縦書きにする" });
    expect(verticalCheckbox).toBeChecked();
    await user.click(verticalCheckbox);

    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(async () => {
      const saved = await backend.loadSettings();
      expect(saved.llm.base_url).toBe("http://localhost:9999/v1");
      expect(saved.editor.vertical).toBe(false);
    });
  });

  it("接続テストを行うとモデルの一覧が見つかる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await useSettingsStore.getState().load(backend);
    renderWithBackend(
      <>
        <ToastHost />
        <SettingsDialog onClose={() => {}} />
      </>,
      backend,
    );

    await user.click(await screen.findByRole("button", { name: "接続テスト" }));

    expect(await screen.findByText(/件のモデルが見つかりました/)).toBeInTheDocument();
  });

  it("API キーを設定すると「設定済み」と表示され、値そのものは表示されない", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await useSettingsStore.getState().load(backend);
    renderWithBackend(<SettingsDialog onClose={() => {}} />, backend);

    const apiKeyInput = await screen.findByPlaceholderText("未設定");
    await user.type(apiKeyInput, "sk-test-12345");
    await user.click(screen.getByRole("button", { name: "設定" }));

    expect(await screen.findByText("API キー（設定済み）")).toBeInTheDocument();
    expect(screen.queryByDisplayValue("sk-test-12345")).not.toBeInTheDocument();
    expect(await backend.hasApiKey()).toBe(true);
  });
});
