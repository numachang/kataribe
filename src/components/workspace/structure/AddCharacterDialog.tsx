import { useState } from "react";
import type { CharacterMeta } from "../../../api/types";
import { useCharacterIdField } from "../../../features/structure/useCharacterIdField";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import { Dialog } from "../../Dialog";
import { CharacterForm } from "../document-form/CharacterForm";
import { TextAreaField } from "../document-form/formFields";
import { SubmitActions } from "./SubmitActions";
import "./StructureDialog.css";

interface AddCharacterDialogProps {
  onClose: () => void;
}

const EMPTY_CHARACTER: CharacterMeta = {
  name: "",
  reading: null,
  role: "",
  summary: "",
  order: null,
};

/** 人物を、自分で書いて足すダイアログ。入力した内容がそのまま人物資料になるので、確認の手順は挟まない。 */
export function AddCharacterDialog({ onClose }: AddCharacterDialogProps) {
  const structure = useStructureEdit();
  const submission = useSubmission();
  const [meta, setMeta] = useState<CharacterMeta>(EMPTY_CHARACTER);
  const [body, setBody] = useState("");
  const idField = useCharacterIdField(meta.reading ?? "", meta.name);

  async function submit(): Promise<void> {
    const succeeded = await submission.run(() =>
      structure.apply({ kind: "add_character", id: idField.idToSend, meta, body }),
    );
    if (succeeded) {
      onClose();
    }
  }

  return (
    <Dialog title="人物を追加" onClose={submission.isSubmitting ? null : onClose}>
      <div className="structure-dialog">
        <CharacterForm meta={meta} onChange={setMeta} />
        <label className="app-field">
          <span>ID</span>
          <input
            value={idField.idText}
            placeholder="読みや名前から自動で決めます"
            onChange={(event) => idField.changeId(event.target.value)}
          />
        </label>
        <p className="structure-dialog__hint">
          ファイル名になります（小文字の英数字とハイフン）。空欄のままなら自動で決めます。
        </p>
        <TextAreaField label="詳細" rows={6} value={body} onChange={setBody} />
        <SubmitActions
          submitLabel="追加"
          submittingLabel="追加しています…"
          isSubmitting={submission.isSubmitting}
          canSubmit={meta.name.trim() !== ""}
          errorMessage={submission.errorMessage}
          onSubmit={() => void submit()}
          onCancel={onClose}
        />
      </div>
    </Dialog>
  );
}
