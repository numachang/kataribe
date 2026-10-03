import { useGenerationSessionContext } from "../../../features/generation/GenerationSessionProvider";
import "./ChangeSetReview.css";
import { FileChangeCard } from "./change-review/FileChangeCard";

/** 生成が終わったあとの変更案。ファイルごとに内容を見せ、適用するか破棄するか選ばせる。 */
export function ChangeSetReview() {
  const session = useGenerationSessionContext();
  const changeSet = session.changeSet;

  if (!changeSet) {
    return null;
  }

  return (
    <div className="changeset-review">
      <p className="changeset-review__summary">{changeSet.summary}</p>

      <div className="changeset-review__files">
        {changeSet.files.map((file) => (
          <FileChangeCard key={file.path} file={file} />
        ))}
      </div>

      {session.applyErrorMessage && (
        <p className="changeset-review__error">{session.applyErrorMessage}</p>
      )}

      <div className="changeset-review__actions">
        <button
          type="button"
          className="app-button"
          disabled={session.isApplying}
          onClick={session.discard}
        >
          破棄
        </button>
        <button
          type="button"
          className="app-button"
          disabled={session.isApplying}
          onClick={session.regenerate}
        >
          もう一度生成
        </button>
        <button
          type="button"
          className="app-button app-button--primary"
          disabled={session.isApplying}
          onClick={() => void session.apply()}
        >
          {session.isApplying ? "適用しています…" : "適用"}
        </button>
      </div>
    </div>
  );
}
