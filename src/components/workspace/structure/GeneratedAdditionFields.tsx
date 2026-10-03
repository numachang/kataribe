import type { ReactNode } from "react";
import type { GeneratedAdditionApi } from "../../../features/structure/useGeneratedAddition";
import { SubmitActions } from "./SubmitActions";
import "./StructureDialog.css";

interface GeneratedAdditionFieldsProps {
  instruction: string;
  onInstructionChange: (instruction: string) => void;
  /** 何を書けばよいかの例文。 */
  instructionPlaceholder: string;
  /** 指示の欄の下に足す入力欄（世界観の資料のファイル名など）。 */
  children?: ReactNode;
  addition: GeneratedAdditionApi;
  onStart: () => void;
  onCancel: () => void;
}

/**
 * 追加のダイアログで「AI に作らせる」を選んだときの入力欄と、生成を始めるボタン。
 * 指示は必須で、始められない理由（生成の途中など）があればボタンを押せなくして理由を添える。
 * 始められるのは「生成を始める」ボタンだけにする（入力欄の Enter では始めない。日本語入力の確定の Enter で始めてしまうため）。
 */
export function GeneratedAdditionFields({
  instruction,
  onInstructionChange,
  instructionPlaceholder,
  children,
  addition,
  onStart,
  onCancel,
}: GeneratedAdditionFieldsProps) {
  return (
    <>
      <p className="structure-dialog__lead">
        AI パネルで進み具合を見て、変更案を確かめてから適用します。
      </p>
      <label className="app-field">
        <span>指示</span>
        <textarea
          rows={5}
          value={instruction}
          placeholder={instructionPlaceholder}
          aria-required="true"
          spellCheck={false}
          onChange={(event) => onInstructionChange(event.target.value)}
        />
      </label>
      {children}
      {addition.blockedReason !== null && (
        <p className="structure-dialog__note" role="status">
          {addition.blockedReason}
        </p>
      )}
      <SubmitActions
        submitLabel="生成を始める"
        submittingLabel="始めています…"
        isSubmitting={addition.isStarting}
        canSubmit={instruction.trim() !== "" && addition.blockedReason === null}
        errorMessage={addition.errorMessage}
        onSubmit={onStart}
        onCancel={onCancel}
      />
    </>
  );
}
