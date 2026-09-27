import { useEffect } from "react";
import { useBackend } from "./api/context";
import { StartScreen } from "./components/start/StartScreen";
import { ToastHost } from "./components/Toast/ToastHost";
import { WorkspaceScreen } from "./components/workspace/WorkspaceScreen";
import { useEditorStore } from "./store/editorStore";
import { useSettingsStore } from "./store/settingsStore";
import { useWorkspaceStore } from "./store/workspaceStore";

/** アプリの入り口。作品が開かれているかどうかで、開始画面と作業画面を切り替える。 */
export function App() {
  const backend = useBackend();
  const overview = useWorkspaceStore((state) => state.overview);

  useEffect(() => {
    void useSettingsStore
      .getState()
      .load(backend)
      .then(() => {
        // エディタの初期の縦横は設定に従う。以降の切り替えは利用者の操作を優先し、
        // 設定の再読み込みのたびに上書きしない。
        const settings = useSettingsStore.getState().settings;
        if (settings) {
          useEditorStore.getState().setVertical(settings.editor.vertical);
        }
      });
  }, [backend]);

  return (
    <>
      <ToastHost />
      {overview ? <WorkspaceScreen /> : <StartScreen />}
    </>
  );
}
