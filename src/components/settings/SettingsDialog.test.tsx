import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createMockBackend } from "../../api/mock";
import type { ModelInfo } from "../../api/types";
import { useSettingsStore } from "../../store/settingsStore";
import { renderWithBackend } from "../../test/renderWithBackend";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
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

  it("Claude Code に切り替えると URL と API キーの欄が消え、claude コマンドとモデルを保存できる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await useSettingsStore.getState().load(backend);
    renderWithBackend(<SettingsDialog onClose={() => {}} />, backend);

    await user.click(await screen.findByRole("radio", { name: /Claude Code/ }));

    expect(screen.queryByDisplayValue("http://localhost:1234/v1")).not.toBeInTheDocument();
    expect(screen.queryByText(/^API キー/)).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "claude コマンド" }), {
      target: { value: "C:/tools/claude.cmd" },
    });
    fireEvent.change(screen.getByDisplayValue("sonnet"), { target: { value: "opus" } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(async () => {
      const saved = await backend.loadSettings();
      expect(saved.llm.provider).toBe("claude_code");
      expect(saved.llm.claude_command).toBe("C:/tools/claude.cmd");
      expect(saved.llm.claude_model).toBe("opus");
      expect(saved.llm.base_url).toBe("http://localhost:1234/v1");
    });
  });

  it("Claude Code の接続テストでは、ログインしていることを知らせる", async () => {
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

    await user.click(await screen.findByRole("radio", { name: /Claude Code/ }));
    await user.click(screen.getByRole("button", { name: "接続テスト" }));

    expect(await screen.findByText(/ログインしています/)).toBeInTheDocument();
  });

  it("接続テストの途中で接続先を切り替えると、前の接続先の結果は出さず、すぐに試し直せる", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    let finishFirstTest: (models: ModelInfo[]) => void = () => {};
    const backend = wrapBackend(inner, {
      listModels: (llm) =>
        llm?.provider === "openai_compatible"
          ? new Promise((resolve) => {
              finishFirstTest = resolve;
            })
          : inner.listModels(llm),
    });
    await useSettingsStore.getState().load(backend);
    renderWithBackend(
      <>
        <ToastHost />
        <SettingsDialog onClose={() => {}} />
      </>,
      backend,
    );

    await user.click(await screen.findByRole("button", { name: "接続テスト" }));
    await user.click(screen.getByRole("radio", { name: /Claude Code/ }));
    const testButton = screen.getByRole("button", { name: "接続テスト" });
    expect(testButton).toBeEnabled();

    finishFirstTest([{ id: "old-server-model", context_length: 4096 }]);
    await user.click(testButton);

    expect(await screen.findByText(/ログインしています/)).toBeInTheDocument();
    expect(screen.queryByText(/件のモデルが見つかりました/)).not.toBeInTheDocument();
    expect(document.querySelector('option[value="old-server-model"]')).toBeNull();
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
