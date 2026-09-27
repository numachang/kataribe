import { beforeEach, describe, expect, it } from "vitest";
import { useEditorStore } from "./editorStore";

function reset(): void {
  useEditorStore.getState().reset();
  useEditorStore.setState({ vertical: true });
}

describe("useEditorStore", () => {
  beforeEach(reset);

  it("loadDocument は内容を反映し、保存状態を clean にする", () => {
    useEditorStore.getState().loadDocument("concept.md", "企画本文", "hash-1");

    const state = useEditorStore.getState();
    expect(state.path).toBe("concept.md");
    expect(state.content).toBe("企画本文");
    expect(state.savedHash).toBe("hash-1");
    expect(state.status).toBe("clean");
  });

  it("updateContent は dirty にする", () => {
    useEditorStore.getState().loadDocument("concept.md", "企画本文", "hash-1");
    useEditorStore.getState().updateContent("書き換えた本文");

    expect(useEditorStore.getState().content).toBe("書き換えた本文");
    expect(useEditorStore.getState().status).toBe("dirty");
  });

  it("markSaving → markSaved で clean に戻り、新しいハッシュを保持する", () => {
    useEditorStore.getState().loadDocument("concept.md", "本文", "hash-1");
    useEditorStore.getState().updateContent("本文2");
    useEditorStore.getState().markSaving();
    expect(useEditorStore.getState().status).toBe("saving");

    useEditorStore.getState().markSaved("hash-2");
    expect(useEditorStore.getState().status).toBe("clean");
    expect(useEditorStore.getState().savedHash).toBe("hash-2");
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
    useEditorStore.getState().loadDocument("manuscript/01/s02.txt", "本文", "hash-3");
    expect(useEditorStore.getState().rubyPreview).toBe(false);
  });

  it("setVertical は文書を切り替えても保持される", () => {
    useEditorStore.getState().setVertical(false);
    useEditorStore.getState().loadDocument("concept.md", "本文", "hash-1");
    expect(useEditorStore.getState().vertical).toBe(false);
  });
});
