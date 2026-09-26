import { create } from "zustand";

export type SaveStatus = "clean" | "dirty" | "saving" | "error";

interface EditorState {
  path: string | null;
  content: string;
  /** 読み込み時・前回保存時の内容のハッシュ。次の保存の競合検出に使う。 */
  savedHash: string | null;
  status: SaveStatus;
  errorMessage: string | null;
  rubyPreview: boolean;
  vertical: boolean;

  loadDocument: (path: string, content: string, hash: string) => void;
  updateContent: (content: string) => void;
  markSaving: () => void;
  markSaved: (hash: string) => void;
  markError: (message: string) => void;
  toggleRubyPreview: () => void;
  setVertical: (vertical: boolean) => void;
  reset: () => void;
}

const initialDocumentState = {
  path: null,
  content: "",
  savedHash: null,
  status: "clean" as SaveStatus,
  errorMessage: null,
};

/** 中央エディタで開いている 1 つの文書の、編集中の内容と保存状態。 */
export const useEditorStore = create<EditorState>((set) => ({
  ...initialDocumentState,
  rubyPreview: false,
  vertical: true,

  loadDocument(path, content, hash) {
    set({
      path,
      content,
      savedHash: hash,
      status: "clean",
      errorMessage: null,
      rubyPreview: false,
    });
  },

  updateContent(content) {
    set({ content, status: "dirty" });
  },

  markSaving() {
    set({ status: "saving" });
  },

  markSaved(hash) {
    set({ status: "clean", savedHash: hash, errorMessage: null });
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
