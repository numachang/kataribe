/** 本文（原稿）のファイルかどうか。ルビのプレビューや品質チェックの対象を本文だけに絞るのに使う。 */
export function isManuscriptFile(path: string | null): boolean {
  return path?.endsWith(".txt") ?? false;
}
