import type { Backend } from "../api/backend";
import type { DocumentFile, EditableDocument } from "../api/types";
import { documentBody } from "../lib/editableDocument";
import { useEditorStore } from "../store/editorStore";

/** 項目に分けず、ファイル全体を文字列として扱う文書。 */
export function textDocument(content: string): EditableDocument {
  return { kind: "text", content };
}

/** 文書のファイル全体の文字列を読む。人物資料・章立てのように項目に分かれて返る文書では失敗する。 */
export async function readText(backend: Backend, path: string): Promise<string> {
  const { document } = await backend.readDocument(path);
  if (document.kind !== "text") {
    throw new Error(`テスト: 「${path}」は項目に分かれた文書（${document.kind}）です`);
  }
  return document.content;
}

/** 文書を読み、エディタのストアに開いた状態にする（画面を介さずに保存ロジックを試すときに使う）。 */
export async function loadIntoEditor(backend: Backend, path: string): Promise<DocumentFile> {
  const file = await backend.readDocument(path);
  useEditorStore.getState().loadDocument(path, file);
  return file;
}

/** エディタで、ファイル全体を直接編集する文書（text）を書き換える。 */
export function editText(content: string): void {
  useEditorStore.getState().updateDocument(textDocument(content));
}

/** エディタが今持っている文書の本文。 */
export function editorBody(): string {
  const { document } = useEditorStore.getState();
  return document === null ? "" : documentBody(document);
}
