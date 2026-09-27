import type { AppSettings, DraftUnit, FontStyle } from "../../api/types";
import { LlmSettingsSection } from "./LlmSettingsSection";
import { GENERATION_NUMBER_LIMITS } from "./settingsNumbers";

const DRAFT_UNIT_DESCRIPTIONS: Record<DraftUnit, string> = {
  chapter: "1 章をまるごと生成します。長い文脈に強い、大きなモデル向けです。",
  scene:
    "1 シーンずつ生成します。長いシーンは何回かに分けて書き継ぎます。中規模以上のモデル向けです。",
  beat: "シーンをさらに細かい展開（ビート）に分けて、1 つずつ生成します（既定）。ローカルのモデルでも分量が安定し、設定から外れにくくなります。",
};

const FONT_STYLE_LABELS: Record<FontStyle, string> = { mincho: "明朝体", gothic: "ゴシック体" };

interface AppSettingsSectionsProps {
  form: AppSettings;
  onChange: (form: AppSettings) => void;
}

/** 設定ダイアログの「アプリ全体」の欄。LLM 接続・生成・エディタの見た目。 */
export function AppSettingsSections({ form, onChange }: AppSettingsSectionsProps) {
  const setGeneration = (changes: Partial<AppSettings["generation"]>) =>
    onChange({ ...form, generation: { ...form.generation, ...changes } });
  const setEditor = (changes: Partial<AppSettings["editor"]>) =>
    onChange({ ...form, editor: { ...form.editor, ...changes } });

  return (
    <>
      <LlmSettingsSection llm={form.llm} onChange={(llm) => onChange({ ...form, llm })} />

      <section className="settings-dialog__section">
        <h3>生成</h3>
        <div className="settings-dialog__choices">
          {(Object.keys(DRAFT_UNIT_DESCRIPTIONS) as DraftUnit[]).map((unit) => (
            <label key={unit} className="settings-dialog__radio">
              <input
                type="radio"
                name="draft-unit"
                checked={form.generation.draft_unit === unit}
                onChange={() => setGeneration({ draft_unit: unit })}
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
              min={GENERATION_NUMBER_LIMITS.chars_per_call.min}
              max={GENERATION_NUMBER_LIMITS.chars_per_call.max}
              value={form.generation.chars_per_call}
              onChange={(event) => setGeneration({ chars_per_call: event.target.valueAsNumber })}
            />
          </label>
          <label className="app-field">
            <span>文脈の長さ（トークン）</span>
            <input
              type="number"
              min={GENERATION_NUMBER_LIMITS.context_tokens.min}
              max={GENERATION_NUMBER_LIMITS.context_tokens.max}
              value={form.generation.context_tokens}
              onChange={(event) => setGeneration({ context_tokens: event.target.valueAsNumber })}
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
              onChange={(event) => setGeneration({ temperature: event.target.valueAsNumber })}
            />
          </label>
          <label className="app-field">
            <span>品質チェックの再生成回数</span>
            <input
              type="number"
              min={0}
              max={5}
              value={form.generation.quality_retries}
              onChange={(event) => setGeneration({ quality_retries: event.target.valueAsNumber })}
            />
          </label>
        </div>

        <label className="settings-dialog__checkbox">
          <input
            type="checkbox"
            checked={form.generation.polish}
            onChange={(event) => setGeneration({ polish: event.target.checked })}
          />
          <span>本文を書いたあとに推敲パスをかける</span>
        </label>

        <label className="settings-dialog__checkbox">
          <input
            type="checkbox"
            checked={form.generation.disable_thinking}
            onChange={(event) => setGeneration({ disable_thinking: event.target.checked })}
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
            onChange={(event) => setEditor({ vertical: event.target.checked })}
          />
          <span>既定で縦書きにする</span>
        </label>

        <div className="settings-dialog__row">
          <label className="app-field">
            <span>書体</span>
            <select
              value={form.editor.font_style}
              onChange={(event) => setEditor({ font_style: event.target.value as FontStyle })}
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
              onChange={(event) => setEditor({ font_size: event.target.valueAsNumber })}
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
              onChange={(event) => setEditor({ line_height: event.target.valueAsNumber })}
            />
          </label>
        </div>
      </section>
    </>
  );
}
