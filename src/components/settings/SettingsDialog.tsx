import { useEffect, useState } from "react";
import { useBackend } from "../../api/context";
import type { AppSettings, DraftUnit, FontStyle, ModelInfo } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { useSettingsStore } from "../../store/settingsStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { Dialog } from "../Dialog";
import "./SettingsDialog.css";

const DRAFT_UNIT_DESCRIPTIONS: Record<DraftUnit, string> = {
  chapter: "1 章をまるごと生成します。長い文脈に強い、大きなモデル向けです。",
  scene:
    "1 シーンずつ生成します。長いシーンは何回かに分けて書き継ぎます。中規模以上のモデル向けです。",
  beat: "シーンをさらに細かい展開（ビート）に分けて、1 つずつ生成します（既定）。ローカルのモデルでも分量が安定し、設定から外れにくくなります。",
};

const FONT_STYLE_LABELS: Record<FontStyle, string> = { mincho: "明朝体", gothic: "ゴシック体" };

/**
 * 数値入力欄の値を検証・補正する。
 * `<input type="number">` の `valueAsNumber` は、空欄や不正な入力のとき NaN になる
 * （`Number(el.value)` と違い、空欄が 0 になったりしない）。編集中はそのまま NaN を保持させ、
 * 保存の直前にだけ、この関数で下限に満たない・NaN な値を規定値に補正する。
 */
function sanitizeNumber(value: number, min: number, fallback: number): number {
  return Number.isFinite(value) && value >= min ? value : fallback;
}

const GENERATION_DEFAULTS = {
  chars_per_call: 1500,
  context_tokens: 16384,
  temperature: 0.8,
  quality_retries: 1,
} as const;

const EDITOR_DEFAULTS = {
  font_size: 17,
  line_height: 1.9,
} as const;

/** 保存の直前に、数値欄の空欄・不正な入力を規定値へ補正した設定を作る。 */
function sanitizeSettings(form: AppSettings): AppSettings {
  return {
    ...form,
    generation: {
      ...form.generation,
      chars_per_call: sanitizeNumber(
        form.generation.chars_per_call,
        100,
        GENERATION_DEFAULTS.chars_per_call,
      ),
      context_tokens: sanitizeNumber(
        form.generation.context_tokens,
        512,
        GENERATION_DEFAULTS.context_tokens,
      ),
      temperature: sanitizeNumber(form.generation.temperature, 0, GENERATION_DEFAULTS.temperature),
      quality_retries: sanitizeNumber(
        form.generation.quality_retries,
        0,
        GENERATION_DEFAULTS.quality_retries,
      ),
    },
    editor: {
      ...form.editor,
      font_size: sanitizeNumber(form.editor.font_size, 10, EDITOR_DEFAULTS.font_size),
      line_height: sanitizeNumber(form.editor.line_height, 1, EDITOR_DEFAULTS.line_height),
    },
  };
}

interface SettingsDialogProps {
  onClose: () => void;
}

