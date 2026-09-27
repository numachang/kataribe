import type { ReactNode } from "react";
import type { AppSettings, DraftUnit, LlmProvider, ProjectSettings } from "../../api/types";
import { applyProjectSettings, withProjectSetting } from "../../lib/projectSettings";
import { ConnectionTestButton, ModelList, PROVIDERS } from "./LlmSettingsSection";
import {
  GENERATION_NUMBER_LIMITS,
  type GenerationNumberKey,
  sanitizeGenerationNumber,
} from "./settingsNumbers";
import { useConnectionTest } from "./useConnectionTest";

const MODEL_LIST_ID = "settings-dialog-project-models";

const DRAFT_UNIT_LABELS: Record<DraftUnit, string> = {
  chapter: "章（chapter）",
  scene: "シーン（scene）",
  beat: "ビート（beat）",
};

interface ProjectSettingsSectionProps {
  /** アプリ全体の設定（編集中のもの）。チェックを入れていない項目は、この値を使う。 */
  app: AppSettings;
  value: ProjectSettings;
  onChange: (value: ProjectSettings) => void;
}

/** 設定ダイアログの「この作品」の欄。チェックを入れた項目だけ、この作品で変える。 */
export function ProjectSettingsSection({ app, value, onChange }: ProjectSettingsSectionProps) {
  const connectionTest = useConnectionTest();
  const effective = applyProjectSettings(app, value);
  const modelKey = effective.llm.provider === "openai_compatible" ? "model" : "claude_model";

  function set<K extends keyof ProjectSettings>(key: K, next: ProjectSettings[K] | undefined) {
    onChange(withProjectSetting(value, key, next));
  }

  /** チェックを入れたら、いまのアプリ全体の値から始める。外したら、アプリ全体の値に戻す。 */
  function override<K extends keyof ProjectSettings>(key: K, inherited: ProjectSettings[K]) {
    return (overridden: boolean) => set(key, overridden ? inherited : undefined);
  }

  function handleProviderChange(provider: LlmProvider): void {
    connectionTest.reset();
    set("provider", provider);
  }

  function numberRow(key: GenerationNumberKey, label: string, step?: number) {
    const written = value[key];
    // kataribe.yaml に手で書かれた範囲外の値は、生成するときに範囲に収めて使われる
    const usedValue =
      written !== undefined && Number.isFinite(written)
        ? sanitizeGenerationNumber(key, written)
        : null;
    return (
      <>
        <OverrideRow
          label={label}
          overridden={value[key] !== undefined}
          inherited={String(app.generation[key])}
          onOverride={override(key, app.generation[key])}
        >
          {(disabled) => (
            <input
              type="number"
              className="app-input"
              aria-label={label}
              min={GENERATION_NUMBER_LIMITS[key].min}
              max={GENERATION_NUMBER_LIMITS[key].max}
              step={step}
              disabled={disabled}
              value={effective.generation[key]}
              onChange={(event) => set(key, event.target.valueAsNumber)}
            />
          )}
        </OverrideRow>
        {usedValue !== null && usedValue !== written && (
          <p className="settings-dialog__note">
            {label}の {written} は使える範囲の外なので、実際には {usedValue} で使います。
          </p>
        )}
      </>
    );
  }

  function switchRow(key: "polish" | "disable_thinking", label: string) {
    return (
      <OverrideRow
        label={label}
        overridden={value[key] !== undefined}
        inherited={app.generation[key] ? "する" : "しない"}
        onOverride={override(key, app.generation[key])}
      >
        {(disabled) => (
          <label className="settings-dialog__checkbox">
            <input
              type="checkbox"
              disabled={disabled}
              checked={effective.generation[key]}
              onChange={(event) => set(key, event.target.checked)}
            />
            <span>する</span>
          </label>
        )}
      </OverrideRow>
    );
  }

  return (
    <>
      <p className="settings-dialog__note">
        チェックを入れた項目だけ、この作品で変えます（作品フォルダの kataribe.yaml
        に保存します）。ほかの項目は、アプリ全体の設定を使います。
      </p>

      <section className="settings-dialog__section">
        <h3>LLM</h3>
        <OverrideRow
          label="接続先の種類"
          overridden={value.provider !== undefined}
          inherited={PROVIDERS[app.llm.provider].title}
          onOverride={(overridden) => {
            connectionTest.reset();
            override("provider", app.llm.provider)(overridden);
          }}
        >
          {(disabled) => (
            <select
              className="app-input"
              aria-label="接続先の種類"
              disabled={disabled}
              value={effective.llm.provider}
              onChange={(event) => handleProviderChange(event.target.value as LlmProvider)}
            >
              {(Object.keys(PROVIDERS) as LlmProvider[]).map((provider) => (
                <option key={provider} value={provider}>
                  {PROVIDERS[provider].title}
                </option>
              ))}
            </select>
          )}
        </OverrideRow>
        <OverrideRow
          label={`モデル（${PROVIDERS[effective.llm.provider].title}）`}
          overridden={value[modelKey] !== undefined}
          inherited={app.llm[modelKey] || "未設定"}
          onOverride={override(modelKey, app.llm[modelKey])}
        >
          {(disabled) => (
            <>
              <input
                aria-label="モデル"
                className="app-input"
                list={MODEL_LIST_ID}
                disabled={disabled}
                value={effective.llm[modelKey]}
                onChange={(event) => set(modelKey, event.target.value)}
              />
              <ModelList id={MODEL_LIST_ID} models={connectionTest.models} />
            </>
          )}
        </OverrideRow>
        <ConnectionTestButton connectionTest={connectionTest} llm={effective.llm} />
        <p className="settings-dialog__note">
          接続先 URL・claude コマンドの場所・API キーは PC
          ごとの設定なので、「アプリ全体」で変えます。
        </p>
      </section>

      <section className="settings-dialog__section">
        <h3>生成</h3>
        <OverrideRow
          label="生成単位"
          overridden={value.draft_unit !== undefined}
          inherited={DRAFT_UNIT_LABELS[app.generation.draft_unit]}
          onOverride={override("draft_unit", app.generation.draft_unit)}
        >
          {(disabled) => (
            <select
              className="app-input"
              aria-label="生成単位"
              disabled={disabled}
              value={effective.generation.draft_unit}
              onChange={(event) => set("draft_unit", event.target.value as DraftUnit)}
            >
              {(Object.keys(DRAFT_UNIT_LABELS) as DraftUnit[]).map((unit) => (
                <option key={unit} value={unit}>
                  {DRAFT_UNIT_LABELS[unit]}
                </option>
              ))}
            </select>
          )}
        </OverrideRow>
        {numberRow("chars_per_call", "1 回あたりの文字数")}
        {numberRow("context_tokens", "文脈の長さ（トークン）")}
        {numberRow("temperature", "temperature", 0.1)}
        {numberRow("quality_retries", "品質チェックの再生成回数")}
        {switchRow("polish", "本文を書いたあとに推敲パスをかける")}
        {switchRow("disable_thinking", "推論モデルの思考を止める")}
      </section>
    </>
  );
}

interface OverrideRowProps {
  label: string;
  /** この作品で変えているか。 */
  overridden: boolean;
  /** アプリ全体の設定の値（表示用）。 */
  inherited: string;
  onOverride: (overridden: boolean) => void;
  /** 入力欄。この作品で変えていないときは `disabled` にして、アプリ全体の値を見せる。 */
  children: (disabled: boolean) => ReactNode;
}

/** 「この作品で変える」のチェックと、その項目の入力欄。 */
function OverrideRow({ label, overridden, inherited, onOverride, children }: OverrideRowProps) {
  return (
    <div className="settings-dialog__override">
      <label className="settings-dialog__checkbox">
        <input
          type="checkbox"
          aria-label={`${label}をこの作品で変える`}
          checked={overridden}
          onChange={(event) => onOverride(event.target.checked)}
        />
        <span className="settings-dialog__override-label">{label}</span>
      </label>
      {children(!overridden)}
      {!overridden && (
        <p className="settings-dialog__note">アプリ全体の設定（{inherited}）を使います。</p>
      )}
    </div>
  );
}
