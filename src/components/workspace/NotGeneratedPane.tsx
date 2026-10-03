import { useState } from "react";
import { useBackend } from "../../api/context";
import type { OverviewEntry } from "../../api/types";
import { startEmptyDocument } from "../../features/editor/startEmptyDocument";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";

interface NotGeneratedPaneProps {
  entry: OverviewEntry;
}

/**
 * まだ生成していない文書を選んだときの、エディタの代わりの表示。
 * 本文のシーンなら、生成を待たずに、空の本文から自分で書き始めるボタンを添える。
 */
export function NotGeneratedPane({ entry }: NotGeneratedPaneProps) {
  const backend = useBackend();
  const [isStarting, setStarting] = useState(false);
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
          disabled={isStarting}
          onClick={() => void startWriting(path)}
        >
          {isStarting ? "作っています…" : "空の本文から書き始める"}
        </button>
      )}
    </div>
  );
}