/** 設定ダイアログ。LLM 接続・生成・エディタの見た目をまとめて変えられる。 */
export function SettingsDialog({ onClose }: SettingsDialogProps) {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const storedSettings = useSettingsStore((state) => state.settings);

  const [form, setForm] = useState<AppSettings | null>(storedSettings);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [hasStoredApiKey, setHasStoredApiKey] = useState(false);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [isTestingConnection, setIsTestingConnection] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    if (storedSettings && !form) {
      setForm(storedSettings);
    }
  }, [storedSettings, form]);

  useEffect(() => {
    backend
      .hasApiKey()
      .then(setHasStoredApiKey)
      .catch((error: unknown) => {
        showToast(toErrorMessage(error, "API キーの状態を確認できませんでした。"), "error");
      });
  }, [backend, showToast]);

  async function handleTestConnection(): Promise<void> {
    if (!form) {
      return;
    }
    setIsTestingConnection(true);
    try {
      const list = await backend.listModels(form.llm);
      setModels(list);
      showToast(`接続できました。${list.length} 件のモデルが見つかりました。`);
    } catch (error) {
      showToast(toErrorMessage(error, "接続できませんでした。"), "error");
    } finally {
      setIsTestingConnection(false);
    }
  }

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

  async function handleSave(): Promise<void> {
    if (!form) {
      return;
    }
    const draftUnitChanged = storedSettings?.generation.draft_unit !== form.generation.draft_unit;
    const sanitized = sanitizeSettings(form);
    setIsSaving(true);
    try {
      await useSettingsStore.getState().save(backend, sanitized);
      showToast("設定を保存しました。");
      if (draftUnitChanged && useWorkspaceStore.getState().overview) {
        // 生成単位が変わると工程の組み立てが変わりうるため、一覧を読み直す。
        await useWorkspaceStore.getState().refreshPipeline(backend);
      }
      onClose();
    } catch (error) {
      showToast(toErrorMessage(error, "設定を保存できませんでした。"), "error");
    } finally {
      setIsSaving(false);
    }
  }

  if (!form) {
    return (
      <Dialog title="設定" onClose={onClose}>
        <p>読み込んでいます…</p>
      </Dialog>
    );
  }

  return (
    <Dialog title="設定" onClose={onClose} wide>
      <div className="settings-dialog">
        <section className="settings-dialog__section">
          <h3>LLM</h3>
          <label className="app-field">
            <span>接続先 URL</span>
            <input
              value={form.llm.base_url}
              onChange={(event) =>
                setForm({ ...form, llm: { ...form.llm, base_url: event.target.value } })
              }
              placeholder="http://localhost:1234/v1"
            />
          </label>
          <label className="app-field">
            <span>モデル</span>
            <input
              list="settings-dialog-models"
              value={form.llm.model}
              onChange={(event) =>
                setForm({ ...form, llm: { ...form.llm, model: event.target.value } })
              }
            />
            <datalist id="settings-dialog-models">
              {models.map((model) => (
                <option key={model.id} value={model.id} />
              ))}
            </datalist>
          </label>
          <button
            type="button"
            className="app-button"
            disabled={isTestingConnection}
            onClick={() => void handleTestConnection()}
          >
            {isTestingConnection ? "確認しています…" : "接続テスト"}
          </button>

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
                <button
                  type="button"
                  className="app-button"
                  onClick={() => void handleClearApiKey()}
                >
                  削除
                </button>
              )}
            </div>
          </label>
        </section>

        <section className="settings-dialog__section">
          <h3>生成</h3>
          <div className="settings-dialog__draft-units">
            {(Object.keys(DRAFT_UNIT_DESCRIPTIONS) as DraftUnit[]).map((unit) => (
              <label key={unit} className="settings-dialog__radio">
                <input
                  type="radio"
                  name="draft-unit"
                  checked={form.generation.draft_unit === unit}
                  onChange={() =>
                    setForm({ ...form, generation: { ...form.generation, draft_unit: unit } })
                  }
                />
                <div>
                  <span className="settings-dialog__radio-title">{unit}</span>
                  <p className="settings-dialog__radio-description">
                    {DRAFT_UNIT_DESCRIPTIONS[unit]}
                  </p>
                </div>
              </label>
            ))}
          </div>

          <div className="settings-dialog__row">
            <label className="app-field">
              <span>1 回あたりの文字数</span>
              <input
                type="number"
                min={100}
                value={form.generation.chars_per_call}
                onChange={(event) =>
                  setForm({
                    ...form,
                    generation: {
                      ...form.generation,
                      chars_per_call: event.target.valueAsNumber,
                    },
                  })
                }
              />
            </label>
            <label className="app-field">
              <span>文脈の長さ（トークン）</span>
              <input
                type="number"
                min={512}
                value={form.generation.context_tokens}
                onChange={(event) =>
                  setForm({
                    ...form,
                    generation: {
                      ...form.generation,
                      context_tokens: event.target.valueAsNumber,
                    },
                  })
                }
              />
            </label>
          </div>

          <div className="settings-dialog__row">
            <label className="app-field">
              <span>temperature</span>
              <input
                type="number"
                min={0}
                max={2}
                step={0.1}
                value={form.generation.temperature}
                onChange={(event) =>
                  setForm({
                    ...form,
                    generation: { ...form.generation, temperature: event.target.valueAsNumber },
                  })
                }
              />
            </label>
            <label className="app-field">
              <span>品質チェックの再生成回数</span>
              <input
                type="number"
                min={0}
                max={5}
                value={form.generation.quality_retries}
                onChange={(event) =>
                  setForm({
                    ...form,
                    generation: {
                      ...form.generation,
                      quality_retries: event.target.valueAsNumber,
                    },
                  })
                }
              />
            </label>
          </div>

          <label className="settings-dialog__checkbox">
            <input
              type="checkbox"
              checked={form.generation.polish}
              onChange={(event) =>
                setForm({
                  ...form,
                  generation: { ...form.generation, polish: event.target.checked },
                })
              }
            />
            <span>本文を書いたあとに推敲パスをかける</span>
          </label>

          <label className="settings-dialog__checkbox">
            <input
              type="checkbox"
              checked={form.generation.disable_thinking}
              onChange={(event) =>
                setForm({
                  ...form,
                  generation: { ...form.generation, disable_thinking: event.target.checked },
                })
              }
            />
            <span>
              推論モデルの思考を止める（ローカル LLM 向け。クラウドの API でエラーになる場合は外す）
            </span>
          </label>
        </section>

        <section className="settings-dialog__section">
          <h3>エディタ</h3>
          <label className="settings-dialog__checkbox">
            <input
              type="checkbox"
              checked={form.editor.vertical}
              onChange={(event) =>
                setForm({ ...form, editor: { ...form.editor, vertical: event.target.checked } })
              }
            />
            <span>既定で縦書きにする</span>
          </label>

          <div className="settings-dialog__row">
            <label className="app-field">
              <span>書体</span>
              <select
                value={form.editor.font_style}
                onChange={(event) =>
                  setForm({
                    ...form,
                    editor: { ...form.editor, font_style: event.target.value as FontStyle },
                  })
                }
              >
                {(Object.keys(FONT_STYLE_LABELS) as FontStyle[]).map((style) => (
                  <option key={style} value={style}>
                    {FONT_STYLE_LABELS[style]}
                  </option>
                ))}
              </select>
            </label>
            <label className="app-field">
              <span>字の大きさ</span>
              <input
                type="number"
                min={10}
                max={32}
                value={form.editor.font_size}
                onChange={(event) =>
                  setForm({
                    ...form,
                    editor: { ...form.editor, font_size: event.target.valueAsNumber },
                  })
                }
              />
            </label>
            <label className="app-field">
              <span>行間</span>
              <input
                type="number"
                min={1}
                max={3}
                step={0.1}
                value={form.editor.line_height}
                onChange={(event) =>
                  setForm({
                    ...form,
                    editor: { ...form.editor, line_height: event.target.valueAsNumber },
                  })
                }
              />
            </label>
          </div>
        </section>

        <div className="settings-dialog__actions">
          <button type="button" className="app-button" onClick={onClose}>
            キャンセル
          </button>
          <button
            type="button"
            className="app-button app-button--primary"
            disabled={isSaving}
            onClick={() => void handleSave()}
          >
            保存
          </button>
        </div>
      </div>
    </Dialog>
  );
}
