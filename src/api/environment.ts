// 実行環境の判定だけを切り出したもの。Backend の生成（index.ts）と
// ウィンドウの後始末（windowLifecycle.ts）の両方が同じ判定を必要とするため。

/** Tauri のネイティブアプリの中で動いているか（false ならブラウザ単体、`pnpm dev` など）。 */
export function runningInsideTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
