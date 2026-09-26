import { useEditorStore } from "../store/editorStore";
import { useSettingsStore } from "../store/settingsStore";
import { useUiStore } from "../store/uiStore";
import { useWorkspaceStore } from "../store/workspaceStore";

/** テスト間で zustand のストア（モジュール単位のシングルトン）を初期状態に戻す。 */
export function resetAllStores(): void {
  useWorkspaceStore.getState().closeWorkspace();
  useEditorStore.getState().reset();
  useEditorStore.setState({ vertical: true, rubyPreview: false });
  useUiStore.setState({ toasts: [] });
  useSettingsStore.setState({ settings: null, status: "idle", errorMessage: null });
}
