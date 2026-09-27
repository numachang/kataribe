import { useEffect, useRef, useState } from "react";
import { useBackend } from "../../api/context";
import type { LlmProvider, LlmSettings, ModelInfo } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";

const PROVIDERS: Record<LlmProvider, { title: string; description: string }> = {
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
};

const MODEL_LIST_ID = "settings-dialog-models";

interface LlmSettingsSectionProps {
  llm: LlmSettings;
  onChange: (llm: LlmSettings) => void;
}

/** 設定ダイアログの LLM の欄。接続先の種類ごとに、必要な項目だけを見せる。 */
export function LlmSettingsSection({ llm, onChange }: LlmSettingsSectionProps) {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [isTestingConnection, setIsTestingConnection] = useState(false);
  // 接続テストの途中で接続先を切り替えたら、前の接続先の結果を一覧に出さないための通し番号
  const connectionTestId = useRef(0);

  function update(changes: Partial<LlmSettings>): void {
    onChange({ ...llm, ...changes });
  }

  function handleProviderChange(provider: LlmProvider): void {
    connectionTestId.current += 1;
    setModels([]);
    setIsTestingConnection(false);
    update({ provider });
  }

  async function handleTestConnection(): Promise<void> {
    const testId = ++connectionTestId.current;
    const isCurrentTest = () => testId === connectionTestId.current;
    setIsTestingConnection(true);
    try {
      const list = await backend.listModels(llm);
      if (!isCurrentTest()) {
        return;
      }
      setModels(list);
      showToast(
        llm.provider === "claude_code"
          ? "Claude Code を使えます（ログインしています）。"
          : `接続できました。${list.length} 件のモデルが見つかりました。`,
      );
    } catch (error) {
      if (isCurrentTest()) {
        showToast(toErrorMessage(error, "接続できませんでした。"), "error");
      }
    } finally {
      if (isCurrentTest()) {
        setIsTestingConnection(false);
      }
    }
  }

  const connectionTestButton = (
    <button
      type="button"
      className="app-button"
      disabled={isTestingConnection}
      onClick={() => void handleTestConnection()}
    >
      {isTestingConnection ? "確認しています…" : "接続テスト"}
    </button>
  );
  const modelList = (
    <datalist id={MODEL_LIST_ID}>
      {models.map((model) => (
        <option key={model.id} value={model.id} />
      ))}
    </datalist>
  );

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

      {llm.provider === "openai_compatible" ? (
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
      ) : (
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
    </section>
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
