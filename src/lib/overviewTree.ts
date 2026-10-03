import type { OverviewEntry, ProjectOverview } from "../api/types";

function findInEntries(entries: OverviewEntry[], path: string): OverviewEntry | null {
  for (const entry of entries) {
    if (entry.path === path) {
      return entry;
    }
    const foundInChildren = findInEntries(entry.children, path);
    if (foundInChildren) {
      return foundInChildren;
    }
  }
  return null;
}

/** 目次の中から、指定した相対パスに対応する項目を探す（章の下のシーンなど、入れ子も辿る）。 */
export function findOverviewEntry(
  overview: ProjectOverview | null,
  path: string | null,
): OverviewEntry | null {
  if (!overview || path === null) {
    return null;
  }
  for (const section of overview.sections) {
    const found = findInEntries(section.entries, path);
    if (found) {
      return found;
    }
  }
  return null;
}

/** 本文の節にある、章 `chapterId` の見出し（その下にシーンが並ぶ）。無ければ null。 */
export function findManuscriptChapter(
  overview: ProjectOverview | null,
  chapterId: string,
): OverviewEntry | null {
  const manuscript = overview?.sections.find((section) => section.kind === "manuscript");
  return manuscript?.entries.find((entry) => entry.chapter === chapterId) ?? null;
}
