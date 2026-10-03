// 目次の項目のパス（作品フォルダからの相対パス）から、項目の種類ごとの識別子を取り出す。

import { isValidSlug } from "./slug";

const CHARACTER_DOCUMENT_PATTERN = /^characters\/([^/]+)\.md$/;
const ADDITIONAL_WORLD_DOCUMENT_PATTERN = /^world\/[^/]+\.md$/;
const WORLD_OVERVIEW_PATH = "world/overview.md";

/** `characters/` 直下の Markdown（人物資料）か。ファイル名が人物 ID の規則に合わなくても、目次には出る。 */
export function isCharacterDocumentPath(path: string): boolean {
  return CHARACTER_DOCUMENT_PATTERN.test(path);
}

/** `characters/<id>.md` の id。人物資料のパスでない、または人物 ID の規則に合わない名前なら null。 */
export function characterIdOfPath(path: string): string | null {
  const id = CHARACTER_DOCUMENT_PATTERN.exec(path)?.[1];
  return id !== undefined && isValidSlug(id) ? id : null;
}

/**
 * 利用者が足した世界観の資料（`world/` 直下の Markdown。概要は除く）か。
 * Windows は大文字小文字を区別しないので、`world/Overview.md` も概要として除く。
 */
export function isAdditionalWorldDocumentPath(path: string): boolean {
  return ADDITIONAL_WORLD_DOCUMENT_PATTERN.test(path) && path.toLowerCase() !== WORLD_OVERVIEW_PATH;
}

/** 利用者が足した世界観の資料のパス（`world/<name>.md`）の name。足した資料のパスでなければ null。 */
export function additionalWorldDocumentNameOfPath(path: string): string | null {
  if (!isAdditionalWorldDocumentPath(path)) {
    return null;
  }
  return path.slice("world/".length, -".md".length);
}
