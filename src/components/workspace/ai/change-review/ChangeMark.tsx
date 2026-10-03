import type { SceneMark } from "./documentComparison";

const MARK_LABELS: Record<SceneMark, string> = {
  changed: "変更",
  added: "追加",
  removed: "削除",
  moved: "順序変更",
};

interface ChangeMarkProps {
  kind: SceneMark;
}

/** 変更案の差分の印。色だけに頼らず、文字でも伝える。 */
export function ChangeMark({ kind }: ChangeMarkProps) {
  return <span className={`change-mark change-mark--${kind}`}>{MARK_LABELS[kind]}</span>;
}
