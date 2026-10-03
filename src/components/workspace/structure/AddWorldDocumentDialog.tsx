import { useId, useState } from "react";
import { useGeneratedAddition } from "../../../features/structure/useGeneratedAddition";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import {
  useWorldDocumentNameField,
  type WorldDocumentNameField,
} from "../../../features/structure/useWorldDocumentNameField";
import { Dialog } from "../../Dialog";
import { TextAreaField, TextField } from "../document-form/formFields";
import { type AdditionMode, AdditionModeSwitch } from "./AdditionModeSwitch";
import { GeneratedAdditionFields } from "./GeneratedAdditionFields";
import { SubmitActions } from "./SubmitActions";
import "./StructureDialog.css";

interface AddWorldDocumentDialogProps {
  onClose: () => void;
}

const INSTRUCTION_PLACEHOLDER =
  "例: 港町で使われる、天気にまつわる言い伝えを用語集としてまとめてください。";

/** 世界観の資料（用語集など）を足すダイアログ。自分で書くか、指示から AI に作らせる（人物のダイアログと同じ）。 */
export function AddWorldDocumentDialog({ onClose }: AddWorldDocumentDialogProps) {
  const structure = useStructureEdit();
  const submission = useSubmission();
  const addition = useGeneratedAddition(onClose);
  // 切り替えても、書きかけの内容を失わないよう、どちらの入力もここで持つ。ファイル名は両方で同じ意味
  const [mode, setMode] = useState<AdditionMode>("manual");
  const [instruction, setInstruction] = useState("");
  const [title, setTitle] = useState("");
  const nameField = useWorldDocumentNameField();
  const [body, setBody] = useState("");

  async function submit(): Promise<void> {
    const succeeded = await submission.run(() =>
      structure.apply({ kind: "add_world_document", name: nameField.nameToSend, title, body }),
    );
    if (succeeded) {
      onClose();
    }
  }

  function startGeneration(): void {
    void addition.start({
      kind: "add_world_document",
      name: nameField.nameToSend,
      instruction: instruction.trim(),
    });
  }

  const isBusy = submission.isSubmitting || addition.isStarting;

  return (
    <Dialog title="世界観の資料を追加" onClose={isBusy ? null : onClose}>
      <div className="structure-dialog">
        <AdditionModeSwitch mode={mode} onChange={setMode} disabled={isBusy} />
        {mode === "generated" ? (
          <GeneratedAdditionFields
            instruction={instruction}
            onInstructionChange={setInstruction}
            instructionPlaceholder={INSTRUCTION_PLACEHOLDER}
            areExtraFieldsValid={nameField.problem === null}
            addition={addition}
            onStart={startGeneration}
            onCancel={onClose}
          >
            <FileNameField field={nameField} />
          </GeneratedAdditionFields>
        ) : (
          <>
            <TextField label="題" value={title} onChange={setTitle} />
            <FileNameField field={nameField} />
            <TextAreaField label="本文" rows={10} value={body} onChange={setBody} />
            <SubmitActions
              submitLabel="追加"
              submittingLabel="追加しています…"
              isSubmitting={submission.isSubmitting}
              canSubmit={title.trim() !== "" && nameField.problem === null}
              errorMessage={submission.errorMessage}
              onSubmit={() => void submit()}
              onCancel={onClose}
            />
          </>
        )}
      </div>
    </Dialog>
  );
}

interface FileNameFieldProps {
  field: WorldDocumentNameField;
}

/**
 * 資料のファイル名の欄と、その説明。自分で書くときも AI に作らせるときも同じ（題は書かれる資料の先頭の見出し）。
 * 使えない名前なら、説明の代わりに理由を出す。
 */
function FileNameField({ field }: FileNameFieldProps) {
  const noteElementId = useId();
  return (
    <>
      <label className="app-field">
        <span>ファイル名（英数字。空欄なら自動）</span>
        <input
          value={field.nameText}
          aria-invalid={field.problem !== null}
          aria-describedby={noteElementId}
          onChange={(event) => field.changeName(event.target.value)}
        />
      </label>
      {field.problem !== null ? (
        <p id={noteElementId} className="structure-dialog__hint structure-dialog__hint--error">
          ファイル名「{field.nameToSend}」は使えません。{field.problem}
        </p>
      ) : (
        <p id={noteElementId} className="structure-dialog__hint">
          world/ の下に、この名前の .md ファイルで保存します。空欄なら、題から自動で決めます。
        </p>
      )}
    </>
  );
}
