import { useEffect, useState } from "react";
import { useBackend } from "../../api/context";
import type { LlmProvider, LlmSettings, ModelInfo } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";
import { type ConnectionTest, useConnectionTest } from "./useConnectionTest";

export const PROVIDERS: Record<LlmProvider, { title: string; description: string }> = {
  openai_compatible: {
    title: "OpenAI 互換 API",
    description:
      "LM Studio・Ollama などのローカル LLM や、OpenRouter・OpenAI などのクラウドの API を使います。",
  },
  claude_code: {
    title: "Claude Code",
    description:
      "Claude Code（claude -p）で Claude に書かせます。API キーは使わず、Claude Code にログインしているアカウントの利用枠を使います。",
  },
  codex: {
    title: "Codex",
    description:
      "Codex CLI（codex exec）で書かせます。API キーは使わず、Codex にログインしているアカウントの利用枠を使います。",
  },
};

/** 接続先ごとの、モデルの項目の名前（設定にも作品の設定にも同じ名前である）。 */
export const MODEL_KEYS = {
  openai_compatible: "model",
  claude_code: "claude_model",
  codex: "codex_model",
} as const satisfies Record<LlmProvider, keyof LlmSettings>;

/** 推論の強さの候補。モデルによって使えるものが違うので、欄には自由に書ける。 */
export const CODEX_REASONING_EFFORTS = ["low", "medium", "high", "xhigh"];
export const CODEX_EFFORT_LIST_ID = "settings-dialog-codex-efforts";

const MODEL_LIST_ID = "settings-dialog-models";

interface LlmSettingsSectionProps {
  llm: LlmSettings;
  onChange: (llm: LlmSettings) => void;
}

/** 設定ダイアログの LLM の欄。接続先の種類ごとに、必要な項目だけを見せる。 */
export function LlmSettingsSection({ llm, onChange }: LlmSettingsSectionProps) {
  const connectionTest = useConnectionTest();

  function update(changes: Partial<LlmSettings>): void {
    onChange({ ...llm, ...changes });
  }

  function handleProviderChange(provider: LlmProvider): void {
    connectionTest.reset();
    update({ provider });
  }

  const connectionTestButton = <ConnectionTestButton connectionTest={connectionTest} llm={llm} />;
  const modelList = <ModelList id={MODEL_LIST_ID} models={connectionTest.models} />;

  return (
    <section className="settings-dialog__section">
      <h3>LLM</h3>
      <div className="settings-dialog__choices">
        {(Object.keys(PROVIDERS) as LlmProvider[]).map((provider) => (
          <label key={provider} className="settings-dialog__radio">
            <input
              type="radio"
              name="llm-provider"
              checked={llm.provider === provider}
              onChange={() => handleProviderChange(provider)}
            />
            <div>
              <span className="settings-dialog__radio-title">{PROVIDERS[provider].title}</span>
              <p className="settings-dialog__radio-description">
                {PROVIDERS[provider].description}
              </p>
            </div>
          </label>
        ))}
      </div>

      {llm.provider === "openai_compatible" && (
        <>
          <label className="app-field">
            <span>接続先 URL</span>
            <input
              value={llm.base_url}
              onChange={(event) => update({ base_url: event.target.value })}
              placeholder="http://localhost:1234/v1"
            />
          </label>
          <label className="app-field">
            <span>モデル</span>
            <input
              list={MODEL_LIST_ID}
              value={llm.model}
              onChange={(event) => update({ model: event.target.value })}
            />
            {modelList}
          </label>
          {connectionTestButton}
          <ApiKeyField />
        </>
      )}
      {llm.provider === "claude_code" && (
        <>
          <label className="app-field">
            <span>モデル</span>
            <input
              list={MODEL_LIST_ID}
              value={llm.claude_model}
              onChange={(event) => update({ claude_model: event.target.value })}
              placeholder="sonnet"
            />
            {modelList}
          </label>
          <label className="app-field">
            <span>claude コマンド</span>
            <input
              value={llm.claude_command}
              onChange={(event) => update({ claude_command: event.target.value })}
              placeholder="claude"
            />
          </label>
          <p className="settings-dialog__note">
            PATH に無い場合は実行ファイルの場所を指定します（npm でインストールした場合は
            claude.cmd）。temperature と「推論モデルの思考を止める」は使われません。文脈の長さは
            100000 トークン程度まで増やせます。
          </p>
          {connectionTestButton}
        </>
      )}
      {llm.provider === "codex" && (
        <>
          <label className="app-field">
            <span>モデル</span>
            <input
              list={MODEL_LIST_ID}
              value={llm.codex_model}
              onChange={(event) => update({ codex_model: event.target.value })}
              placeholder="gpt-6.1-sol（空欄なら Codex の既定）"
            />
            {modelList}
          </label>
          <label className="app-field">
            <span>推論の強さ</span>
            <input
              list={CODEX_EFFORT_LIST_ID}
              value={llm.codex_reasoning_effort}
              onChange={(event) => update({ codex_reasoning_effort: event.target.value })}
              placeholder="high（空欄なら Codex の既定）"
            />
            <CodexEffortList />
          </label>
          <label className="app-field">
            <span>codex コマンド</span>
            <input
              value={llm.codex_command}
              onChange={(event) => update({ codex_command: event.target.value })}
              placeholder="codex"
            />
          </label>
          <p className="settings-dialog__note">
            PATH に無い場合は実行ファイルの場所を指定します（npm でインストールした場合は
            codex.cmd）。temperature と「推論モデルの思考を止める」は使われません。文脈の長さは
            100000 トークン程度まで増やせます。Codex
            の個人用の指示（~/.codex/AGENTS.md）は読み込まれます。
          </p>
          {connectionTestButton}
        </>
      )}
    </section>
  );
}

