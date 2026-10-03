import { useCallback, useEffect, useSyncExternalStore } from "react";
import { useBackend } from "../../api/context";
import type { EditableDocument } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { findOverviewEntry } from "../../lib/overviewTree";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { documentSaveController } from "./documentSaveController";

const UNSAVED_WORK_BLOCKS_SWITCH =
  "保存できていない編集があるため、文書を切り替えませんでした。保存してから切り替えてください。";

/**
 * 中央エディタで「今開いている文書」を保つためのロジック一式。
 * 目次でファイルを選ぶと読み込み、入力が止まって 1 秒後・Ctrl+S・ファイル切り替え・
 * 作品を閉じる・ウィンドウを閉じる前に保存する。
 *
 * 実際の保存の実行・直列化・競合の検出と解決は `documentSaveController` に任せている
 * （生成した変更案を適用する前にも同じ保存処理を使うため、React の外に置いてある）。
 */
export function useDocumentEditor() {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const currentPath = useWorkspaceStore((state) => state.currentPath);

  const path = useEditorStore((state) => state.path);
  const document = useEditorStore((state) => state.document);
  const parseError = useEditorStore((state) => state.parseError);
  const status = useEditorStore((state) => state.status);
  const errorMessage = useEditorStore((state) => state.errorMessage);
  const rubyPreview = useEditorStore((state) => state.rubyPreview);
  const vertical = useEditorStore((state) => state.vertical);

  const conflict = useSyncExternalStore(
    documentSaveController.subscribe,
    documentSaveController.getConflict,
    documentSaveController.getConflict,
  );

  // 開いている文書が変わるたびに読み込む。切り替わる直前には、前の文書の保存を済ませておく
  // （このクリーンアップは、ファイルの切り替えだけでなく、コンポーネントが真にアンマウントされる
  //  ときにも実行される。「保存してから取り消す」の順を守るため、保存側で使うタイマーの取り消しは
  //  documentSaveController の flush に一本化してあり、ここで個別に cancel は呼ばない）。
  useEffect(() => {
    let cancelled = false;
    // 前の文書の保存（予約された分を含む）が終わってから、エディタの中身を入れ替える。
    // 先に入れ替えると、予約された保存が新しい文書の内容で走り、前の文書の最後の入力が失われる。
    void documentSaveController
      .flush(backend)
      .then(() => {
        if (cancelled) {
          return;
        }
        const shownPath = useEditorStore.getState().path;
        if (shownPath !== null && shownPath === currentPath) {
          // 既に表示している（切り替えを取り消して戻ってきたときなど）。読み直すと未保存の編集を消す。
          return;
        }
        if (documentSaveController.hasUnsavedWork()) {
          // 前の文書に保存できていない編集がある。切り替えると編集が消えるので、切り替えを取り消す。
          showToast(UNSAVED_WORK_BLOCKS_SWITCH, "error");
          if (shownPath === null) {
            useWorkspaceStore.getState().clearCurrentDocument();
          } else {
            useWorkspaceStore.getState().openDocument(shownPath);
          }
          return;
        }
        if (currentPath === null) {
          useEditorStore.getState().reset();
          return;
        }
        const entry = findOverviewEntry(useWorkspaceStore.getState().overview, currentPath);
        if (entry && !entry.exists) {
          // まだ生成されていない文書。読み込もうとせず、EditorPane 側に専用の空の状態を表示させる。
          useEditorStore.getState().reset();
          return;
        }
        return backend.readDocument(currentPath).then((file) => {
          if (!cancelled) {
            useEditorStore.getState().loadDocument(currentPath, file);
          }
        });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          showToast(toErrorMessage(error, "文書を開けませんでした。"), "error");
          useWorkspaceStore.getState().clearCurrentDocument();
        }
      });
    return () => {
      cancelled = true;
      void documentSaveController.flush(backend);
    };
  }, [backend, currentPath, showToast]);

  const handleDocumentChange = useCallback(
    (next: EditableDocument) => {
      useEditorStore.getState().updateDocument(next);
      documentSaveController.notifyChange(backend);
    },
    [backend],
  );

  const saveNow = useCallback(() => {
    documentSaveController.saveNow(backend);
  }, [backend]);

  const resolveConflictByReloading = useCallback(
    () => documentSaveController.resolveConflictByReloading(backend),
    [backend],
  );

  const resolveConflictByOverwriting = useCallback(
    () => documentSaveController.resolveConflictByOverwriting(backend),
    [backend],
  );

  const dismissConflict = useCallback(() => documentSaveController.dismissConflict(), []);

  return {
    path,
    document,
    parseError,
    status,
    errorMessage,
    rubyPreview,
    vertical,
    conflict,
    onDocumentChange: handleDocumentChange,
    saveNow,
    toggleRubyPreview: () => useEditorStore.getState().toggleRubyPreview(),
    setVertical: (value: boolean) => useEditorStore.getState().setVertical(value),
    resolveConflictByReloading,
    resolveConflictByOverwriting,
    dismissConflict,
  };
}
