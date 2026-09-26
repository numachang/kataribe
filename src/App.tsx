import { useEffect } from "react";
import { useBackend } from "./api/context";
import { StartScreen } from "./components/start/StartScreen";
import { ToastHost } from "./components/Toast/ToastHost";
import { WorkspaceScreen } from "./components/workspace/WorkspaceScreen";
import { useSettingsStore } from "./store/settingsStore";
import { useWorkspaceStore } from "./store/workspaceStore";

/** アプリの入り口。作品が開かれているかどうかで、開始画面と作業画面を切り替える。 */
export function App() {
  const backend = useBackend();
  const overview = useWorkspaceStore((state) => state.overview);

  useEffect(() => {
    void useSettingsStore.getState().load(backend);
  }, [backend]);

  return (
    <>
      <ToastHost />
      {overview ? <WorkspaceScreen /> : <StartScreen />}
    </>
  );
}