interface ConnectionTestButtonProps {
  connectionTest: ConnectionTest;
  llm: LlmSettings;
}

/** `llm` で接続を試すボタン。 */
export function ConnectionTestButton({ connectionTest, llm }: ConnectionTestButtonProps) {
  return (
    <button
      type="button"
      className="app-button"
      disabled={connectionTest.isTesting}
      onClick={() => void connectionTest.run(llm)}
    >
      {connectionTest.isTesting ? "確認しています…" : "接続テスト"}
    </button>
  );
}

interface ModelListProps {
  /** モデル欄の `list` 属性に渡す id。 */
  id: string;
  models: ModelInfo[];
}

/** 接続テストで見つかったモデルを、モデル欄の候補にする。 */
export function ModelList({ id, models }: ModelListProps) {
  return (
    <datalist id={id}>
      {models.map((model) => (
        <option key={model.id} value={model.id} />
      ))}
    </datalist>
  );
}

/** 推論の強さの候補。 */
export function CodexEffortList() {
  return (
    <datalist id={CODEX_EFFORT_LIST_ID}>
      {CODEX_REASONING_EFFORTS.map((effort) => (
        <option key={effort} value={effort} />
      ))}
    </datalist>
  );
}

/** API キーの欄。値そのものは表示せず、設定済みかどうかだけを見せる。 */
function ApiKeyField() {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const [hasStoredApiKey, setHasStoredApiKey] = useState(false);
  const [apiKeyInput, setApiKeyInput] = useState("");

  useEffect(() => {
    backend
      .hasApiKey()
      .then(setHasStoredApiKey)
      .catch((error: unknown) => {
        showToast(toErrorMessage(error, "API キーの状態を確認できませんでした。"), "error");
      });
  }, [backend, showToast]);

  async function handleSetApiKey(): Promise<void> {
    if (apiKeyInput.trim().length === 0) {
      return;
    }
    try {
      await backend.setApiKey(apiKeyInput.trim());
      setApiKeyInput("");
      setHasStoredApiKey(true);
      showToast("API キーを保存しました。");
    } catch (error) {
      showToast(toErrorMessage(error, "API キーを保存できませんでした。"), "error");
    }
  }

  async function handleClearApiKey(): Promise<void> {
    try {
      await backend.setApiKey(null);
      setHasStoredApiKey(false);
      showToast("API キーを削除しました。");
    } catch (error) {
      showToast(toErrorMessage(error, "API キーを削除できませんでした。"), "error");
    }
  }

  return (
    <label className="app-field">
      <span>API キー{hasStoredApiKey && "（設定済み）"}</span>
      <div className="settings-dialog__api-key-row">
        <input
          type="password"
          value={apiKeyInput}
          onChange={(event) => setApiKeyInput(event.target.value)}
          placeholder={hasStoredApiKey ? "変更する場合のみ入力" : "未設定"}
        />
        <button type="button" className="app-button" onClick={() => void handleSetApiKey()}>
          設定
        </button>
        {hasStoredApiKey && (
          <button type="button" className="app-button" onClick={() => void handleClearApiKey()}>
            削除
          </button>
        )}
      </div>
    </label>
  );
}
