import type { Backend } from "../../api/backend";
import { useEditorStore } from "../../store/editorStore";
import { useWorkspaceStore } from "../../store/workspaceStore";

/**
 * まだ無い文書（生成していない本文など）を、空の内容で作り、自分で書き始められるようにする。
 * 作ったあと、その文書を開いたままなら読み込み、目次と工程の「ある・ない」を直す。
 */
export async function startEmptyDocument(backend: Backend, path: string): Promise<void> {
  await backend.writeDocument(path, { kind: "text", content: "" }, null);
  const file = await backend.readDocument(path);
  // 作っている間に別の文書へ移っていたら、その文書のエディタを置き換えない
  if (useWorkspaceStore.getState().currentPath === path) {
    useEditorStore.getState().loadDocument(path, file);
  }
  const workspace = useWorkspaceStore.getState();
  await Promise.all([workspace.refreshOverview(backend), workspace.refreshPipeline(backend)]);
}
