import { useCallback, useState } from "react";
import { BackendError } from "../../api/backend";
import { toErrorMessage } from "../../lib/errorMessage";

/**
 * ダイアログの「追加」「削除」ボタンが行う、失敗しうる 1 つの操作の状態。
 * 実行中は二重に押せないようにし、失敗したら理由をダイアログの中に出せるよう持っておく。
 */
export function useSubmission() {
  const [isSubmitting, setSubmitting] = useState(false);
  // 投げられた値そのもの（null や undefined が投げられても、失敗があったことは分かるよう包んで持つ）
  const [failure, setFailure] = useState<{ error: unknown } | null>(null);

  /** `action` を実行する。成功したかを返す（成功すれば、呼び出し側がダイアログを閉じる）。 */
  const run = useCallback(async (action: () => Promise<void>): Promise<boolean> => {
    setSubmitting(true);
    setFailure(null);
    try {
      await action();
      return true;
    } catch (error) {
      setFailure({ error });
      return false;
    } finally {
      setSubmitting(false);
    }
  }, []);

  const clearFailure = useCallback(() => setFailure(null), []);

  return {
    isSubmitting,
    errorMessage: failure === null ? null : toErrorMessage(failure.error, "操作に失敗しました。"),
    /** 作品が、確かめたあとに外で変わっていたための失敗か。 */
    isConflict: failure?.error instanceof BackendError && failure.error.kind === "conflict",
    run,
    clearFailure,
  };
}
