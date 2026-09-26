import { useCallback, useEffect, useRef, useState } from "react";
import { BackendError } from "../../api/backend";
import { useBackend } from "../../api/context";
import { createAutosaveScheduler } from "../../lib/autosave";
import { toErrorMessage } from "../../lib/errorMessage";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";

const AUTOSAVE_DELAY_MS = 1000;

export interface ConflictState {
  path: string;
}

/**
 * 中央エディタで「今開いている文書」を保つためのロジック一式。
 * 目次でファイルを選ぶと読み込み、入力が止まって 1 秒後・Ctrl+S・ファイル切り替え・
 * ウィンドウを閉じる前に保存する。競合したときは呼び出し側へ知らせる。
 */
export function useDocumentEditor() {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const currentPath = useWorkspaceStore((state) => state.currentPath);

  const path = useEditorStore((state) => state.path);
  const content = useEditorStore((state) => state.content);
  const status = useEditorStore((state) => state.status);
  const errorMessage = useEditorStore((state) => state.errorMessage);
  const rubyPreview = useEditorStore((state) => state.rubyPreview);
  const vertical = useEditorStore((state) => state.vertical);

  const [conflict, setConflict] = useState<ConflictState | null>(null);

  // スケジューラのコールバックは古いクロージャで実行されがちなので、常に最新の値を ref で参照する。
  const latestRef = useRef({ path, content, savedHash: useEditorStore.getState().savedHash });
  useEffect(() => {
    latestRef.current = { path, content, savedHash: useEditorStore.getState().savedHash };
  });

  const save = useCallback(async (): Promise<void> => {
    const { path: targetPath, content: targetContent, savedHash } = latestRef.current;
    if (targetPath === null) {
      return;
    }
    useEditorStore.getState().markSaving();
    try {
      const newHash = await backend.writeFile(targetPath, targetContent, savedHash);
      useEditorStore.getState().markSaved(newHash);
    } catch (error) {
      if (error instanceof BackendError && error.kind === "conflict") {
        useEditorStore.getState().markError(error.message);
        setConflict({ path: targetPath });
        return;
      }
      const message = toErrorMessage(error, "保存できませんでした。");
      useEditorStore.getState().markError(message);
      showToast(message, "error");
    }
  }, [backend, showToast]);

  const schedulerRef = useRef(createAutosaveScheduler(AUTOSAVE_DELAY_MS, () => void save()));
  useEffect(() => {
    schedulerRef.current = createAutosaveScheduler(AUTOSAVE_DELAY_MS, () => void save());
    return () => schedulerRef.current.cancel();
  }, [save]);

  // 開いている文書が変わるたびに読み込む。切り替わる直前には、前の文書の保存を済ませておく。
  useEffect(() => {
    if (currentPath === null) {
      useEditorStore.getState().reset();
      return;
    }
    let cancelled = false;
    backend
      .readFile(currentPath)
      .then((file) => {
        if (!cancelled) {
          useEditorStore.getState().loadDocument(currentPath, file.content, file.hash);
          setConflict(null);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          showToast(toErrorMessage(error, "文書を開けませんでした。"), "error");
        }
      });
    return () => {
      cancelled = true;
      schedulerRef.current.flushIfPending();
    };
  }, [backend, currentPath, showToast]);

  // ウィンドウを閉じる前にも保存しておく。
  useEffect(() => {
    function handleBeforeUnload(): void {
      schedulerRef.current.flushIfPending();
    }
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, []);

  const handleContentChange = useCallback((next: string) => {
    useEditorStore.getState().updateContent(next);
    schedulerRef.current.notifyChange();
  }, []);

  const saveNow = useCallback(() => {
    schedulerRef.current.cancel();
    void save();
  }, [save]);

  const resolveConflictByReloading = useCallback(async () => {
    if (!conflict) {
      return;
    }
    const file = await backend.readFile(conflict.path);
    useEditorStore.getState().loadDocument(conflict.path, file.content, file.hash);
    setConflict(null);
  }, [backend, conflict]);

  const resolveConflictByOverwriting = useCallback(async () => {
    if (!conflict) {
      return;
    }
    const file = await backend.readFile(conflict.path);
    const newHash = await backend.writeFile(conflict.path, latestRef.current.content, file.hash);
    useEditorStore.getState().markSaved(newHash);
    setConflict(null);
  }, [backend, conflict]);

  return {
    path,
    content,
    status,
    errorMessage,
    rubyPreview,
    vertical,
    conflict,
    onContentChange: handleContentChange,
    saveNow,
    toggleRubyPreview: () => useEditorStore.getState().toggleRubyPreview(),
    setVertical: (value: boolean) => useEditorStore.getState().setVertical(value),
    resolveConflictByReloading,
    resolveConflictByOverwriting,
  };
}
