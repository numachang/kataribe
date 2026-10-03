import { useId } from "react";
import "./StructureDialog.css";

/** 追加のしかた。`manual` は自分で書く、`generated` は指示から AI に作らせる。 */
export type AdditionMode = "manual" | "generated";

const MODE_LABELS: Record<AdditionMode, string> = {
  manual: "自分で書く",
  generated: "AI に作らせる",
};

const MODES = Object.keys(MODE_LABELS) as AdditionMode[];

interface AdditionModeSwitchProps {
  mode: AdditionMode;
  onChange: (mode: AdditionMode) => void;
  /** 切り替えを止める（追加を実行している間など）。 */
  disabled?: boolean;
}

/**
 * 追加のダイアログの上部で、自分で書くか AI に作らせるかを選ぶ。
 * ラジオボタンなので、キーボードの矢印でも切り替えられる。見た目は 2 つ並んだ選択肢にする。
 */
export function AdditionModeSwitch({ mode, onChange, disabled = false }: AdditionModeSwitchProps) {
  const groupName = useId();
  return (
    <div className="addition-mode-switch" role="radiogroup" aria-label="追加のしかた">
      {MODES.map((candidate) => (
        <label key={candidate} className="addition-mode-switch__option">
          <input
            type="radio"
            name={groupName}
            checked={mode === candidate}
            disabled={disabled}
            onChange={() => onChange(candidate)}
          />
          <span>{MODE_LABELS[candidate]}</span>
        </label>
      ))}
    </div>
  );
}
