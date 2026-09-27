import { useEffect, useState } from "react";
import { useBackend } from "../../api/context";
import { createWindowLifecycle } from "../../api/windowLifecycle";
import { documentSaveController } from "../../features/editor/documentSaveController";
import { GenerationSessionProvider } from "../../features/generation/GenerationSessionProvider";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";
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
  const showToast = useUiStore((state) => state.showToast);
  const overview = useWorkspaceStore((state) => state.overview);
  const [isSettingsOpen, setSettingsOpen] = useState(false);

  // ウィンドウを閉じる前にも、開いている文書の保存を済ませる。保存できなければ閉じない。
  useEffect(() => {
    const lifecycle = createWindowLifecycle();
    return lifecycle.interceptClose(async () => {
      await documentSaveController.flush(backend);
      return !documentSaveController.hasUnsavedWork();
    });
  }, [backend]);

  if (!overview) {
    return null;
  }

  async function handleCloseProject(): Promise<void> {
    // 保存の完了を待ってから作品を閉じる。保存できなかった編集があれば閉じない（原稿を失わないため）。
    await documentSaveController.flush(backend);
    if (documentSaveController.hasUnsavedWork()) {
      showToast("保存できていない編集があるため、作品を閉じませんでした。", "error");
      return;
    }
    try {
      await backend.closeProject();
    } catch (error) {
      showToast(toErrorMessage(error, "作品を閉じられませんでした。"), "error");
    }
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
            <button type="button" className="app-button" onClick={() => void handleCloseProject()}>
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
