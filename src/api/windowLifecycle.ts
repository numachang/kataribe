import { getCurrentWindow } from "@tauri-apps/api/window";
import { runningInsideTauri } from "./environment";

/**
 * ウィンドウを閉じる直前に、非同期の後始末（保存）を挟むための小さな抽象。
 * 本物のアプリ（Tauri）では「閉じる」を一度止め、後始末が終わってから閉じ直す。
 * ブラウザ単体（`pnpm dev`）では OS のタブ・ウィンドウを本当に止めることはできないため、
 * ベストエフォートで後始末を試みるだけにする。
 */
export interface WindowLifecycle {
  /**
   * ウィンドウを閉じようとしたときに `prepareToClose` を呼ぶようにする。
   * `prepareToClose` が true（閉じてよい）で解決したときだけ閉じる（止められる環境の場合）。
   * 返り値の関数で登録を解除できる。
   */
  interceptClose(prepareToClose: () => Promise<boolean>): () => void;
}

/** 本物のアプリ用。Tauri は、このハンドラが終わってから（止められていなければ）ウィンドウを閉じる。 */
class TauriWindowLifecycle implements WindowLifecycle {
  interceptClose(prepareToClose: () => Promise<boolean>): () => void {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void getCurrentWindow()
      .onCloseRequested(async (event) => {
        if (!(await prepareToClose())) {
          // 保存できなかった原稿があるので閉じない。理由は保存処理が画面に表示している
          event.preventDefault();
        }
      })
      .then((stopListening) => {
        if (disposed) {
          stopListening();
          return;
        }
        unlisten = stopListening;
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }
}

/** ブラウザ単体（`pnpm dev`、テスト）用。閉じるのを止めることはできないため、後始末を投げておくだけ。 */
class BrowserWindowLifecycle implements WindowLifecycle {
  interceptClose(prepareToClose: () => Promise<boolean>): () => void {
    function handleBeforeUnload(): void {
      void prepareToClose();
    }
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }
}

/** 実行環境に応じたウィンドウの後始末フックを作る。 */
export function createWindowLifecycle(): WindowLifecycle {
  return runningInsideTauri() ? new TauriWindowLifecycle() : new BrowserWindowLifecycle();
}
