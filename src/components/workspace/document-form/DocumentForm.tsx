import "./DocumentForm.css";
import type { EditableDocument } from "../../../api/types";
import { ChapterForm } from "./ChapterForm";
import { CharacterForm } from "./CharacterForm";

interface DocumentFormProps {
  document: EditableDocument;
  onChange: (document: EditableDocument) => void;
}

/** 項目に分けて開いた文書（人物資料・章立て）の、本文の上に置く項目のフォーム。それ以外の文書には何も出さない。 */
export function DocumentForm({ document, onChange }: DocumentFormProps) {
  switch (document.kind) {
    case "character":
      return (
        <CharacterForm meta={document.meta} onChange={(meta) => onChange({ ...document, meta })} />
      );
    case "chapter":
      return (
        <ChapterForm meta={document.meta} onChange={(meta) => onChange({ ...document, meta })} />
      );
    case "text":
      return null;
  }
}

/** 本文の入力欄の上に出す見出し。項目に分けない文書（ファイル全体を直接編集する）には付けない。 */
export function bodyHeading(document: EditableDocument): string | null {
  switch (document.kind) {
    case "character":
      return "詳細";
    case "chapter":
      return "ストーリーライン";
    case "text":
      return null;
  }
}
