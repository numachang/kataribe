import { useState } from "react";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import { listPlanChapters } from "../../../lib/overviewTree";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { Dialog } from "../../Dialog";
import { CHAPTER_FIELD_LABELS, STORYLINE_LABEL } from "../document-form/fieldLabels";
import { TextAreaField, TextField } from "../document-form/formFields";
import { SubmitActions } from "./SubmitActions";
import "./StructureDialog.css";

interface AddChapterDialogProps {
  /** 初めに選んでおく位置。この章の前に足す。null なら末尾。 */
  before: string | null;
  onClose: () => void;
}

const END_OF_CHAPTERS = "末尾";

/** 章を、自分で書いて足すダイアログ。足す位置は、既存の章の前か、末尾から選ぶ。 */
export function AddChapterDialog({ before, onClose }: AddChapterDialogProps) {
  const structure = useStructureEdit();
  const submission = useSubmission();
  const overview = useWorkspaceStore((state) => state.overview);
  const existingChapters = listPlanChapters(overview);
  const [title, setTitle] = useState("");
  const [storyline, setStoryline] = useState("");
  const [position, setPosition] = useState<string | null>(before);

  async function submit(): Promise<void> {
    const succeeded = await submission.run(() =>
      structure.apply({ kind: "add_chapter", before: position, title, storyline }),
    );
    if (succeeded) {
      onClose();
    }
  }

  return (
    <Dialog title="章を追加" onClose={submission.isSubmitting ? null : onClose}>
      <div className="structure-dialog">
        <TextField label={CHAPTER_FIELD_LABELS.title} value={title} onChange={setTitle} />
        <TextAreaField label={STORYLINE_LABEL} rows={6} value={storyline} onChange={setStoryline} />
        <label className="app-field">
          <span>位置</span>
          <select
            value={position ?? ""}
            onChange={(event) => setPosition(event.target.value === "" ? null : event.target.value)}
          >
            {existingChapters.map((existing) => (
              <option key={existing.chapter} value={existing.chapter ?? ""}>
                {existing.label} の前
              </option>
            ))}
            <option value="">{END_OF_CHAPTERS}</option>
          </select>
        </label>
        {position !== null && <InsertionNotes before={position} />}
        <SubmitActions
          submitLabel="追加"
          submittingLabel="追加しています…"
          isSubmitting={submission.isSubmitting}
          canSubmit={title.trim() !== ""}
          errorMessage={submission.errorMessage}
          onSubmit={() => void submit()}
          onCancel={onClose}
        />
      </div>
    </Dialog>
  );
}

/** 章を途中に足すときに、利用者に知らせておくこと。 */
function InsertionNotes({ before }: { before: string }) {
  return (
    <>
      <p className="structure-dialog__hint">
        第{Number(before)}章以降の章は、番号が 1
        つ後ろにずれます（本文のフォルダも一緒に移ります）。
      </p>
      <p className="structure-dialog__hint">
        シーン構成の無い章を途中に足すと、それより後ろの本文の工程は、その章のシーン構成ができるまで進みません。
      </p>
    </>
  );
}
