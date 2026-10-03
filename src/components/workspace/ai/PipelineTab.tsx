import { useEffect } from "react";
import { useBackend } from "../../../api/context";
import type { PipelineStep, StepState, Task } from "../../../api/types";
import { useGenerationSessionContext } from "../../../features/generation/GenerationSessionProvider";
import { useGenerationStartBlockedReason } from "../../../features/structure/useStructureAvailability";
import { toErrorMessage } from "../../../lib/errorMessage";
import { taskKey } from "../../../lib/taskKey";
import { useUiStore } from "../../../store/uiStore";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import "./PipelineTab.css";

const STAGE_GROUPS: Array<{ label: string; kinds: Array<Task["kind"]> }> = [
  { label: "企画", kinds: ["concept"] },
  { label: "文体", kinds: ["style"] },
  { label: "世界観", kinds: ["world"] },
  { label: "登場人物", kinds: ["cast", "character"] },
  { label: "あらすじ", kinds: ["synopsis"] },
  { label: "章立て", kinds: ["outline"] },
  { label: "シーン構成", kinds: ["scene_plan"] },
  { label: "本文", kinds: ["draft"] },
];

const STATE_LABELS: Record<StepState, string> = {
  done: "完了",
  ready: "生成できます",
  blocked: "保留",
};

function groupSteps(pipeline: PipelineStep[]): Array<{ label: string; steps: PipelineStep[] }> {
  return STAGE_GROUPS.map((group) => ({
    label: group.label,
    steps: pipeline.filter((step) => group.kinds.includes(step.task.kind)),
  })).filter((group) => group.steps.length > 0);
}

/** 「工程」タブ。企画から本文まで、段階ごとに何が終わっていて何ができるかを見せる。 */
export function PipelineTab() {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const pipeline = useWorkspaceStore((state) => state.pipeline);
  const session = useGenerationSessionContext();

  useEffect(() => {
    useWorkspaceStore
      .getState()
      .refreshPipeline(backend)
      .catch((error: unknown) => {
        showToast(toErrorMessage(error, "工程の一覧を読み込めませんでした。"), "error");
      });
  }, [backend, showToast]);

  const groups = groupSteps(pipeline);
  const firstReady = pipeline.find((step) => step.state === "ready") ?? null;
  const startBlockedReason = useGenerationStartBlockedReason();
  const isBusy = session.phase !== "idle" || startBlockedReason !== null;

  return (
    <div className="pipeline-tab">
      <div className="pipeline-tab__toolbar">
        <button
          type="button"
          className="app-button app-button--primary"
          disabled={!firstReady || isBusy}
          title={startBlockedReason ?? undefined}
          onClick={() => firstReady && session.start(firstReady.task)}
        >
          次の工程を実行
        </button>
        {session.autoAdvancing ? (
          <button
            type="button"
            className="app-button app-button--danger"
            onClick={session.stopAutoAdvance}
          >
            自動で進めるのを止める
          </button>
        ) : (
          <button
            type="button"
            className="app-button"
            disabled={!firstReady || isBusy}
            title={startBlockedReason ?? undefined}
            onClick={session.runAutoAdvance}
          >
            自動で進める
          </button>
        )}
      </div>

      <div className="pipeline-tab__groups">
        {groups.map((group) => (
          <section key={group.label} className="pipeline-tab__group">
            <h3>{group.label}</h3>
            <ul>
              {group.steps.map((step) => (
                <li key={taskKey(step.task)} className="pipeline-tab__step">
                  <div className="pipeline-tab__step-main">
                    <span className={`pipeline-tab__badge pipeline-tab__badge--${step.state}`}>
                      {STATE_LABELS[step.state]}
                    </span>
                    <span className="pipeline-tab__step-label">{step.label}</span>
                  </div>
                  {step.state === "blocked" && step.blocked_by && (
                    <p className="pipeline-tab__blocked-reason">{step.blocked_by}</p>
                  )}
                  {step.state === "ready" && (
                    <button
                      type="button"
                      className="app-button"
                      disabled={isBusy}
                      title={startBlockedReason ?? undefined}
                      onClick={() => session.start(step.task)}
                    >
                      生成
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}
