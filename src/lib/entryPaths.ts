// 目次の項目のパス（作品フォルダからの相対パス）から、項目の種類ごとの識別子を取り出す。

const CHARACTER_PATH_PATTERN = /^characters\/(.+)\.md$/;
const ADDITIONAL_WORLD_DOCUMENT_PATTERN = /^world\/[^/]+\.md$/;
const WORLD_OVERVIEW_PATH = "world/overview.md";

/** `characters/<id>.md` の id。人物資料のパスでなければ null。 */
export function characterIdOfPath(path: string): string | null {
  return CHARACTER_PATH_PATTERN.exec(path)?.[1] ?? null;
}

/** 利用者が足した世界観の資料（`world/` 直下の Markdown。概要は除く）か。 */
export function isAdditionalWorldDocumentPath(path: string): boolean {
  return ADDITIONAL_WORLD_DOCUMENT_PATTERN.test(path) && path !== WORLD_OVERVIEW_PATH;
}
