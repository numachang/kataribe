import { useCallback } from "react";
import { useBackend } from "../../api/context";
import type { Task } from "../../api/types";
import { documentSaveController } from "../editor/documentSaveController";
import { useGenerationSessionContext } from "../generation/GenerationSessionProvider";
import { useStructureEditBlockedReason } from "./useStructureAvailability";
import { useSubmission } from "./useSubmission";

export interface GeneratedAdditionApi {
  /** 生成を始められない理由（生成の途中・構成の操作の途中など）。始められるなら null。 */
  blockedReason: string | null;
  /** 開いている文書を保存し終えて、生成を始めるまでの間。 */
  isStarting: boolean;
  errorMessage: string | null;
  start: (task: Task) => Promise<void>;
}

/**
 * 指示から人物・資料などを作って足す生成を、追加のダイアログから始める手順。
 * 始められる状態か確かめ、開いている文書の保存を済ませ（生成の途中で保存された編集と食い違わないように）、
 * 生成のセッションを始めたらダイアログを閉じる（`onStarted`）。進み具合と変更案の確認は AI パネルが受け持つ。
 * 始められない理由があるときは何もしない（押せない理由はダイアログに出す）。
 */
export function useGeneratedAddition(onStarted: () => void): GeneratedAdditionApi {
  const backend = useBackend();
  const { start: startGeneration } = useGenerationSessionContext();
  const blockedReason = useStructureEditBlockedReason("AI に作らせて追加");
  const { run, isSubmitting, errorMessage } = useSubmission();

  const start = useCallback(
    async (task: Task): Promise<void> => {
      if (blockedReason !== null) {
        return;
      }
      const started = await run(async () => {
        await documentSaveController.flush(backend);
        startGeneration(task);
      });
      if (started) {
        onStarted();
      }
    },
    [backend, blockedReason, onStarted, run, startGeneration],
  );

  return { blockedReason, isStarting: isSubmitting, errorMessage, start };
}
