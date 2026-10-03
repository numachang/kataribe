import { useState } from "react";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import { Dialog } from "../../Dialog";
import { OptionalTextField, TextAreaField, TextField } from "../document-form/formFields";
import { SubmitActions } from "./SubmitActions";
import "./StructureDialog.css";

interface AddWorldDocumentDialogProps {
  onClose: () => void;
}

/** 世界観の資料（用語集など）を、自分で書いて足すダイアログ。 */
export function AddWorldDocumentDialog({ onClose }: AddWorldDocumentDialogProps) {
  const structure = useStructureEdit();
  const submission = useSubmission();
  const [title, setTitle] = useState("");
  const [name, setName] = useState<string | null>(null);
  const [body, setBody] = useState("");

  async function submit(): Promise<void> {
    const succeeded = await submission.run(() =>
      structure.apply({ kind: "add_world_document", name, title, body }),
    );
    if (succeeded) {
      onClose();
    }
  }

  return (
    <Dialog title="世界観の資料を追加" onClose={submission.isSubmitting ? null : onClose}>
      <div className="structure-dialog">
        <TextField label="題" value={title} onChange={setTitle} />
        <OptionalTextField
          label="ファイル名（英数字。空欄なら自動）"
          value={name}
          onChange={setName}
        />
        <p className="structure-dialog__hint">
          world/ の下に、この名前の .md ファイルで保存します。空欄なら、題から自動で決めます。
        </p>
        <TextAreaField label="本文" rows={10} value={body} onChange={setBody} />
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
