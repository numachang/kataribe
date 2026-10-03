import type { SceneMark } from "./documentComparison";

/** シーンの印に、ファイルの改名（`renamed`。章の番号の振り直し）を足したもの。 */
type ChangeMarkKind = SceneMark | "renamed";

const MARK_LABELS: Record<ChangeMarkKind, string> = {
  changed: "変更",
  added: "追加",
  removed: "削除",
  moved: "順序変更",
  renamed: "移動",
};

interface ChangeMarkProps {
  kind: ChangeMarkKind;
}

/** 変更案の差分の印。色だけに頼らず、文字でも伝える。 */
export function ChangeMark({ kind }: ChangeMarkProps) {
  return <span className={`change-mark change-mark--${kind}`}>{MARK_LABELS[kind]}</span>;
}
