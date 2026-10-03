import { useId, useState } from "react";
import type { CharacterMeta } from "../../../api/types";
import { useCharacterIdField } from "../../../features/structure/useCharacterIdField";
import { useGeneratedAddition } from "../../../features/structure/useGeneratedAddition";
import { useStructureEdit } from "../../../features/structure/useStructureEdit";
import { useSubmission } from "../../../features/structure/useSubmission";
import { Dialog } from "../../Dialog";
import { CharacterForm } from "../document-form/CharacterForm";
import { TextAreaField } from "../document-form/formFields";
import { type AdditionMode, AdditionModeSwitch } from "./AdditionModeSwitch";
import { GeneratedAdditionFields } from "./GeneratedAdditionFields";
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

const INSTRUCTION_PLACEHOLDER =
  "例: 主人公を子どもの頃から知っている、年上の幼なじみ。口は悪いが面倒見がよい。";

/**
 * 人物を足すダイアログ。自分で書くときは、入力した内容がそのまま人物資料になるので、確認の手順は挟まない。
 * AI に作らせるときは、指示から生成を始め、変更案の確認は AI パネルで行う。
 */
export function AddCharacterDialog({ onClose }: AddCharacterDialogProps) {
  const structure = useStructureEdit();
  const submission = useSubmission();
  const addition = useGeneratedAddition(onClose);
  // 切り替えても、書きかけの内容を失わないよう、どちらの入力もここで持つ
  const [mode, setMode] = useState<AdditionMode>("manual");
  const [instruction, setInstruction] = useState("");
  const [meta, setMeta] = useState<CharacterMeta>(EMPTY_CHARACTER);
  const [body, setBody] = useState("");
  const idField = useCharacterIdField(meta.reading ?? "", meta.name);
  const idNoteElementId = useId();

  async function submit(): Promise<void> {
    const succeeded = await submission.run(() =>
      structure.apply({ kind: "add_character", id: idField.idToSend, meta, body }),
    );
    if (succeeded) {
      onClose();
    }
  }

  function startGeneration(): void {
    void addition.start({ kind: "add_character", instruction: instruction.trim() });
  }

  const isBusy = submission.isSubmitting || addition.isStarting;

  return (
    <Dialog title="人物を追加" onClose={isBusy ? null : onClose}>
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
          />
        ) : (
          <>
            <CharacterForm meta={meta} onChange={setMeta} />
            <label className="app-field">
              <span>ID</span>
              <input
                value={idField.idText}
                placeholder="読みや名前から自動で決めます"
                aria-invalid={idField.problem !== null}
                aria-describedby={idNoteElementId}
                onChange={(event) => idField.changeId(event.target.value)}
              />
            </label>
            {idField.problem !== null ? (
              <p
                id={idNoteElementId}
                className="structure-dialog__hint structure-dialog__hint--error"
              >
                ID「{idField.idText.trim()}」は使えません。{idField.problem}
              </p>
            ) : (
              <p id={idNoteElementId} className="structure-dialog__hint">
                ファイル名になります（小文字の英数字とハイフン）。空欄のままなら自動で決めます。
              </p>
            )}
            <TextAreaField label="詳細" rows={6} value={body} onChange={setBody} />
            <SubmitActions
              submitLabel="追加"
              submittingLabel="追加しています…"
              isSubmitting={submission.isSubmitting}
              canSubmit={meta.name.trim() !== "" && idField.problem === null}
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
