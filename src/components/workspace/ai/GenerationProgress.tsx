import type { GenerationStepDisplay } from "../../../features/generation/eventAccumulator";
import { useGenerationSessionContext } from "../../../features/generation/GenerationSessionProvider";
import "./GenerationProgress.css";

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
