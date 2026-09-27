import type { GenerationEvent } from "../../api/types";

// generate() が届ける GenerationEvent の並びを、画面に表示できる形へ積み上げていく。
// バックエンドや React に依存しない純粋な関数なので、単体テストしやすい。

export interface GenerationNotice {
  level: "info" | "warning";
  message: string;
}

export interface GenerationStepDisplay {
  label: string;
  index: number;
  total: number;
  /** この回を始めた時刻（エポックからのミリ秒）。経過時間の表示に使う。 */
  startedAt: number;
  content: string;
  reasoning: string;
  notices: GenerationNotice[];
  finished: boolean;
  promptTokens: number | null;
  completionTokens: number | null;
  elapsedMs: number | null;
}

export interface GenerationDisplay {
  /** 使っている LLM の名前。生成を始めたときに届く。 */
  model: string | null;
  steps: GenerationStepDisplay[];
}

export function createEmptyGenerationDisplay(): GenerationDisplay {
  return { model: null, steps: [] };
}

function updateLastStep(
  display: GenerationDisplay,
  update: (step: GenerationStepDisplay) => GenerationStepDisplay,
): GenerationDisplay {
  const lastIndex = display.steps.length - 1;
  const last = display.steps[lastIndex];
  if (last === undefined) {
    return display;
  }
  const steps = [...display.steps];
  steps[lastIndex] = update(last);
  return { ...display, steps };
}

/**
 * 1 件の GenerationEvent を積み上げて、新しい表示状態を返す。
 * `receivedAt` はイベントを受け取った時刻（エポックからのミリ秒）。純粋な関数に保つため、呼び出し側が渡す。
 */
export function applyGenerationEvent(
  display: GenerationDisplay,
  event: GenerationEvent,
  receivedAt: number,
): GenerationDisplay {
  switch (event.kind) {
    case "started":
      return { ...display, model: event.model };
    case "step_started": {
      const step: GenerationStepDisplay = {
        label: event.label,
        index: event.index,
        total: event.total,
        startedAt: receivedAt,
        content: "",
        reasoning: "",
        notices: [],
        finished: false,
        promptTokens: null,
        completionTokens: null,
        elapsedMs: null,
      };
      return { ...display, steps: [...display.steps, step] };
    }
    case "content":
      return updateLastStep(display, (step) => ({ ...step, content: step.content + event.text }));
    case "reasoning":
      return updateLastStep(display, (step) => ({
        ...step,
        reasoning: step.reasoning + event.text,
      }));
    case "notice":
      return updateLastStep(display, (step) => ({
        ...step,
        notices: [...step.notices, { level: event.level, message: event.message }],
      }));
    case "step_finished":
      return updateLastStep(display, (step) => ({
        ...step,
        finished: true,
        promptTokens: event.prompt_tokens,
        completionTokens: event.completion_tokens,
        elapsedMs: event.elapsed_ms,
      }));
  }
}
