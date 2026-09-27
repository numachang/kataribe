import { useState } from "react";
import { useGenerationSessionContext } from "../../../features/generation/GenerationSessionProvider";
import "./ChangeSetReview.css";

/** 生成が終わったあとの変更案。ファイルごとに内容を見せ、適用するか破棄するか選ばせる。 */
export function ChangeSetReview() {
  const session = useGenerationSessionContext();
  const [showPreviousFor, setShowPreviousFor] = useState<Record<string, boolean>>({});
  const changeSet = session.changeSet;

  if (!changeSet) {
    return null;
  }

  return (
    <div className="changeset-review">
      <p className="changeset-review__summary">{changeSet.summary}</p>

      <div className="changeset-review__files">
        {changeSet.files.map((file) => {
          const showingPrevious = showPreviousFor[file.path] ?? false;
          return (
            <div key={file.path} className="changeset-review__file">
              <div className="changeset-review__file-header">
                <span className="changeset-review__file-path">{file.path}</span>
                {file.previous !== null && (
                  <button
                    type="button"
                    className="app-button"
                    onClick={() =>
                      setShowPreviousFor((current) => ({
                        ...current,
                        [file.path]: !showingPrevious,
                      }))
                    }
                  >
                    {showingPrevious ? "変更後を見る" : "変更前を見る"}
                  </button>
                )}
              </div>
              <pre className="changeset-review__content">
                {showingPrevious ? file.previous : file.content}
              </pre>
            </div>
          );
        })}
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
