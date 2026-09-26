import { useState } from "react";
import { useBackend } from "../../api/context";
import type { ProjectOverview } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { useSettingsStore } from "../../store/settingsStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { NewProjectDialog } from "./NewProjectDialog";
import "./StartScreen.css";

// useSyncExternalStore は getSnapshot が毎回新しい参照を返すと無限ループになるため、
// 「まだ設定を読み込めていない」ときの既定値はモジュール直下で 1 つだけ作って使い回す。
const NO_RECENT_PROJECTS: string[] = [];

function folderDisplayName(folder: string): string {
  const parts = folder.split(/[\\/]/).filter((part) => part.length > 0);
  return parts.at(-1) ?? folder;
}

/** 起動時の画面。最近の作品を開く、新しい作品を作る、作品フォルダを開く、のいずれかを選ぶ。 */
export function StartScreen() {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const recentProjects = useSettingsStore(
    (state) => state.settings?.recent_projects ?? NO_RECENT_PROJECTS,
  );

  const [isNewProjectOpen, setNewProjectOpen] = useState(false);
  const [isOpening, setIsOpening] = useState(false);

  function enterWorkspace(overview: ProjectOverview): void {
    useWorkspaceStore.getState().openWorkspace(overview);
    void useWorkspaceStore.getState().refreshPipeline(backend);
  }

  async function openFolder(folder: string): Promise<void> {
    setIsOpening(true);
    try {
      const overview = await backend.openProject(folder);
      enterWorkspace(overview);
    } catch (error) {
      showToast(toErrorMessage(error, "作品を開けませんでした。"), "error");
    } finally {
      setIsOpening(false);
    }
  }

  async function handleOpenFolderClick(): Promise<void> {
    const folder = await backend.pickFolder();
    if (folder) {
      await openFolder(folder);
    }
  }

  return (
    <div className="start-screen">
      <div className="start-screen__card">
        <h1 className="start-screen__title">kataribe</h1>
        <p className="start-screen__subtitle">LLM と一緒に、日本語の小説を書く。</p>

        <div className="start-screen__actions">
          <button
            type="button"
            className="app-button app-button--primary"
            onClick={() => setNewProjectOpen(true)}
          >
            新しい作品
          </button>
          <button
            type="button"
            className="app-button"
            disabled={isOpening}
            onClick={() => void handleOpenFolderClick()}
          >
            作品フォルダを開く
          </button>
        </div>

        <section className="start-screen__recent">
          <h2>最近の作品</h2>
          {recentProjects.length === 0 ? (
            <p className="start-screen__empty">まだ開いた作品がありません。</p>
          ) : (
            <ul className="start-screen__list">
              {recentProjects.map((folder) => (
                <li key={folder}>
                  <button
                    type="button"
                    className="start-screen__recent-item"
                    disabled={isOpening}
                    onClick={() => void openFolder(folder)}
                  >
                    <span className="start-screen__recent-name">{folderDisplayName(folder)}</span>
                    <span className="start-screen__recent-path">{folder}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      {isNewProjectOpen && (
        <NewProjectDialog
          onClose={() => setNewProjectOpen(false)}
          onCreated={(overview) => {
            setNewProjectOpen(false);
            enterWorkspace(overview);
          }}
        />
      )}
    </div>
  );
}
