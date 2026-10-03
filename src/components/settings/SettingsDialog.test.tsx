import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import type { ModelInfo, ProjectSettings } from "../../api/types";
import { documentSaveController } from "../../features/editor/documentSaveController";
import { useSettingsStore } from "../../store/settingsStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { editorBody, editText, loadIntoEditor } from "../../test/documents";
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

/** 作品の設定を、読んだときのハッシュを使って保存しておく（ダイアログを開く前の状態を作る）。 */
async function storeProjectSettings(backend: Backend, settings: ProjectSettings): Promise<void> {
  const { hash } = await backend.loadProjectSettings();
  await backend.saveProjectSettings(settings, hash);
}

async function storedProjectSettings(backend: Backend): Promise<ProjectSettings> {
  return (await backend.loadProjectSettings()).settings;
}

/** サンプル作品を開いた状態で、設定ダイアログを表示する。 */
async function renderWithOpenProject(backend: Backend = createMockBackend({ delayMs: 0 })) {
  await useSettingsStore.getState().load(backend);
  const overview = await backend.openProject(SAMPLE_PROJECT_FOLDER);
  useWorkspaceStore.getState().openWorkspace(overview);
  renderWithBackend(
    <>
      <ToastHost />
      <SettingsDialog onClose={() => {}} />
    </>,
    backend,
  );
  return backend;
}

