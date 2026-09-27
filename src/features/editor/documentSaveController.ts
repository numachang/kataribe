import type { Backend } from "../../api/backend";
import { BackendError } from "../../api/backend";
import { createAutosaveScheduler } from "../../lib/autosave";
import { toErrorMessage } from "../../lib/errorMessage";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";

const AUTOSAVE_DELAY_MS = 1000;

export interface ConflictState {
  path: string;
  /** 保存しようとしていた内容。競合を解決する（上書き・再読み込み）ときに使う。 */
  content: string;
  /** 保存しようとしたときの前提ハッシュ。 */
  expectedHash: string | null;
}

interface SaveTarget {
  path: string;
  content: string;
  expectedHash: string | null;
}

type ConflictListener = (conflict: ConflictState | null) => void;

/**
 * 中央エディタで「今開いている文書」の保存を一元管理する。
 *
 * 画面には常に 1 つのエディタしかないため、モジュール単位のシングルトンとして持つ。
 * `useDocumentEditor`（表示用）と `useGenerationSession`（変更案を適用する前に保存を済ませる）の
 * 両方から、同じ保存処理・同じ競合状態を参照できるようにするため、React の外に置いている。
 *
 * 保存はどのタイミングで要求されても、保存先のパス・内容・前提ハッシュをその場で確定させてから直列に実行する。
 * 実行中に別の保存が要求されたら、今の保存が終わったあとにもう一度だけ実行する（最新の内容で）。
 * 保存の結果は、対象のパスが今も表示中の文書と一致するときだけエディタの状態に反映する。
 * こうすることで、保存の完了が別の文書に切り替わったあとに届いても、無関係な文書の状態を壊さない。
 */
class DocumentSaveController {
  private conflict: ConflictState | null = null;
  private readonly listeners = new Set<ConflictListener>();
  private latestBackend: Backend | null = null;
  private saving: Promise<void> | null = null;
  private saveAgainFor: Backend | null = null;

  private readonly scheduler = createAutosaveScheduler(AUTOSAVE_DELAY_MS, () => {
    if (this.latestBackend) {
      this.enqueueSave(this.latestBackend);
    }
  });

  /** 競合状態が変わるたびに呼ばれる（`useSyncExternalStore` から使う想定）。 */
  subscribe = (listener: ConflictListener): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  getConflict = (): ConflictState | null => this.conflict;

  /**
   * 保存できていない編集（保存の失敗・未解決の競合・まだ保存していない入力）が残っているか。
   * `flush` のあとに呼び、作品やウィンドウを閉じてよいかの判断に使う。
   */
  hasUnsavedWork = (): boolean =>
    this.conflict !== null || useEditorStore.getState().status !== "clean";

  /** 入力があったことを伝える。1 秒後に、そのとき開いている文書を保存する。 */
  notifyChange = (backend: Backend): void => {
    this.latestBackend = backend;
    this.scheduler.notifyChange();
  };

  /** 保留中の自動保存があれば取り消し、代わりに今すぐ保存する。 */
  saveNow = (backend: Backend): void => {
    this.latestBackend = backend;
    this.scheduler.cancel();
    this.enqueueSave(backend);
  };

  /**
   * 保留中の自動保存があれば今すぐ実行し、実行中の保存も終わるまで待つ。
   * ファイルを切り替える・作品を閉じる・ウィンドウを閉じる・変更案を適用する、それぞれの直前に呼ぶ。
   */
  flush = async (backend: Backend): Promise<void> => {
    this.latestBackend = backend;
    this.scheduler.flushIfPending();
    while (this.saving) {
      await this.saving;
    }
  };

  private enqueueSave(backend: Backend): void {
    if (this.saving) {
      this.saveAgainFor = backend;
      return;
    }
    this.saving = this.runSave(backend).finally(() => {
      this.saving = null;
      const again = this.saveAgainFor;
      this.saveAgainFor = null;
      if (again) {
        this.enqueueSave(again);
      }
    });
  }

