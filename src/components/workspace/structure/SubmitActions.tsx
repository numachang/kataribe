import "./StructureDialog.css";

interface SubmitActionsProps {
  submitLabel: string;
  /** 実行中に出す文言。 */
  submittingLabel: string;
  isSubmitting: boolean;
  /** 送ってよい入力になっているか（必須の項目が埋まっているなど）。 */
  canSubmit: boolean;
  errorMessage: string | null;
  onSubmit: () => void;
  onCancel: () => void;
}

/**
 * 追加のダイアログの下部。失敗の理由と、「やめる」「追加」のボタン。
 * 送れるのはこのボタンだけにする（入力欄の Enter では送らない。日本語入力の確定の Enter で送ってしまうため）。
 */
export function SubmitActions({
  submitLabel,
  submittingLabel,
  isSubmitting,
  canSubmit,
  errorMessage,
  onSubmit,
  onCancel,
}: SubmitActionsProps) {
  return (
    <>
      {errorMessage !== null && (
        <p className="structure-dialog__error" role="alert">
          {errorMessage}
        </p>
      )}
      <div className="structure-dialog__actions">
        <button type="button" className="app-button" disabled={isSubmitting} onClick={onCancel}>
          やめる
        </button>
        <button
          type="button"
          className="app-button app-button--primary"
          disabled={!canSubmit || isSubmitting}
          onClick={onSubmit}
        >
          {isSubmitting ? submittingLabel : submitLabel}
        </button>
      </div>
    </>
  );
}
