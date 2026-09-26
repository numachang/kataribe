import { create } from "zustand";
import type { Backend } from "../api/backend";
import type { AppSettings } from "../api/types";

type LoadStatus = "idle" | "loading" | "ready" | "error";

interface SettingsState {
  settings: AppSettings | null;
  status: LoadStatus;
  errorMessage: string | null;
  load: (backend: Backend) => Promise<void>;
  save: (backend: Backend, next: AppSettings) => Promise<void>;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "設定の読み込みに失敗しました。";
}

/** アプリ設定（AppSettings）のキャッシュ。読み込み・保存のたびに Backend を呼ぶ。 */
export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  status: "idle",
  errorMessage: null,

  async load(backend) {
    set({ status: "loading", errorMessage: null });
    try {
      const settings = await backend.loadSettings();
      set({ settings, status: "ready" });
    } catch (error) {
      set({ status: "error", errorMessage: toErrorMessage(error) });
    }
  },

  async save(backend, next) {
    await backend.saveSettings(next);
    set({ settings: next, status: "ready" });
  },
}));
