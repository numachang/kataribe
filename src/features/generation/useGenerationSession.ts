import { useCallback, useEffect, useRef, useState } from "react";
import { BackendError } from "../../api/backend";
import { useBackend } from "../../api/context";
import type { ChangeSet, Task } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { writeBesideEditor } from "../editor/openDocumentSync";
import {
  applyGenerationEvent,
  createEmptyGenerationDisplay,
  type GenerationDisplay,
} from "./eventAccumulator";

export type GenerationPhase = "idle" | "running" | "reviewing" | "error";

export interface GenerationSessionApi {
  phase: GenerationPhase;
  currentTask: Task | null;
  display: GenerationDisplay;
  changeSet: ChangeSet | null;
  errorMessage: string | null;
  /** 変更案の適用中かどうか。適用中は「適用」などのボタンを無効にする。 */
  isApplying: boolean;
  /** 適用に失敗したときのメッセージ。phase は reviewing のまま、変更案の下に表示する。 */
  applyErrorMessage: string | null;
  autoAdvancing: boolean;
  start: (task: Task) => void;
  cancel: () => void;
  apply: () => Promise<void>;
  discard: () => void;
  regenerate: () => void;
  runAutoAdvance: () => void;
  stopAutoAdvance: () => void;
}

/**
 * ジョブ ID を採番する。画面をリロードするとモジュール変数は 0 に戻ってしまうため、
 * 連番ではなく `crypto.randomUUID()` で一意な ID にする
 * （連番だと、リロード前に Rust 側へ残っていたジョブと ID が衝突しうる）。
 */
function createJobId(): string {
  return crypto.randomUUID();
}

const UNSAVED_WORK_BLOCKS_APPLY =
  "開いている文書に保存できていない編集があるため、適用しませんでした。保存してから、もう一度適用してください。";

function touchesDocument(changeSet: ChangeSet, path: string): boolean {
  return changeSet.files.some((file) => file.path === path);
}

/**
 * 「工程」タブと「この文書」タブが共有する、生成 1 回分のセッション。
 * 一度に実行できる生成は 1 件だけで、どちらのタブから始めても同じ進捗・結果を表示する。
 *
 * このフックは作品を開いている間だけ（`GenerationSessionProvider` の寿命だけ）マウントされる。
 * 作品を閉じてアンマウントされたら、自動で進めるループを止め、実行中のジョブを中止し、
 * それ以降は変更案の適用など画面の状態を変える処理を一切行わない。
 */
