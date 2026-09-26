import { Dialog } from "../Dialog";
import "./ConflictDialog.css";

interface ConflictDialogProps {
  path: string;
  onReload: () => void;
  onOverwrite: () => void;
}

/** 保存しようとした文書が外部で変更されていたときに、どちらを残すか選ばせる。 */
export function ConflictDialog({ path, onReload, onOverwrite }: ConflictDialogProps) {
  return (
    <Dialog title="外部で変更されています" onClose={null}>
      <p>
        「{path}」は、この画面を開いている間に外部で変更されたようです。
        <br />
        今の編集内容を捨てて読み込み直すか、外部の変更を上書きして今の内容を残すか選んでください。
      </p>
      <div className="conflict-dialog__actions">
        <button type="button" className="app-button" onClick={onReload}>
          再読み込み
        </button>
        <button type="button" className="app-button app-button--primary" onClick={onOverwrite}>
          上書きする
        </button>
      </div>
    </Dialog>
  );
}
