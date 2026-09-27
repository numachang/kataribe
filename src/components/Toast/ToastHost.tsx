import { useEffect } from "react";
import { useUiStore } from "../../store/uiStore";
import "./ToastHost.css";

const AUTO_DISMISS_MS = 6000;

/** 画面上部に積み上がるトースト通知。エラーや完了の案内を日本語のまま表示する。 */
export function ToastHost() {
  const toasts = useUiStore((state) => state.toasts);
  const dismissToast = useUiStore((state) => state.dismissToast);

  return (
    <div className="toast-host" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <ToastEntry
          key={toast.id}
          id={toast.id}
          kind={toast.kind}
          message={toast.message}
          onDismiss={dismissToast}
        />
      ))}
    </div>
  );
}

interface ToastEntryProps {
  id: string;
  kind: "info" | "error";
  message: string;
  onDismiss: (id: string) => void;
}

function ToastEntry({ id, kind, message, onDismiss }: ToastEntryProps) {
  useEffect(() => {
    const timer = setTimeout(() => onDismiss(id), AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [id, onDismiss]);

  return (
    <div className={`toast-host__item toast-host__item--${kind}`}>
      <span>{message}</span>
      <button
        type="button"
        className="toast-host__close"
        onClick={() => onDismiss(id)}
        aria-label="閉じる"
      >
        ×
      </button>
    </div>
  );
}