export function useGenerationSession(): GenerationSessionApi {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);

  const [phase, setPhase] = useState<GenerationPhase>("idle");
  const [currentTask, setCurrentTask] = useState<Task | null>(null);
  const [display, setDisplay] = useState<GenerationDisplay>(createEmptyGenerationDisplay());
  const [changeSet, setChangeSet] = useState<ChangeSet | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [applyErrorMessage, setApplyErrorMessage] = useState<string | null>(null);
  const [isApplying, setIsApplying] = useState(false);
  const [autoAdvancing, setAutoAdvancing] = useState(false);

  const activeJobId = useRef<string | null>(null);
  const autoAdvanceStopRequested = useRef(false);
  const unmountedRef = useRef(false);

  // 作品を閉じる（このフックがアンマウントされる）ときに、自動で進めるループを止め、
  // 実行中の生成ジョブを中止する。アンマウント後は変更案の適用など画面の状態を変える処理を行わない。
  useEffect(() => {
    // StrictMode では後始末のあとにもう一度実行されるので、ここで戻しておく
    unmountedRef.current = false;
    return () => {
      unmountedRef.current = true;
      autoAdvanceStopRequested.current = true;
      const jobId = activeJobId.current;
      if (jobId) {
        // アンマウント後はエラーを表示する場所がないので、失敗しても静かに諦める。
        backend.cancelGeneration(jobId).catch(() => {});
      }
    };
  }, [backend]);

  const runTask = useCallback(
    async (task: Task): Promise<ChangeSet | null> => {
      const jobId = createJobId();
      activeJobId.current = jobId;
      setPhase("running");
      setCurrentTask(task);
      setDisplay(createEmptyGenerationDisplay());
      setChangeSet(null);
      setErrorMessage(null);
      setApplyErrorMessage(null);

      try {
        const result = await backend.generate(jobId, task, (event) => {
          if (unmountedRef.current) {
            return;
          }
          const receivedAt = Date.now();
          setDisplay((previous) => applyGenerationEvent(previous, event, receivedAt));
        });
        if (unmountedRef.current) {
          return null;
        }
        setChangeSet(result);
        setPhase("reviewing");
        return result;
      } catch (error) {
        if (unmountedRef.current) {
          return null;
        }
        if (error instanceof BackendError && error.kind === "cancelled") {
          setPhase("idle");
          return null;
        }
        const message = toErrorMessage(error, "生成に失敗しました。");
        setErrorMessage(message);
        setPhase("error");
        showToast(message, "error");
        return null;
      } finally {
        activeJobId.current = null;
      }
    },
    [backend, showToast],
  );

  /**
   * 変更案を適用する。開いている文書との食い違いを防ぐ手順（先に保存を済ませ、適用後に読み直す）は
   * `writeBesideEditor` に任せる。
   * 成功したかどうかを返す。呼び出し側（自動で進めるループ）はこれで止まるべきかを判断する。
   */
  const applyResult = useCallback(
    async (result: ChangeSet): Promise<boolean> => {
      if (unmountedRef.current) {
        return false;
      }
      setIsApplying(true);
      setApplyErrorMessage(null);
      try {
        const applied = await writeBesideEditor(backend, {
          touches: (path) => touchesDocument(result, path),
          unsavedWorkMessage: UNSAVED_WORK_BLOCKS_APPLY,
          write: async () => {
            // 保存を待つ間に画面が閉じられた（作品を閉じたなど）なら、適用しない
            if (unmountedRef.current) {
              return false;
            }
            const overview = await backend.applyChangeSet(result);
            if (!unmountedRef.current) {
              useWorkspaceStore.getState().setOverview(overview);
              await useWorkspaceStore.getState().refreshPipeline(backend);
            }
            return true;
          },
        });
        if (!applied || unmountedRef.current) {
          return applied;
        }

        setPhase("idle");
        setCurrentTask(null);
        setChangeSet(null);
        setDisplay(createEmptyGenerationDisplay());
        showToast(result.summary);
        return true;
      } catch (error) {
        if (unmountedRef.current) {
          return false;
        }
        // 適用の失敗は reviewing のまま変更案を残し、下にエラーを表示する（生成の失敗とは区別する）。
        const message = toErrorMessage(error, "適用に失敗しました。");
        setApplyErrorMessage(message);
        showToast(message, "error");
        return false;
      } finally {
        if (!unmountedRef.current) {
          setIsApplying(false);
        }
      }
    },
    [backend, showToast],
  );

  const start = useCallback(
    (task: Task) => {
      void runTask(task);
    },
    [runTask],
  );

  const cancel = useCallback(() => {
    autoAdvanceStopRequested.current = true;
    const jobId = activeJobId.current;
    if (jobId) {
      backend.cancelGeneration(jobId).catch((error: unknown) => {
        showToast(toErrorMessage(error, "中止できませんでした。"), "error");
      });
    }
  }, [backend, showToast]);

  const apply = useCallback(async () => {
    if (!changeSet) {
      return;
    }
    await applyResult(changeSet);
  }, [applyResult, changeSet]);

  const discard = useCallback(() => {
    setPhase("idle");
    setCurrentTask(null);
    setChangeSet(null);
    setErrorMessage(null);
    setApplyErrorMessage(null);
    setDisplay(createEmptyGenerationDisplay());
  }, []);

  const regenerate = useCallback(() => {
    if (currentTask) {
      void runTask(currentTask);
    }
  }, [currentTask, runTask]);

  const runAutoAdvance = useCallback(() => {
    autoAdvanceStopRequested.current = false;
    setAutoAdvancing(true);

    void (async () => {
      try {
        while (!autoAdvanceStopRequested.current && !unmountedRef.current) {
          await useWorkspaceStore.getState().refreshPipeline(backend);
          if (unmountedRef.current) {
            break;
          }
          const nextReady = useWorkspaceStore
            .getState()
            .pipeline.find((step) => step.state === "ready");
          if (!nextReady) {
            break;
          }
          const result = await runTask(nextReady.task);
          if (!result || autoAdvanceStopRequested.current || unmountedRef.current) {
            break;
          }
          const applied = await applyResult(result);
          if (!applied) {
            // 適用が失敗した工程を無限に生成し続けないよう、ここで止める。
            // 変更案は reviewing のまま残っているので、利用者が見直せる。
            break;
          }
        }
      } finally {
        if (!unmountedRef.current) {
          setAutoAdvancing(false);
        }
      }
    })();
  }, [applyResult, backend, runTask]);

  const stopAutoAdvance = useCallback(() => {
    autoAdvanceStopRequested.current = true;
    cancel();
  }, [cancel]);

  return {
    phase,
    currentTask,
    display,
    changeSet,
    errorMessage,
    isApplying,
    applyErrorMessage,
    autoAdvancing,
    start,
    cancel,
    apply,
    discard,
    regenerate,
    runAutoAdvance,
    stopAutoAdvance,
  };
}
