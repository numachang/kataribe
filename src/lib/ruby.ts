import type { Segment } from "../api/types";

// カクヨム・小説家になろう互換のルビ／傍点記法を解析する。
// 対応する記法（docs/architecture.md §3.1）:
//   |親《よみ》 ｜親《よみ》   … 明示的に親文字を指定したルビ
//   漢字《よみ》              … 直前の漢字の連続を親文字とする自動ルビ
//   《《傍点》》              … 傍点
//   |《 ｜《                  … 「《」を記法として解釈させないための逃げ

const EMPHASIS = /《《(?<text>[^》《]*)》》/gu;
const ESCAPE = /[|｜]《/gu;
const EXPLICIT_RUBY = /[|｜](?<base>[^《》\n]+)《(?<reading>[^》《]+)》/gu;
const AUTO_RUBY = /(?<base>[\p{Script=Han}々〆ヶ]+)《(?<reading>[^》《]+)》/gu;

interface Token {
  start: number;
  end: number;
  segment: Segment;
}

function findTokens(
  text: string,
  pattern: RegExp,
  toSegment: (match: RegExpExecArray) => Segment,
): Token[] {
  const tokens: Token[] = [];
  for (const match of text.matchAll(pattern)) {
    if (match.index === undefined) {
      continue;
    }
    tokens.push({
      start: match.index,
      end: match.index + match[0].length,
      segment: toSegment(match),
    });
  }
  return tokens;
}

function requireGroup(match: RegExpExecArray, name: string): string {
  const value = match.groups?.[name];
  if (value === undefined) {
    throw new Error(`ルビ記法の解析に失敗しました（グループ ${name} が見つかりません）`);
  }
  return value;
}

/** 記法をすべて見つけ、開始位置が早い順・同じ位置なら長い一致が勝つ順に並べる。 */
function collectTokens(text: string): Token[] {
  const emphasisTokens = findTokens(text, EMPHASIS, (match) => ({
    kind: "emphasis",
    text: requireGroup(match, "text"),
  }));
  const escapeTokens = findTokens(text, ESCAPE, () => ({ kind: "text", text: "《" }));
  const explicitTokens = findTokens(text, EXPLICIT_RUBY, (match) => ({
    kind: "ruby",
    base: requireGroup(match, "base"),
    reading: requireGroup(match, "reading"),
  }));
  const autoTokens = findTokens(text, AUTO_RUBY, (match) => ({
    kind: "ruby",
    base: requireGroup(match, "base"),
    reading: requireGroup(match, "reading"),
  }));

  const all = [...emphasisTokens, ...escapeTokens, ...explicitTokens, ...autoTokens];
  all.sort((left, right) => left.start - right.start || right.end - left.end);

  const nonOverlapping: Token[] = [];
  let cursor = 0;
  for (const token of all) {
    if (token.start < cursor) {
      continue;
    }
    nonOverlapping.push(token);
    cursor = token.end;
  }
  return nonOverlapping;
}

function mergeAdjacentText(segments: Segment[]): Segment[] {
  const merged: Segment[] = [];
  for (const segment of segments) {
    const previous = merged.at(-1);
    if (segment.kind === "text" && previous?.kind === "text") {
      merged[merged.length - 1] = { kind: "text", text: previous.text + segment.text };
      continue;
    }
    merged.push(segment);
  }
  return merged;
}

/** 本文をルビ・傍点の記法つきセグメントへ分解する。 */
export function parseRubySegments(text: string): Segment[] {
  if (text === "") {
    return [];
  }

  const tokens = collectTokens(text);
  const segments: Segment[] = [];
  let cursor = 0;
  for (const token of tokens) {
    if (token.start > cursor) {
      segments.push({ kind: "text", text: text.slice(cursor, token.start) });
    }
    segments.push(token.segment);
    cursor = token.end;
  }
  if (cursor < text.length) {
    segments.push({ kind: "text", text: text.slice(cursor) });
  }
  return mergeAdjacentText(segments);
}

/** ルビの読み・記法を取り除いた素のテキスト（親文字・傍点の文字は残す）。 */
export function stripRubyNotation(text: string): string {
  return parseRubySegments(text)
    .map((segment) => (segment.kind === "ruby" ? segment.base : segment.text))
    .join("");
}
