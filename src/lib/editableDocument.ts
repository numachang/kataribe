import type { EditableDocument } from "../api/types";

/**
 * 文書の本文。文字数・品質チェック・ルビのプレビューは、front matter の項目を含めずに本文だけを対象にする。
 * 項目に分けない文書（text）は、ファイル全体が本文。
 */
export function documentBody(document: EditableDocument): string {
  return document.kind === "text" ? document.content : document.body;
}

/** 本文だけを差し替えた文書。項目（front matter）はそのまま引き継ぐ。 */
export function withDocumentBody(document: EditableDocument, body: string): EditableDocument {
  return document.kind === "text" ? { ...document, content: body } : { ...document, body };
}
