import type { KeyboardEvent } from "react";
import { useState } from "react";
import { useGenerationSessionContext } from "../../../features/generation/GenerationSessionProvider";
import { findOverviewEntry } from "../../../lib/overviewTree";
import { taskForEntry } from "../../../lib/taskForEntry";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import "./DocumentTab.css";

/** 「この文書」タブ。今開いている文書に対して、生成するか、指示して書き直すかを選ぶ。 */
export function DocumentTab() {
  const overview = useWorkspaceStore((state) => state.overview);
  const currentPath = useWorkspaceStore((state) => state.currentPath);
  const session = useGenerationSessionContext();
  const [instruction, setInstruction] = useState("");
  const isBusy = session.phase !== "idle";

  if (currentPath === null) {
    return <p className="document-tab__empty">左の目次から文書を選んでください。</p>;
  }

  const entry = findOverviewEntry(overview, currentPath);
  if (!entry) {
    return null;
  }

  if (!entry.exists) {
    const task = taskForEntry(currentPath, entry.kind);
    return (
      <div className="document-tab">
        <p>この文書はまだ生成されていません。</p>
        {task && (
          <button
            type="button"
            className="app-button app-button--primary"
            disabled={isBusy}
            onClick={() => session.start(task)}
          >
            {entry.kind === "scene" ? "本文を生成" : "生成"}
          </button>
        )}
      </div>
    );
  }

  function handleReviseKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
    const isSubmitShortcut = (event.ctrlKey || event.metaKey) && event.key === "Enter";
    if (isSubmitShortcut && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submitRevision();
    }
  }

  function submitRevision(): void {
    const trimmed = instruction.trim();
    if (trimmed.length === 0 || isBusy || currentPath === null) {
      return;
    }
    session.start({ kind: "revise", path: currentPath, instruction: trimmed });
  }

  return (
    <div className="document-tab">
      <label className="app-field">
        <span>指示して書き直す</span>
        <textarea
          rows={4}
          value={instruction}
          onChange={(event) => setInstruction(event.target.value)}
          onKeyDown={handleReviseKeyDown}
          placeholder="例: もっと不穏な雰囲気にしてください"
        />
      </label>
      <button
        type="button"
        className="app-button app-button--primary"
        disabled={isBusy || instruction.trim().length === 0}
        onClick={submitRevision}
      >
        書き直す（Ctrl+Enter）
      </button>
    </div>
  );
}
