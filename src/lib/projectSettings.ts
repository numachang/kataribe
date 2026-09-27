import type { GenerationSettings, LlmSettings, ProjectSettings } from "../api/types";

/** 作品の設定ファイル（作品情報）のパス。 */
export const MANIFEST_PATH = "kataribe.yaml";

// Record にしておくと、型に項目が増えたのに足し忘れたときに型エラーになる
const KNOWN_PROJECT_SETTINGS: Record<keyof ProjectSettings, true> = {
  provider: true,
  model: true,
  claude_model: true,
  draft_unit: true,
  chars_per_call: true,
  context_tokens: true,
  temperature: true,
  polish: true,
  quality_retries: true,
  disable_thinking: true,
};

/**
 * この版が知っている作品の設定の項目。画面が受け取る設定には、この版が知らない項目（新しい版で足された
 * 設定）も入りうるので、数えたり比べたりするときはこの項目だけを見る。
 */
export const PROJECT_SETTING_KEYS = Object.keys(
  KNOWN_PROJECT_SETTINGS,
) as (keyof ProjectSettings)[];

/** この作品で変えている（書いてある）項目の数。 */
export function countProjectSettings(settings: ProjectSettings): number {
  return PROJECT_SETTING_KEYS.filter((key) => settings[key] !== undefined).length;
}

/** 作品の設定を重ねる前の、アプリ全体の LLM と生成の設定。 */
export interface EngineSettingsLike {
  llm: LlmSettings;
  generation: GenerationSettings;
}

/**
 * アプリ全体の設定に、作品の設定で書いてある項目だけを重ねる（Rust の `ProjectSettings::apply` と同じ規則）。
 * 画面で「実際に使う値」を見せるのと、偽バックエンドの生成に使う。値の範囲の補正はしない。
 */
export function applyProjectSettings(
  base: EngineSettingsLike,
  project: ProjectSettings,
): EngineSettingsLike {
  return {
    llm: {
      ...base.llm,
      provider: project.provider ?? base.llm.provider,
      model: project.model ?? base.llm.model,
      claude_model: project.claude_model ?? base.llm.claude_model,
    },
    generation: {
      ...base.generation,
      draft_unit: project.draft_unit ?? base.generation.draft_unit,
      chars_per_call: project.chars_per_call ?? base.generation.chars_per_call,
      context_tokens: project.context_tokens ?? base.generation.context_tokens,
      temperature: project.temperature ?? base.generation.temperature,
      polish: project.polish ?? base.generation.polish,
      quality_retries: project.quality_retries ?? base.generation.quality_retries,
      disable_thinking: project.disable_thinking ?? base.generation.disable_thinking,
    },
  };
}

/** この版が知っている項目の値がすべて同じか（項目の順番は問わない）。 */
export function isSameProjectSettings(left: ProjectSettings, right: ProjectSettings): boolean {
  return PROJECT_SETTING_KEYS.every((key) => left[key] === right[key]);
}

/** `key` の項目を `value` にした設定を返す。`undefined` なら項目ごと消す（アプリ全体の設定に戻す）。 */
export function withProjectSetting<K extends keyof ProjectSettings>(
  settings: ProjectSettings,
  key: K,
  value: ProjectSettings[K] | undefined,
): ProjectSettings {
  const next = { ...settings };
  if (value === undefined) {
    delete next[key];
  } else {
    next[key] = value;
  }
  return next;
}
