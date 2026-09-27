import { useEffect, useState } from "react";
import type { GenerationStepDisplay } from "../../../features/generation/eventAccumulator";
import { useGenerationSessionContext } from "../../../features/generation/GenerationSessionProvider";
import { countGraphemes } from "../../../lib/graphemes";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import "./GenerationProgress.css";

const numberFormat = new Intl.NumberFormat("ja-JP");

/** 終わった回の結果（CLI の「完了（…）」と同じ形）。トークン数は届いたときだけ出す。 */
export function describeFinishedStep(step: GenerationStepDisplay): string {
  const seconds = ((step.elapsedMs ?? 0) / 1000).toFixed(1);
  if (step.promptTokens === null || step.completionTokens === null) {
    return `完了（${seconds} 秒）`;
  }
  return `完了（${seconds} 秒、入力 ${numberFormat.format(step.promptTokens)} トークン・出力 ${numberFormat.format(step.completionTokens)} トークン）`;
}

/** `since` からの経過秒数を、1 秒ごとに数え直して見せる。 */
export function ElapsedSeconds({ since }: { since: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  return <>経過 {Math.max(0, Math.floor((now - since) / 1000))} 秒</>;
}

interface StepStatusProps {
  step: GenerationStepDisplay;
  isRunning: boolean;
}

/**
 * 1 回分の進み具合。本文を流さない回（要約など）でも動いていることが分かるよう、経過時間を数える。
 * 終わった回は、かかった時間とトークン数を出す。
 */
function StepStatus({ step, isRunning }: StepStatusProps) {
  if (step.finished) {
    return <p className="generation-progress__step-status">{describeFinishedStep(step)}</p>;
  }
  if (!isRunning) {
    return <p className="generation-progress__step-status">途中で止まりました</p>;
  }
  const received = countGraphemes(step.content);
  const detail =
    received > 0
      ? `受け取った文字 ${numberFormat.format(received)} 字`
      : step.reasoning
        ? "考えています"
        : "応答を待っています";
  return (
    <p className="generation-progress__step-status">
      <ElapsedSeconds since={step.startedAt} />・{detail}
    </p>
  );
}

/**
 * 思考（reasoning）の断片が届いている間、本文（content）がまだ来ていなければ
 * 「考え中…」と分かるようにする。本文が来始めたか、段階が終われば通常の見出しに戻す。
 */
function reasoningSummary(step: GenerationStepDisplay): string {
  const isThinking = step.reasoning.length > 0 && step.content.length === 0 && !step.finished;
  return isThinking ? "考え中…（思考過程）" : "思考過程";
}

/** 生成中の進捗を表示する。段階ごとのラベル・本文のストリーミング・思考・注意書きを見せる。 */
export function GenerationProgress() {
  const session = useGenerationSessionContext();
  const pipeline = useWorkspaceStore((state) => state.pipeline);
  const doneCount = pipeline.filter((step) => step.state === "done").length;

  return (
    <div className="generation-progress">
      <div className="generation-progress__header">
        <span>{session.phase === "error" ? "生成に失敗しました" : "生成しています…"}</span>
        {session.phase === "running" &&
          (session.autoAdvancing ? (
            <button
              type="button"
              className="app-button app-button--danger"
              onClick={session.stopAutoAdvance}
            >
              自動で進めるのを止める
            </button>
          ) : (
            <button
              type="button"
              className="app-button app-button--danger"
              onClick={session.cancel}
            >
              中止
            </button>
          ))}
      </div>

      <div className="generation-progress__summary">
        {session.display.model && <span>使う LLM: {session.display.model}</span>}
        {pipeline.length > 0 && (
          <span>
            工程 {doneCount}/{pipeline.length} 済み
          </span>
        )}
      </div>

      {session.phase === "error" && session.errorMessage && (
        <p className="generation-progress__error">{session.errorMessage}</p>
      )}

      {session.display.steps.map((step, index) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: 段階には一意な id がないため
        <div key={index} className="generation-progress__step">
          <div className="generation-progress__step-label">
            {step.label}
            {step.total > 1 && (
              <span className="generation-progress__step-progress">
                （{step.index}/{step.total}）
              </span>
            )}
          </div>
          <StepStatus step={step} isRunning={session.phase === "running"} />
          {step.reasoning && (
            <details className="generation-progress__reasoning">
              <summary>{reasoningSummary(step)}</summary>
              <pre>{step.reasoning}</pre>
            </details>
          )}
          <pre className="generation-progress__content">{step.content}</pre>
          {step.notices.map((notice, noticeIndex) => (
            <p
              // biome-ignore lint/suspicious/noArrayIndexKey: 注意書きには一意な id がないため
              key={noticeIndex}
              className={`generation-progress__notice generation-progress__notice--${notice.level}`}
            >
              {notice.message}
            </p>
          ))}
        </div>
      ))}

      {session.phase === "error" && (
        <div className="generation-progress__actions">
          <button type="button" className="app-button" onClick={session.discard}>
            閉じる
          </button>
          <button
            type="button"
            className="app-button app-button--primary"
            onClick={session.regenerate}
          >
            もう一度試す
          </button>
        </div>
      )}
    </div>
  );
}
