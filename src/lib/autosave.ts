// 自動保存のデバウンス処理だけを切り出した、保存先に依存しない純粋なロジック。
// 「入力が止まって 1 秒後に保存」「Ctrl+S やファイル切り替えでは即座に保存」を
// タイマーの組み立てとして表現する。実際に保存する処理（backend.writeDocument 呼び出し）は
// 呼び出し側が `onDue` に渡す。

export interface AutosaveScheduler {
  /** 入力があったことを伝える。既存のタイマーを仕切り直して待つ。 */
  notifyChange(): void;
  /** 保留中のタイマーがあれば、待たずに今すぐ実行する。 */
  flushIfPending(): void;
  /** 保留中のタイマーを取り消す（保存せず破棄したいとき）。 */
  cancel(): void;
  /** タイマーが保留中かどうか。 */
  readonly isPending: boolean;
}

/**
 * `delayMs` 後に `onDue` を呼ぶスケジューラを作る。
 * `notifyChange` を呼ぶたびに待ち時間は最初から数え直される。
 */
export function createAutosaveScheduler(delayMs: number, onDue: () => void): AutosaveScheduler {
  let timeoutHandle: ReturnType<typeof setTimeout> | null = null;

  function cancel(): void {
    if (timeoutHandle !== null) {
      clearTimeout(timeoutHandle);
      timeoutHandle = null;
    }
  }

  return {
    notifyChange(): void {
      cancel();
      timeoutHandle = setTimeout(() => {
        timeoutHandle = null;
        onDue();
      }, delayMs);
    },
    flushIfPending(): void {
      if (timeoutHandle === null) {
        return;
      }
      cancel();
      onDue();
    },
    cancel,
    get isPending(): boolean {
      return timeoutHandle !== null;
    },
  };
}
