import { create } from "zustand";

export type ToastKind = "info" | "error";

export interface ToastItem {
  id: string;
  kind: ToastKind;
  message: string;
}

interface UiState {
  toasts: ToastItem[];
  showToast: (message: string, kind?: ToastKind) => string;
  dismissToast: (id: string) => void;
}

let nextToastId = 0;

function createToastId(): string {
  nextToastId += 1;
  return `toast-${nextToastId}`;
}

/** 画面上部のトースト通知。どこからでも `showToast` でエラーや案内を出せる。 */
export const useUiStore = create<UiState>((set) => ({
  toasts: [],
  showToast: (message, kind = "info") => {
    const id = createToastId();
    set((state) => ({ toasts: [...state.toasts, { id, kind, message }] }));
    return id;
  },
  dismissToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) }));
  },
}));
