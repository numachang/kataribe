import type { MoveFileChange } from "../../../../api/types";
import { ChangeMark } from "./ChangeMark";

interface MoveCardProps {
  file: MoveFileChange;
}

/** 変更案のうち、ファイルまたはフォルダの改名（章の番号の振り直し）。中身は変わらないことを添えて、行き先を見せる。 */
export function MoveCard({ file }: MoveCardProps) {
  return (
    <div className="changeset-review__file">
      <div className="changeset-review__file-header">
        <span className="changeset-review__file-path">
          {file.from} → {file.to}
        </span>
        <ChangeMark kind="renamed" />
      </div>
      <p className="changeset-review__move-note">中身は変わりません。</p>
    </div>
  );
}