describe("作品ごとの設定", () => {
  it("作品を開いていなければ、設定する範囲の切り替えは出ない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await useSettingsStore.getState().load(backend);
    renderWithBackend(<SettingsDialog onClose={() => {}} />, backend);

    await screen.findByText("生成");
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();
  });

  it("この作品だけ Claude Code に変えて保存でき、アプリ全体の設定は変わらない", async () => {
    const user = userEvent.setup();
    const backend = await renderWithOpenProject();

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));
    await user.click(
      await screen.findByRole("checkbox", { name: "接続先の種類をこの作品で変える" }),
    );
    await user.selectOptions(screen.getByRole("combobox", { name: "接続先の種類" }), "claude_code");
    await user.click(
      screen.getByRole("checkbox", { name: "モデル（Claude Code）をこの作品で変える" }),
    );
    const modelInput = screen.getByRole("combobox", { name: "モデル" });
    await user.clear(modelInput);
    await user.type(modelInput, "haiku");
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(async () => {
      expect(await storedProjectSettings(backend)).toEqual({
        provider: "claude_code",
        claude_model: "haiku",
      });
    });
    const appSettings = await backend.loadSettings();
    expect(appSettings.llm.provider).toBe("openai_compatible");
    expect(appSettings.llm.claude_model).toBe("sonnet");
  });

  it("この作品で変えていない項目は、アプリ全体の値を見せて、変えられないようにする", async () => {
    const user = userEvent.setup();
    await renderWithOpenProject();

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));

    const charsPerCall = await screen.findByRole("spinbutton", { name: "1 回あたりの文字数" });
    expect(charsPerCall).toBeDisabled();
    expect(charsPerCall).toHaveValue(1500);
    expect(screen.getByText("アプリ全体の設定（1500）を使います。")).toBeInTheDocument();

    await user.click(
      screen.getByRole("checkbox", { name: "1 回あたりの文字数をこの作品で変える" }),
    );

    expect(charsPerCall).toBeEnabled();
    expect(screen.queryByText("アプリ全体の設定（1500）を使います。")).not.toBeInTheDocument();
  });

  it("チェックを外すと、その項目は作品の設定から消える", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);
    await storeProjectSettings(backend, { polish: true, temperature: 0.3 });
    await renderWithOpenProject(backend);

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));
    await user.click(
      await screen.findByRole("checkbox", { name: "temperatureをこの作品で変える" }),
    );
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(async () => {
      expect(await storedProjectSettings(backend)).toEqual({ polish: true });
    });
  });

  it("アプリ全体の画面では、作品の設定が優先されることを知らせる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);
    await storeProjectSettings(backend, { provider: "claude_code", context_tokens: 100000 });
    await renderWithOpenProject(backend);

    expect(await screen.findByText(/作品ごとの設定が 2 項目あり/)).toBeInTheDocument();
  });

  it("作品の生成単位を変えると、工程の一覧を読み直す", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    let pipelineCalls = 0;
    const backend = wrapBackend(inner, {
      pipeline: () => {
        pipelineCalls += 1;
        return inner.pipeline();
      },
    });
    await renderWithOpenProject(backend);

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));
    await user.click(await screen.findByRole("checkbox", { name: "生成単位をこの作品で変える" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "生成単位" }), "scene");
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(pipelineCalls).toBe(1));
    expect(await storedProjectSettings(backend)).toEqual({ draft_unit: "scene" });
  });

  it("作品の設定を触らずに保存したときは、kataribe.yaml を書き換えない", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    await inner.openProject(SAMPLE_PROJECT_FOLDER);
    // CLI などで範囲外の値が書かれていても、触っていなければそのまま残す
    await storeProjectSettings(inner, { chars_per_call: 50 });
    let projectSaves = 0;
    const backend = wrapBackend(inner, {
      saveProjectSettings: (settings, hash) => {
        projectSaves += 1;
        return inner.saveProjectSettings(settings, hash);
      },
    });
    await renderWithOpenProject(backend);

    await user.click(await screen.findByRole("checkbox", { name: "既定で縦書きにする" }));
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(await screen.findByText("設定を保存しました。")).toBeInTheDocument();
    expect(projectSaves).toBe(0);
    expect(await storedProjectSettings(inner)).toEqual({ chars_per_call: 50 });
  });

  it("作品の設定だけ保存に失敗したら、アプリ全体の設定は保存できたことと一緒に知らせる", async () => {
    const user = userEvent.setup();
    const backend = await renderWithOpenProject();

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));
    await user.click(
      await screen.findByRole("checkbox", { name: "推論モデルの思考を止めるをこの作品で変える" }),
    );
    // ダイアログを開いたあとに、外で kataribe.yaml が変わった
    await storeProjectSettings(backend, { polish: true });
    await user.click(screen.getByRole("tab", { name: "アプリ全体" }));
    await user.click(screen.getByRole("checkbox", { name: "既定で縦書きにする" }));
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(
      await screen.findByText(
        /アプリ全体の設定は保存しましたが、作品の設定は保存できませんでした。.*開き直して/,
      ),
    ).toBeInTheDocument();
    expect((await backend.loadSettings()).editor.vertical).toBe(false);
    expect(await storedProjectSettings(backend)).toEqual({ polish: true });
  });

  it("作品情報をエディタで開いていれば、作品の設定を保存したあとに読み直す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);
    await renderWithOpenProject(backend);
    useWorkspaceStore.getState().openDocument("kataribe.yaml");
    await loadIntoEditor(backend, "kataribe.yaml");

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));
    await user.click(await screen.findByRole("checkbox", { name: "生成単位をこの作品で変える" }));
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(editorBody()).toContain("draft_unit: beat");
    });
  });

  it("タブを切り替えても、入力中の API キーは消えない", async () => {
    const user = userEvent.setup();
    await renderWithOpenProject();

    await user.type(await screen.findByPlaceholderText("未設定"), "sk-typing");
    await user.click(screen.getByRole("tab", { name: "この作品（月霧の館）" }));
    await user.click(screen.getByRole("tab", { name: "アプリ全体" }));

    expect(screen.getByPlaceholderText("未設定")).toHaveValue("sk-typing");
  });

  it("作品情報に保存前の編集があっても、その編集を保存したうえで作品の設定を保存できる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await useSettingsStore.getState().load(backend);
    useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
    useWorkspaceStore.getState().openDocument("kataribe.yaml");
    await loadIntoEditor(backend, "kataribe.yaml");
    // 入力したときと同じく、内容を変えて保存の予約をする（まだ保存されていない）
    editText(`${editorBody()}# 手で足したメモ
`);
    documentSaveController.notifyChange(backend);
    renderWithBackend(
      <>
        <ToastHost />
        <SettingsDialog onClose={() => {}} />
      </>,
      backend,
    );

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));
    await user.click(await screen.findByRole("checkbox", { name: "生成単位をこの作品で変える" }));
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(await screen.findByText("設定を保存しました。")).toBeInTheDocument();
    expect(await storedProjectSettings(backend)).toEqual({ draft_unit: "beat" });
  });

  it("作品の設定を読み込めなければ、「この作品」にその理由を出す", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const backend = wrapBackend(inner, {
      loadProjectSettings: async () => {
        throw new Error("kataribe.yaml の settings を読めません");
      },
    });
    await renderWithOpenProject(backend);

    await user.click(await screen.findByRole("tab", { name: "この作品（月霧の館）" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("kataribe.yaml の settings");
  });
});
