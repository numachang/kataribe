import type { KeyboardEvent } from "react";
import { useDocumentEditor } from "../../features/editor/useDocumentEditor";
import { isManuscriptFile } from "../../lib/manuscript";
import { findOverviewEntry } from "../../lib/overviewTree";
import { useSettingsStore } from "../../store/settingsStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { ConflictDialog } from "./ConflictDialog";
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

/** 中央のエディタ。IME・アンドゥを OS 標準のまま扱えるよう、素の textarea をそのまま使う。 */
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

  if (editor.path !== currentPath) {
    // 前の文書から切り替わっている途中。ここで前の文書の内容を表示し続けると、
    // 目次の選択と表示・保存先がずれて見えるため、読み込み中の空の状態を出す。
    return <EmptyPane message="読み込んでいます…" />;
  }

  const fontStyle = settings?.editor.font_style ?? "mincho";
  const fontSize = settings?.editor.font_size ?? 17;
  const lineHeight = settings?.editor.line_height ?? 1.9;
  const canPreviewRuby = isManuscriptFile(currentPath);

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
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

      <div className="editor-pane__body">
        {editor.rubyPreview ? (
          <RubyPreview
            text={editor.content}
            vertical={editor.vertical}
            fontFamily={FONT_FAMILY_VARIABLE[fontStyle]}
            fontSize={fontSize}
            lineHeight={lineHeight}
          />
        ) : (
          <textarea
            className={`editor-pane__textarea${editor.vertical ? " editor-pane__textarea--vertical" : ""}`}
            style={{ fontFamily: FONT_FAMILY_VARIABLE[fontStyle], fontSize, lineHeight }}
            value={editor.content}
            onChange={(event) => editor.onContentChange(event.target.value)}
            onKeyDown={handleKeyDown}
            spellCheck={false}
            aria-label={currentPath}
          />
        )}
      </div>

      <StatusBar
        text={editor.content}
        targetChars={entry?.target_chars ?? null}
        status={editor.status}
        errorMessage={editor.errorMessage}
      />
    </div>
  );
}
