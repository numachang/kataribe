import { useEffect, useState } from "react";
import type { StructurePlan } from "../../../api/types";
import type { RemoveEdit } from "../../../features/structure/structureRequest";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import { toErrorMessage } from "../../../lib/errorMessage";
import { Dialog } from "../../Dialog";
import { RemovalDetails } from "./RemovalDetails";
import "./StructureDialog.css";

const CONFLICT_MESSAGE = "作品が変わったため削除しませんでした。";

interface RemoveConfirmDialogProps {
  edit: RemoveEdit;
  onClose: () => void;
}

/** 変更案を作っている途中か、作れたか、作れなかったか。 */
type PlanState =
  | { status: "planning" }
  | { status: "ready"; plan: StructurePlan }
  | { status: "failed"; message: string };

/**
 * 削除の確認。ゴミ箱へ移るもの（本文は字数つき）と、消すと参照が切れるシーンを見せてから、反映する。
 * 確認している間に作品が外で変わっていたら、削除せずにそのことを知らせ、もう一度確かめられるようにする。
 */
export function RemoveConfirmDialog({ edit, onClose }: RemoveConfirmDialogProps) {
  const structure = useStructureEdit();
  const submission = useSubmission();
  const [planState, setPlanState] = useState<PlanState>({ status: "planning" });
  const [attempt, setAttempt] = useState(0);

  // biome-ignore lint/correctness/useExhaustiveDependencies: attempt は「もう一度確かめる」で変更案を作り直すための合図
  useEffect(() => {
    let isStale = false;
    setPlanState({ status: "planning" });
    structure
      .prepare(edit)
      .then((plan) => {
        if (!isStale) {
          setPlanState({ status: "ready", plan });
        }
      })
      .catch((error: unknown) => {
        if (!isStale) {
          setPlanState({
            status: "failed",
            message: toErrorMessage(error, "削除する内容を確かめられませんでした。"),
          });
        }
      });
    return () => {
      isStale = true;
    };
  }, [edit, structure.prepare, attempt]);

  async function remove(plan: StructurePlan): Promise<void> {
    const succeeded = await submission.run(() => structure.commit(plan));
    if (succeeded) {
      onClose();
    }
  }

  function checkAgain(): void {
    submission.clearFailure();
    setAttempt(attempt + 1);
  }

  const readyPlan = planState.status === "ready" ? planState.plan : null;

  return (
    <Dialog title="削除の確認" onClose={submission.isSubmitting ? null : onClose}>
      <div className="structure-dialog">
        {planState.status === "planning" && (
          <p className="structure-dialog__lead">確かめています…</p>
        )}
        {planState.status === "failed" && (
          <p className="structure-dialog__error" role="alert">
            {planState.message}
          </p>
        )}
        {planState.status === "ready" && <RemovalDetails plan={planState.plan} />}

        {submission.isConflict ? (
          <p className="structure-dialog__error" role="alert">
            {CONFLICT_MESSAGE}
          </p>
        ) : (
          submission.errorMessage !== null && (
            <p className="structure-dialog__error" role="alert">
              {submission.errorMessage}
            </p>
          )
        )}

        <div className="structure-dialog__actions">
          <button
            type="button"
            className="app-button"
            disabled={submission.isSubmitting}
            onClick={onClose}
          >
            やめる
          </button>
          {submission.isConflict ? (
            <button type="button" className="app-button" onClick={checkAgain}>
              もう一度確かめる
            </button>
          ) : (
            <button
              type="button"
              className="app-button app-button--danger"
              disabled={readyPlan === null || submission.isSubmitting}
              onClick={() => {
                if (readyPlan !== null) {
                  void remove(readyPlan);
                }
              }}
            >
              {submission.isSubmitting ? "移しています…" : "ゴミ箱へ移す"}
            </button>
          )}
        </div>
      </div>
    </Dialog>
  );
}
