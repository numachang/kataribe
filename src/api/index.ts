import type { Backend } from "./backend";
import { runningInsideTauri } from "./environment";
import { createMockBackend } from "./mock";
import { TauriBackend } from "./tauri";

/**
 * 実行環境に応じたアプリ本体を作る。
 * Tauri の中で動いていれば本物のコマンドを呼び、ブラウザ単体（`pnpm dev`）ならメモリ上の偽実装を使う。
 */
export function createBackend(): Backend {
  return runningInsideTauri() ? new TauriBackend() : createMockBackend();
}
