import { beforeEach, describe, expect, it } from "vitest";
import type { DocumentFile, EditableDocument } from "../api/types";
import { useEditorStore } from "./editorStore";

function reset(): void {
  useEditorStore.getState().reset();
  useEditorStore.setState({ vertical: true });
}

function textFile(content: string, hash: string): DocumentFile {
  return { document: { kind: "text", content }, hash, parse_error: null };
}

describe("useEditorStore", () => {
  beforeEach(reset);

  it("loadDocument は文書を反映し、保存状態を clean にする", () => {
    useEditorStore.getState().loadDocument("concept.md", textFile("企画本文", "hash-1"));

    const state = useEditorStore.getState();
    expect(state.path).toBe("concept.md");
    expect(state.document).toEqual({ kind: "text", content: "企画本文" });
    expect(state.parseError).toBeNull();
    expect(state.savedHash).toBe("hash-1");
    expect(state.status).toBe("clean");
  });

  it("loadDocument は、front matter を読めなかった理由を保持する", () => {
    useEditorStore.getState().loadDocument("characters/rin.md", {
      ...textFile("---\nname: [\n---\n", "hash-1"),
      parse_error: "characters/rin.md の 2 行目を読めません",
    });

    expect(useEditorStore.getState().parseError).toBe("characters/rin.md の 2 行目を読めません");
  });

  it("updateDocument は dirty にして、版番号を進める", () => {
    useEditorStore.getState().loadDocument("concept.md", textFile("企画本文", "hash-1"));
    const revisionAfterLoad = useEditorStore.getState().revision;

    useEditorStore.getState().updateDocument({ kind: "text", content: "書き換えた本文" });

    const state = useEditorStore.getState();
    expect(state.document).toEqual({ kind: "text", content: "書き換えた本文" });
    expect(state.status).toBe("dirty");
    expect(state.revision).toBeGreaterThan(revisionAfterLoad);
  });

  it("版番号は、読み込み直しても戻らない（古い保存の完了を、今の文書のものと取り違えないため）", () => {
    useEditorStore.getState().loadDocument("concept.md", textFile("本文", "hash-1"));
    useEditorStore.getState().updateDocument({ kind: "text", content: "本文2" });
    const revisionBeforeReload = useEditorStore.getState().revision;

    useEditorStore.getState().loadDocument("concept.md", textFile("外で書き換えた本文", "hash-2"));

    expect(useEditorStore.getState().revision).toBeGreaterThan(revisionBeforeReload);
  });

  it("markSaving → markSaved で clean に戻り、新しいハッシュと保存した文書を保持する", () => {
    useEditorStore.getState().loadDocument("concept.md", textFile("本文", "hash-1"));
    const edited: EditableDocument = { kind: "text", content: "本文2" };
    useEditorStore.getState().updateDocument(edited);
    useEditorStore.getState().markSaving();
    expect(useEditorStore.getState().status).toBe("saving");

    useEditorStore.getState().markSaved("hash-2", edited);
    expect(useEditorStore.getState().status).toBe("clean");
    expect(useEditorStore.getState().savedHash).toBe("hash-2");
    expect(useEditorStore.getState().savedDocument).toEqual(edited);
  });

  it("recordSaved は dirty のまま、保存した内容の基準だけ更新する", () => {
    useEditorStore.getState().loadDocument("concept.md", textFile("本文", "hash-1"));
    const saved: EditableDocument = { kind: "text", content: "本文2" };
    useEditorStore.getState().updateDocument(saved);
    useEditorStore.getState().updateDocument({ kind: "text", content: "本文3" });

    useEditorStore.getState().recordSaved("hash-2", saved);
    expect(useEditorStore.getState().status).toBe("dirty");
    expect(useEditorStore.getState().savedHash).toBe("hash-2");
    expect(useEditorStore.getState().savedDocument).toEqual(saved);
  });

  it("markError はエラーメッセージを保持する", () => {
    useEditorStore.getState().markError("外部で変更されています");
    expect(useEditorStore.getState().status).toBe("error");
    expect(useEditorStore.getState().errorMessage).toBe("外部で変更されています");
  });

  it("toggleRubyPreview は真偽を反転させる", () => {
    expect(useEditorStore.getState().rubyPreview).toBe(false);
    useEditorStore.getState().toggleRubyPreview();
    expect(useEditorStore.getState().rubyPreview).toBe(true);
  });

  it("新しい文書を開くと rubyPreview は false に戻る", () => {
    useEditorStore.getState().toggleRubyPreview();
    useEditorStore.getState().loadDocument("manuscript/01/s02.txt", textFile("本文", "hash-3"));
    expect(useEditorStore.getState().rubyPreview).toBe(false);
  });

  it("setVertical は文書を切り替えても保持される", () => {
    useEditorStore.getState().setVertical(false);
    useEditorStore.getState().loadDocument("concept.md", textFile("本文", "hash-1"));
    expect(useEditorStore.getState().vertical).toBe(false);
  });
});
