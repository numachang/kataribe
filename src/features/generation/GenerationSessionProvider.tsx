import type { ReactNode } from "react";
import { createContext, useContext } from "react";
import { type GenerationSessionApi, useGenerationSession } from "./useGenerationSession";

const GenerationSessionContext = createContext<GenerationSessionApi | null>(null);

/**
 * 「工程」タブと「この文書」タブから同じ生成セッションを見られるようにする。
 * 作業画面のルートで 1 度だけマウントする。
 */
export function GenerationSessionProvider({ children }: { children: ReactNode }) {
  const session = useGenerationSession();
  return (
    <GenerationSessionContext.Provider value={session}>
      {children}
    </GenerationSessionContext.Provider>
  );
}

export function useGenerationSessionContext(): GenerationSessionApi {
  const session = useContext(GenerationSessionContext);
  if (!session) {
    throw new Error(
      "useGenerationSessionContext は GenerationSessionProvider の内側で呼び出してください。",
    );
  }
  return session;
}
