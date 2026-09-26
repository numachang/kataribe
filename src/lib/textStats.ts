import type { TextStats } from "../api/types";
import { countGraphemes } from "./graphemes";
import { stripRubyNotation } from "./ruby";

const DIALOGUE_QUOTE_START = /^[「『]/;
const MANUSCRIPT_PAGE_SIZE = 400;

function splitLines(text: string): string[] {
  return text.split(/\r\n|\r|\n/);
}

function isBlank(line: string): boolean {
  return line.trim().length === 0;
}

/** 本文の文字数・段落数・会話行数・原稿用紙換算枚数を数える。 */
export function computeTextStats(text: string): TextStats {
  const withoutRuby = stripRubyNotation(text);
  const withoutWhitespace = withoutRuby.replace(/\s+/gu, "");
  const chars = countGraphemes(withoutWhitespace);

  const lines = splitLines(text).filter((line) => !isBlank(line));
  const paragraphs = lines.length;
  const dialogueLines = lines.filter((line) => DIALOGUE_QUOTE_START.test(line.trim())).length;
  const manuscriptPages = Math.round((chars / MANUSCRIPT_PAGE_SIZE) * 10) / 10;

  return {
    chars,
    paragraphs,
    dialogue_lines: dialogueLines,
    manuscript_pages: manuscriptPages,
  };
}
