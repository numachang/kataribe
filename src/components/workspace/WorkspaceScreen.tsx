import { useState } from "react";
import { useBackend } from "../../api/context";
import { GenerationSessionProvider } from "../../features/generation/GenerationSessionProvider";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { ResizableSplit } from "../ResizableSplit";
import { SettingsDialog } from "../settings/SettingsDialog";
import { AiPanel } from "./ai/AiPanel";
import { EditorPane } from "./EditorPane";
import { ProjectTree } from "./ProjectTree";
import "./WorkspaceScreen.css";

/** 作業画面。左：目次、中央：エディタ、右：AI パネルの 3 ペイン。 */
export function WorkspaceScreen() {
  const backend = useBackend();
  const overview = useWorkspaceStore((state) => state.overview);
  const [isSettingsOpen, setSettingsOpen] = useState(false);

  if (!overview) {
    return null;
  }

  function handleCloseProject(): void {
    void backend.closeProject();
    useWorkspaceStore.getState().closeWorkspace();
  }

  return (
    <GenerationSessionProvider>
      <div className="workspace-screen">
        <header className="workspace-screen__topbar">
          <span className="workspace-screen__title">{overview.title}</span>
          <div className="workspace-screen__topbar-actions">
            <button type="button" className="app-button" onClick={() => setSettingsOpen(true)}>
              設定
            </button>
            <button type="button" className="app-button" onClick={handleCloseProject}>
              作品を閉じる
            </button>
          </div>
        </header>

        <div className="workspace-screen__body">
          <ResizableSplit
            storageKey="kataribe:workspace-panels"
            left={<ProjectTree />}
            center={<EditorPane />}
            right={<AiPanel />}
          />
        </div>

        {isSettingsOpen && <SettingsDialog onClose={() => setSettingsOpen(false)} />}
      </div>
    </GenerationSessionProvider>
  );
}
