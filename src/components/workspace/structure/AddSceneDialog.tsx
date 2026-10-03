import { useState } from "react";
import type { NewScenePlan } from "../../../api/types";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import { findManuscriptChapter } from "../../../lib/overviewTree";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { Dialog } from "../../Dialog";
import { SceneFields } from "../document-form/SceneFields";
import { SubmitActions } from "./SubmitActions";
import "./StructureDialog.css";

interface AddSceneDialogProps {
  chapter: string;
  /** 初めに選んでおく位置。このシーンの前に足す。null なら章の末尾。 */
  before: string | null;
  onClose: () => void;
}

const EMPTY_SCENE: NewScenePlan = {
  title: "",
  summary: "",
  pov: null,
  characters: [],
  place: null,
  time: null,
  target_chars: null,
};

/** 章にシーンを、自分で書いて足すダイアログ。足す位置は、既存のシーンの前か、章の末尾から選ぶ。 */
export function AddSceneDialog({ chapter, before, onClose }: AddSceneDialogProps) {
  const structure = useStructureEdit();
  const submission = useSubmission();
  const overview = useWorkspaceStore((state) => state.overview);
  const chapterEntry = findManuscriptChapter(overview, chapter);
  const existingScenes = chapterEntry?.children ?? [];
  const [scene, setScene] = useState<NewScenePlan>(EMPTY_SCENE);
  const [position, setPosition] = useState<string | null>(before);

  async function submit(): Promise<void> {
    const succeeded = await submission.run(() =>
      structure.apply({ kind: "add_scene", chapter, before: position, scene }),
    );
    if (succeeded) {
      onClose();
    }
  }

  return (
    <Dialog title="シーンを追加" wide onClose={submission.isSubmitting ? null : onClose}>
      <div className="structure-dialog">
        <p className="structure-dialog__lead">
          追加先: {chapterEntry?.label ?? `第${Number(chapter)}章`}
        </p>
        <label className="app-field">
          <span>位置</span>
          <select
            value={position ?? ""}
            onChange={(event) => setPosition(event.target.value === "" ? null : event.target.value)}
          >
            {existingScenes.map((existing) => (
              <option key={existing.scene} value={existing.scene ?? ""}>
                {existing.label} の前
              </option>
            ))}
            <option value="">章の最後</option>
          </select>
        </label>
        <SceneFields scene={scene} onChange={setScene} />
        <SubmitActions
          submitLabel="追加"
          submittingLabel="追加しています…"
          isSubmitting={submission.isSubmitting}
          canSubmit={scene.title.trim() !== ""}
          errorMessage={submission.errorMessage}
          onSubmit={() => void submit()}
          onCancel={onClose}
        />
      </div>
    </Dialog>
  );
}
