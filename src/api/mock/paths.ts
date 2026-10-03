// 作品フォルダ内の相対パスの組み立て。docs/architecture.md §2 のフォルダ構成と一致させる。
// kataribe-project の `layout` モジュールに相当する、偽実装だけで使うもの。

import { isValidSlug } from "../../lib/slug";

export const MANIFEST_PATH = "kataribe.yaml";
export const CONCEPT_PATH = "concept.md";
export const STYLE_PATH = "style.md";
export const WORLD_OVERVIEW_PATH = "world/overview.md";
export const SYNOPSIS_PATH = "plot/synopsis.md";

export function characterPath(id: string): string {
  return `characters/${id}.md`;
}

export function worldDocumentPath(name: string): string {
  return `world/${name}.md`;
}

export function chapterPath(chapterId: string): string {
  return `plot/chapters/${chapterId}.md`;
}

export function scenePath(chapterId: string, sceneId: string): string {
  return `manuscript/${chapterId}/${sceneId}.txt`;
}

const DRIVE_PREFIX_PATTERN = /^[A-Za-z]:/;

/**
 * 作品フォルダ内の相対パスとして成り立つか。`RelPath::new` のうち、絶対パス・ドライブ指定・`\`・
 * `.` / `..` / 空の区間を拒む部分だけを再現する（偽実装が作品の外を指すパスを受け付けないため）。
 */
export function isProjectRelativePath(path: string): boolean {
  if (path === "" || path.includes("\\") || DRIVE_PREFIX_PATTERN.test(path)) {
    return false;
  }
  return path.split("/").every((segment) => segment !== "" && segment !== "." && segment !== "..");
}

const CHAPTER_ID_PATTERN = /^\d{2,3}$/;
const WORLD_FILE_PATTERN = /^world\/([^/]+)\.md$/;
const CHARACTER_FILE_PATTERN = /^characters\/([^/]+)\.md$/;
const CHAPTER_FILE_PATTERN = /^plot\/chapters\/([^/]+)\.md$/;

/** 世界観の概要（`world/overview.md`）の名前。足す資料には使えない。 */
const WORLD_OVERVIEW_NAME = "overview";

/** 足す世界観の資料のファイル名として使えるか。slug の規則に合い、概要の名前でないこと。 */
export function isValidWorldDocumentName(name: string): boolean {
  return isValidSlug(name) && name !== WORLD_OVERVIEW_NAME;
}

/**
 * `world/<name>.md`（概要以外）なら name、それ以外は null。
 * Windows は大文字小文字を区別しないので、`world/Overview.md` も概要（足した資料ではない）とみなす。
 */
export function worldDocumentNameFromPath(path: string): string | null {
  const name = WORLD_FILE_PATTERN.exec(path)?.[1];
  return name !== undefined && name.toLowerCase() !== WORLD_OVERVIEW_NAME ? name : null;
}

/** `characters/<有効な id>.md` なら id、それ以外は null（`document_kind` が人物資料と判定する条件）。 */
export function characterIdFromPath(path: string): string | null {
  const id = CHARACTER_FILE_PATTERN.exec(path)?.[1];
  return id !== undefined && isValidSlug(id) ? id : null;
}

/** `plot/chapters/<NN または NNN>.md` なら章 id、それ以外は null（`ChapterId::new` と同じ規則）。 */
export function chapterIdFromPath(path: string): string | null {
  const id = CHAPTER_FILE_PATTERN.exec(path)?.[1];
  return id !== undefined && CHAPTER_ID_PATTERN.test(id) ? id : null;
}
