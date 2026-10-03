import { useState } from "react";
import type { FileChange } from "../../../../api/types";
import { buildDocumentView, isStructuredDocument, type ViewedSide } from "./documentComparison";
import { RawContent } from "./RawContent";
import { StructuredDocumentView } from "./StructuredDocumentView";
import { type ParsedDocumentState, useParsedDocument } from "./useParsedDocument";

interface FileChangeCardProps {
  file: FileChange;
}

/** 項目に分けて見せられなかった理由。分けられた（または分けない文書の）ときは null。 */
function rawContentNotice(state: ParsedDocumentState): string | null {
  if (state.status === "failed") {
    return `項目に分けられませんでした（${state.reason}）。書かれたまま表示しています。`;
  }
  if (state.status === "ready" && state.parsed.parse_error !== null) {
    return `項目に分けられないため、書かれたまま表示しています。${state.parsed.parse_error}`;
  }
  return null;
}

function readyDocument(state: ParsedDocumentState) {
  return state.status === "ready" ? state.parsed.document : null;
}

/**
 * 変更案の 1 ファイル。人物資料・章立ては項目と本文に分けて、それ以外は中身のままで見せる。
 * 変更前があれば、変更後との切り替えと、変わったところの印を付ける。
 */
export function FileChangeCard({ file }: FileChangeCardProps) {
  const [showingPrevious, setShowingPrevious] = useState(false);
  const after = useParsedDocument(file.path, file.content);
  const before = useParsedDocument(file.path, file.previous);

  const side: ViewedSide = showingPrevious ? "before" : "after";
  const shown = showingPrevious ? before : after;
  const counterpart = showingPrevious ? after : before;
  const counterpartName = showingPrevious ? "変更後" : "変更前";
  const shownContent = showingPrevious ? (file.previous ?? "") : file.content;
  const shownDocument = readyDocument(shown);
  const view =
    shownDocument !== null && isStructuredDocument(shownDocument)
      ? buildDocumentView(shownDocument, readyDocument(counterpart), side)
      : null;

  return (
    <div className="changeset-review__file">
      <div className="changeset-review__file-header">
        <span className="changeset-review__file-path">{file.path}</span>
        {file.previous !== null && (
          <button
            type="button"
            className="app-button"
            onClick={() => setShowingPrevious(!showingPrevious)}
          >
            {showingPrevious ? "変更後を見る" : "変更前を見る"}
          </button>
        )}
      </div>
      {view === null ? (
        <RawContent content={shownContent} notice={rawContentNotice(shown)} />
      ) : (
        <>
          {file.previous !== null && !view.compared && (
            <p className="changeset-review__comparison-note">
              {counterpartName}と比べられないため、印は付けていません。
            </p>
          )}
          <StructuredDocumentView view={view} />
        </>
      )}
    </div>
  );
}
