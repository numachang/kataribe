import type { TrashFileChange } from "../../../../api/types";
import { TrashedFileList } from "../../structure/TrashedFileList";
import { ChangeMark } from "./ChangeMark";

interface TrashCardProps {
  file: TrashFileChange;
}

/** 変更案のうち、ゴミ箱へ移す 1 つの変更。消えるのではなく、ゴミ箱へ移ることと、移るファイルを見せる。 */
export function TrashCard({ file }: TrashCardProps) {
  return (
    <div className="changeset-review__file">
      <div className="changeset-review__file-header">
        <span className="changeset-review__file-path">{file.path}</span>
        <ChangeMark kind="removed" />
      </div>
      <div className="changeset-review__trash">
        <p className="changeset-review__trash-heading">ゴミ箱へ移すファイル</p>
        <TrashedFileList files={file.files} />
      </div>
    </div>
  );
}
