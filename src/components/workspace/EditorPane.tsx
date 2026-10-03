import type { KeyboardEvent } from "react";
import { useDocumentEditor } from "../../features/editor/useDocumentEditor";
import { documentBody, withDocumentBody } from "../../lib/editableDocument";
import { isManuscriptFile } from "../../lib/manuscript";
import { findOverviewEntry } from "../../lib/overviewTree";
import { useSettingsStore } from "../../store/settingsStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { ConflictDialog } from "./ConflictDialog";
import { bodyHeading, DocumentForm } from "./document-form/DocumentForm";
import { RubyPreview } from "./RubyPreview";
import { StatusBar } from "./StatusBar";
import "./EditorPane.css";

const FONT_FAMILY_VARIABLE = {
  mincho: "var(--font-mincho)",
  gothic: "var(--font-gothic)",
} as const;

function EmptyPane({ message }: { message: string }) {
  return (
    <div className="editor-pane editor-pane--empty">
      <p>{message}</p>
    </div>
  );
}

/**
 * 中央のエディタ。IME・アンドゥを OS 標準のまま扱えるよう、本文は素の textarea をそのまま使う。
 * 人物資料・章立ては、本文の上に項目のフォームを置く。
 */
export function EditorPane() {
  const editor = useDocumentEditor();
  const overview = useWorkspaceStore((state) => state.overview);
  const currentPath = useWorkspaceStore((state) => state.currentPath);
  const settings = useSettingsStore((state) => state.settings);

  // 競合は、今表示している文書と無関係な（切り替え済みの）文書について起きることもあるため、
  // 選択・読み込みの状態より先に判定する。
  if (editor.conflict) {
    return (
      <div className="editor-pane">
        <ConflictDialog
          path={editor.conflict.path}
          onReload={() => void editor.resolveConflictByReloading()}
          onOverwrite={() => void editor.resolveConflictByOverwriting()}
          onDismiss={editor.dismissConflict}
        />
      </div>
    );
  }

  if (currentPath === null) {
    return <EmptyPane message="左の目次から文書を選んでください。" />;
  }

  const entry = findOverviewEntry(overview, currentPath);
  if (entry && !entry.exists) {
    return <EmptyPane message="この文書はまだ生成されていません。" />;
  }

  if (editor.path !== currentPath || editor.document === null) {
    // 前の文書から切り替わっている途中。ここで前の文書の内容を表示し続けると、
    // 目次の選択と表示・保存先がずれて見えるため、読み込み中の空の状態を出す。
    return <EmptyPane message="読み込んでいます…" />;
  }

  const fontStyle = settings?.editor.font_style ?? "mincho";
  const fontSize = settings?.editor.font_size ?? 17;
  const lineHeight = settings?.editor.line_height ?? 1.9;
  const canPreviewRuby = isManuscriptFile(currentPath);

  const document = editor.document;
  const body = documentBody(document);
  const heading = bodyHeading(document);

  // フォームの入力欄でも保存できるよう、本文とフォームを包む要素で受ける。
  function handleKeyDown(event: KeyboardEvent<HTMLElement>): void {
    const isSaveShortcut = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s";
    if (isSaveShortcut && !event.nativeEvent.isComposing) {
      event.preventDefault();
      editor.saveNow();
    }
  }

  return (
    <div className="editor-pane">
      <div className="editor-pane__toolbar">
        <span className="editor-pane__path">{currentPath}</span>
        <div className="editor-pane__toolbar-actions">
          {canPreviewRuby && (
            <button type="button" className="app-button" onClick={editor.toggleRubyPreview}>
              {editor.rubyPreview ? "編集に戻る" : "ルビ・傍点プレビュー"}
            </button>
          )}
          <button
            type="button"
            className="app-button"
            onClick={() => editor.setVertical(!editor.vertical)}
          >
            {editor.vertical ? "横書きにする" : "縦書きにする"}
          </button>
        </div>
      </div>

      {/* biome-ignore lint/a11y/noStaticElementInteractions: 子の入力欄から伝わるキー操作を受けるだけで、この要素自体は操作の対象ではない */}
      <section className="editor-pane__body" onKeyDown={handleKeyDown}>
        {document.kind !== "text" && (
          <div className="editor-pane__form">
            <DocumentForm document={document} onChange={editor.onDocumentChange} />
          </div>
        )}
        <div className="editor-pane__text">
          {editor.parseError !== null && (
            <div className="editor-pane__notice">
              <p>front matter を読めないため、ファイルをそのまま表示しています。</p>
              <p className="editor-pane__notice-reason">{editor.parseError}</p>
            </div>
          )}
          {heading !== null && <h2 className="editor-pane__heading">{heading}</h2>}
          {editor.rubyPreview ? (
            <RubyPreview
              text={body}
              vertical={editor.vertical}
              fontFamily={FONT_FAMILY_VARIABLE[fontStyle]}
              fontSize={fontSize}
              lineHeight={lineHeight}
            />
          ) : (
            <textarea
              className={`editor-pane__textarea${editor.vertical ? " editor-pane__textarea--vertical" : ""}`}
              style={{ fontFamily: FONT_FAMILY_VARIABLE[fontStyle], fontSize, lineHeight }}
              value={body}
              onChange={(event) =>
                editor.onDocumentChange(withDocumentBody(document, event.target.value))
              }
              spellCheck={false}
              aria-label={currentPath}
            />
          )}
        </div>
      </section>

      <StatusBar
        text={body}
        targetChars={entry?.target_chars ?? null}
        status={editor.status}
        errorMessage={editor.errorMessage}
      />
    </div>
  );
}
