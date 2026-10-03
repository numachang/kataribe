import { create } from "zustand";
import type { DocumentFile, EditableDocument } from "../api/types";

export type SaveStatus = "clean" | "dirty" | "saving" | "error";

interface EditorState {
  path: string | null;
  /** 編集中の文書。何も開いていなければ null。 */
  document: EditableDocument | null;
  /** 人物資料・章立てなのに front matter を読めず、文字列のまま開いたときの理由。 */
  parseError: string | null;
  /**
   * 文書が入れ替わるたびに増える版番号（読み込みと編集の両方で増える。減らさない）。
   * 保存中や書き換えの間に編集されたかを、文書の中身の比較ではなくこの番号の一致で判定する。
   */
  revision: number;
  /** 読み込み時・前回保存時のファイル全体のハッシュ。次の保存の競合検出に使う。 */
  savedHash: string | null;
  status: SaveStatus;
  errorMessage: string | null;
  rubyPreview: boolean;
  vertical: boolean;
  loadDocument: (path: string, file: DocumentFile) => void;
  updateDocument: (document: EditableDocument) => void;
  markSaving: () => void;
  markSaved: (hash: string) => void;
  /** 保存は成功したが、保存中にさらに編集が進んでいるときに使う。dirty のまま基準ハッシュだけ更新する。 */
  recordSavedHash: (hash: string) => void;
  markError: (message: string) => void;
  toggleRubyPreview: () => void;
  setVertical: (vertical: boolean) => void;
  reset: () => void;
}

const initialDocumentState = {
  path: null,
  document: null,
  parseError: null,
  savedHash: null,
  status: "clean" as SaveStatus,
  errorMessage: null,
};

/** 中央エディタで開いている 1 つの文書の、編集中の内容と保存状態。 */
export const useEditorStore = create<EditorState>((set) => ({
  ...initialDocumentState,
  revision: 0,
  rubyPreview: false,
  vertical: true,
  loadDocument(path, file) {
    set((state) => ({
      path,
      document: file.document,
      parseError: file.parse_error,
      revision: state.revision + 1,
      savedHash: file.hash,
      status: "clean",
      errorMessage: null,
      rubyPreview: false,
    }));
  },
  updateDocument(document) {
    set((state) => ({ document, revision: state.revision + 1, status: "dirty" }));
  },
  markSaving() {
    set({ status: "saving" });
  },
  markSaved(hash) {
    set({ status: "clean", savedHash: hash, errorMessage: null });
  },
  recordSavedHash(hash) {
    set({ savedHash: hash });
  },
  markError(message) {
    set({ status: "error", errorMessage: message });
  },
  toggleRubyPreview() {
    set((state) => ({ rubyPreview: !state.rubyPreview }));
  },
  setVertical(vertical) {
    set({ vertical });
  },
  reset() {
    set({ ...initialDocumentState, rubyPreview: false });
  },
}));