  private async runSave(backend: Backend): Promise<void> {
    const { path, content, savedHash } = useEditorStore.getState();
    if (path === null) {
      return;
    }
    const target: SaveTarget = { path, content, expectedHash: savedHash };
    if (useEditorStore.getState().path === target.path) {
      useEditorStore.getState().markSaving();
    }
    try {
      const newHash = await backend.writeFile(target.path, target.content, target.expectedHash);
      this.applySaveSuccess(target, newHash);
    } catch (error) {
      if (error instanceof BackendError && error.kind === "conflict") {
        this.setConflict({
          path: target.path,
          content: target.content,
          expectedHash: target.expectedHash,
        });
        if (useEditorStore.getState().path === target.path) {
          useEditorStore.getState().markError("この文書は外部で変更されています。");
        }
        return;
      }
      const message = toErrorMessage(error, "保存できませんでした。");
      if (useEditorStore.getState().path === target.path) {
        useEditorStore.getState().markError(message);
      }
      useUiStore
        .getState()
        .showToast(`「${target.path}」を保存できませんでした: ${message}`, "error");
    }
  }

  private applySaveSuccess(target: SaveTarget, hash: string): void {
    const current = useEditorStore.getState();
    if (current.path !== target.path) {
      // 既に別の文書に切り替わっている。ディスクへの書き込み自体は済んでいるので、
      // 今表示している（無関係な）文書の状態には触れない。
      return;
    }
    if (current.content === target.content) {
      useEditorStore.getState().markSaved(hash);
    } else {
      // 保存中にさらに編集が進んだ。まだ dirty のまま、次の保存の基準ハッシュだけ更新する。
      useEditorStore.getState().recordSavedHash(hash);
    }
  }

  private setConflict(next: ConflictState | null): void {
    this.conflict = next;
    for (const listener of this.listeners) {
      listener(next);
    }
  }

  /** 競合ダイアログの「再読み込み」。失敗しても例外を投げず、トーストで知らせる。 */
  resolveConflictByReloading = async (backend: Backend): Promise<void> => {
    const current = this.conflict;
    if (!current) {
      return;
    }
    try {
      const file = await backend.readFile(current.path);
      this.setConflict(null);
      if (useEditorStore.getState().path === current.path) {
        useEditorStore.getState().loadDocument(current.path, file.content, file.hash);
      }
    } catch (error) {
      useUiStore.getState().showToast(toErrorMessage(error, "読み込み直せませんでした。"), "error");
    }
  };

  /**
   * 競合ダイアログの「上書きする」。外部でファイルが削除されていた場合は、新規作成として扱う
   * （expectedHash = null）。失敗しても例外を投げず、トーストで知らせて競合状態は残す。
   */
  resolveConflictByOverwriting = async (backend: Backend): Promise<void> => {
    const current = this.conflict;
    if (!current) {
      return;
    }
    try {
      const expectedHash = await this.currentDiskHash(backend, current.path);
      const newHash = await backend.writeFile(current.path, current.content, expectedHash);
      this.setConflict(null);
      if (useEditorStore.getState().path === current.path) {
        if (useEditorStore.getState().content === current.content) {
          useEditorStore.getState().markSaved(newHash);
        } else {
          useEditorStore.getState().recordSavedHash(newHash);
        }
      }
      useUiStore.getState().showToast("上書きして保存しました。");
    } catch (error) {
      useUiStore.getState().showToast(toErrorMessage(error, "上書きできませんでした。"), "error");
    }
  };

  private async currentDiskHash(backend: Backend, path: string): Promise<string | null> {
    try {
      const file = await backend.readFile(path);
      return file.hash;
    } catch (error) {
      if (error instanceof BackendError && error.kind === "not_found") {
        return null;
      }
      throw error;
    }
  }

  /** 競合ダイアログを、どちらも選ばずに閉じる。保存されていないことを知らせる。 */
  dismissConflict = (): void => {
    if (!this.conflict) {
      return;
    }
    this.setConflict(null);
    useUiStore
      .getState()
      .showToast("保存せずに閉じました。編集内容は保存されていません。", "error");
  };

  /** テスト間で状態を初期化する。 */
  reset = (): void => {
    this.scheduler.cancel();
    this.saving = null;
    this.saveAgainFor = null;
    this.latestBackend = null;
    this.setConflict(null);
  };
}

export const documentSaveController = new DocumentSaveController();
