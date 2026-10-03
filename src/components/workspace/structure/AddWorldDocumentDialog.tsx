import { useState } from "react";
import { useGeneratedAddition } from "../../../features/structure/useGeneratedAddition";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import { Dialog } from "../../Dialog";
import { OptionalTextField, TextAreaField, TextField } from "../document-form/formFields";
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

  function startGeneration(): void {
    void addition.start({ kind: "add_world_document", name, instruction: instruction.trim() });
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
            addition={addition}
            onStart={startGeneration}
            onCancel={onClose}
          >
            <FileNameField name={name} onChange={setName} />
          </GeneratedAdditionFields>
        ) : (
          <>
            <TextField label="題" value={title} onChange={setTitle} />
            <FileNameField name={name} onChange={setName} />
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
          </>
        )}
      </div>
    </Dialog>
  );
}

interface FileNameFieldProps {
  name: string | null;
  onChange: (name: string | null) => void;
}

/** 資料のファイル名の欄と、その説明。自分で書くときも AI に作らせるときも同じ（題は書かれる資料の先頭の見出し）。 */
function FileNameField({ name, onChange }: FileNameFieldProps) {
  return (
    <>
      <OptionalTextField
        label="ファイル名（英数字。空欄なら自動）"
        value={name}
        onChange={onChange}
      />
      <p className="structure-dialog__hint">
        world/ の下に、この名前の .md ファイルで保存します。空欄なら、題から自動で決めます。
      </p>
    </>
  );
}
