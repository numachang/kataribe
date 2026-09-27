import type { ReactNode } from "react";
import { useEffect } from "react";
import "./Dialog.css";

interface DialogProps {
  title: string;
  onClose: (() => void) | null;
  children: ReactNode;
  wide?: boolean;
}

/** 見た目をそろえた、ごく普通のモーダルダイアログ。Escape キーで閉じられるようにする。 */
export function Dialog({ title, onClose, children, wide = false }: DialogProps) {
  useEffect(() => {
    if (!onClose) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      // IME の変換中に確定させるための Escape まで拾って閉じてしまわないようにする。
      if (event.key === "Escape" && !event.isComposing) {
        onClose?.();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div className="dialog-overlay">
      <div
        className={`dialog-card${wide ? " dialog-card--wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="dialog-card__header">
          <h2>{title}</h2>
          {onClose && (
            <button
              type="button"
              className="dialog-card__close"
              onClick={onClose}
              aria-label="閉じる"
            >
              ×
            </button>
          )}
        </div>
        <div className="dialog-card__body">{children}</div>
      </div>
    </div>
  );
}
