// 書記素（見た目の 1 文字）単位で文字列を数える・分割するための補助関数。
// 結合文字や絵文字も 1 文字として数えるため、`string.length`（UTF-16 コード単位）は使わない。

const segmenter = new Intl.Segmenter("ja", { granularity: "grapheme" });

/** 書記素の配列に分割する。 */
export function toGraphemes(text: string): string[] {
  return Array.from(segmenter.segment(text), (part) => part.segment);
}

/** 書記素単位の文字数を数える。 */
export function countGraphemes(text: string): number {
  let count = 0;
  for (const _part of segmenter.segment(text)) {
    count += 1;
  }
  return count;
}
