import { useState } from "react";
import { useBackend } from "../../api/context";
import type { OverviewEntry } from "../../api/types";
import { startEmptyDocument } from "../../features/editor/startEmptyDocument";
import { useStructureEditBlockedReason } from "../../features/structure/useStructureAvailability";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";

interface NotGeneratedPaneProps {
  entry: OverviewEntry;
}

/**
 * まだ生成していない文書を選んだときの、エディタの代わりの表示。
 * 本文のシーンなら、生成を待たずに、空の本文から自分で書き始めるボタンを添える。
 * 生成の途中や変更案の確認中は押せない（新規に書く本文の書き先が、生成した変更案とずれて適用できなくなるため）。
 * 目次の追加・削除・並べ替えを適用している間も押せない（章の番号の振り直しの前のパスに、本文を作ってしまうため）。
 */
export function NotGeneratedPane({ entry }: NotGeneratedPaneProps) {
  const backend = useBackend();
  const [isStarting, setStarting] = useState(false);
  const blockedReason = useStructureEditBlockedReason("本文を作成");
  const path = entry.path;
  const canStartWriting = entry.kind === "scene" && path !== null;

  async function startWriting(targetPath: string): Promise<void> {
    setStarting(true);
    try {
      await startEmptyDocument(backend, targetPath);
    } catch (error) {
      useUiStore.getState().showToast(toErrorMessage(error, "本文を作れませんでした。"), "error");
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="editor-pane editor-pane--empty">
      <p>この文書はまだ生成されていません。</p>
      {canStartWriting && (
        <button
          type="button"
          className="app-button"
          disabled={isStarting || blockedReason !== null}
          title={blockedReason ?? undefined}
          onClick={() => void startWriting(path)}
        >
          {isStarting ? "作っています…" : "空の本文から書き始める"}
        </button>
      )}
    </div>
  );
}
