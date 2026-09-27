/**
 * 生成の数値の設定ごとの、使える範囲と、空欄・不正な入力のときに代わりに使う値。
 * 範囲は Rust の `GenerationSettings::sanitized` と同じにする（画面と生成とで値が食い違わないように）。
 * アプリ全体の設定と作品の設定で同じ規則を使う。
 */
export const GENERATION_NUMBER_LIMITS = {
  chars_per_call: { min: 200, max: 20000, fallback: 1500 },
  context_tokens: { min: 2048, max: 1_000_000, fallback: 16384 },
  temperature: { min: 0, max: 2, fallback: 0.8 },
  quality_retries: { min: 0, max: 5, fallback: 1 },
} as const;

export type GenerationNumberKey = keyof typeof GENERATION_NUMBER_LIMITS;

export const GENERATION_NUMBER_KEYS = Object.keys(
  GENERATION_NUMBER_LIMITS,
) as GenerationNumberKey[];

/**
 * 数値入力欄の値を検証・補正する。
 * `<input type="number">` の `valueAsNumber` は、空欄や不正な入力のとき NaN になる
 * （`Number(el.value)` と違い、空欄が 0 になったりしない）。編集中はそのまま NaN を保持させ、
 * 保存の直前にだけ、この関数で下限に満たない・NaN な値を規定値に補正する。
 */
export function sanitizeNumber(value: number, min: number, fallback: number): number {
  return Number.isFinite(value) && value >= min ? value : fallback;
}

/**
 * 生成の数値の設定を、{@link GENERATION_NUMBER_LIMITS} の規則で補正する。
 * 空欄・不正な入力は既定値にし、範囲を外れた値は範囲の端に収める。
 */
export function sanitizeGenerationNumber(key: GenerationNumberKey, value: number): number {
  const { min, max, fallback } = GENERATION_NUMBER_LIMITS[key];
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(Math.max(value, min), max);
}
