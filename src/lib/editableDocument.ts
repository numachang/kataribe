import type { EditableDocument } from "../api/types";

/**
 * 文書の本文。文字数・品質チェック・ルビのプレビューは、front matter の項目を含めずに本文だけを対象にする。
 * 項目に分けない文書（text）は、ファイル全体が本文。
 */
export function documentBody(document: EditableDocument): string {
  return document.kind === "text" ? document.content : document.body;
}

/**
 * 項目（front matter）が前回から変わったか。項目を持たない文書（text）や、前回が無いときは、変わったものとみなす。
 * 項目は JSON にできる値だけなので、JSON の文字列で比べる。
 */
export function hasChangedMeta(
  previous: EditableDocument | null,
  document: EditableDocument,
): boolean {
  if (previous === null || previous.kind === "text" || document.kind === "text") {
    return true;
  }
  return JSON.stringify(previous.meta) !== JSON.stringify(document.meta);
}

/** 本文だけを差し替えた文書。項目（front matter）はそのまま引き継ぐ。 */
export function withDocumentBody(document: EditableDocument, body: string): EditableDocument {
  return document.kind === "text" ? { ...document, content: body } : { ...document, body };
}
