import { useCallback, useRef, useState } from "react";
import { BackendError } from "../../api/backend";
import { useBackend } from "../../api/context";
import type { ChangeSet, Task } from "../../api/types";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
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
  autoAdvancing: boolean;
  start: (task: Task) => void;
  cancel: () => void;
  apply: () => Promise<void>;
  discard: () => void;
  regenerate: () => void;
  runAutoAdvance: () => void;
  stopAutoAdvance: () => void;
}

let jobSequence = 0;

function createJobId(): string {
  jobSequence += 1;
  return `job-${jobSequence}`;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "生成に失敗しました。";
}

/**
 * 「工程」タブと「この文書」タブが共有する、生成 1 回分のセッション。
 * 一度に実行できる生成は 1 件だけで、どちらのタブから始めても同じ進捗・結果を表示する。
 */
export function useGenerationSession(): GenerationSessionApi {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);

  const [phase, setPhase] = useState<GenerationPhase>("idle");
  const [currentTask, setCurrentTask] = useState<Task | null>(null);
  const [display, setDisplay] = useState<GenerationDisplay>(createEmptyGenerationDisplay());
  const [changeSet, setChangeSet] = useState<ChangeSet | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [autoAdvancing, setAutoAdvancing] = useState(false);

  const activeJobId = useRef<string | null>(null);
  const autoAdvanceStopRequested = useRef(false);

  const runTask = useCallback(
    async (task: Task): Promise<ChangeSet | null> => {
      const jobId = createJobId();
      activeJobId.current = jobId;
      setPhase("running");
      setCurrentTask(task);
      setDisplay(createEmptyGenerationDisplay());
      setChangeSet(null);
      setErrorMessage(null);

      try {
        const result = await backend.generate(jobId, task, (event) => {
          setDisplay((previous) => applyGenerationEvent(previous, event));
        });
        setChangeSet(result);
        setPhase("reviewing");
        return result;
      } catch (error) {
        if (error instanceof BackendError && error.kind === "cancelled") {
          setPhase("idle");
          return null;
        }
        const message = toErrorMessage(error);
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

  const applyResult = useCallback(
    async (result: ChangeSet): Promise<void> => {
      try {
        const overview = await backend.applyChangeSet(result);
        useWorkspaceStore.getState().setOverview(overview);
        await useWorkspaceStore.getState().refreshPipeline(backend);

        const editor = useEditorStore.getState();
        const touchesOpenDocument =
          editor.path !== null && result.files.some((file) => file.path === editor.path);
        if (touchesOpenDocument && editor.path !== null) {
          const file = await backend.readFile(editor.path);
          useEditorStore.getState().loadDocument(editor.path, file.content, file.hash);
        }

        setPhase("idle");
        setCurrentTask(null);
        setChangeSet(null);
        setDisplay(createEmptyGenerationDisplay());
        showToast(result.summary);
      } catch (error) {
        const message = toErrorMessage(error);
        setErrorMessage(message);
        setPhase("error");
        showToast(message, "error");
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
      void backend.cancelGeneration(jobId);
    }
  }, [backend]);

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
        while (!autoAdvanceStopRequested.current) {
          await useWorkspaceStore.getState().refreshPipeline(backend);
          const nextReady = useWorkspaceStore
            .getState()
            .pipeline.find((step) => step.state === "ready");
          if (!nextReady) {
            break;
          }
          const result = await runTask(nextReady.task);
          if (!result || autoAdvanceStopRequested.current) {
            break;
          }
          await applyResult(result);
        }
      } finally {
        setAutoAdvancing(false);
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
