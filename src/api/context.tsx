import type { ReactNode } from "react";
import { createContext, useContext } from "react";
import type { Backend } from "./backend";

const BackendContext = createContext<Backend | null>(null);

interface BackendProviderProps {
  backend: Backend;
  children: ReactNode;
}

/** 画面のどこからでも `useBackend` でアプリ本体を呼べるようにする。 */
export function BackendProvider({ backend, children }: BackendProviderProps) {
  return <BackendContext.Provider value={backend}>{children}</BackendContext.Provider>;
}

/** 現在の Backend（本番は Tauri、開発とテストでは偽実装）を取得する。 */
export function useBackend(): Backend {
  const backend = useContext(BackendContext);
  if (!backend) {
    throw new Error("useBackend は BackendProvider の内側で呼び出してください。");
  }
  return backend;
}
