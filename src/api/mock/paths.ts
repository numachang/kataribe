// 作品フォルダ内の相対パスの組み立て。docs/architecture.md §2 のフォルダ構成と一致させる。
// kataribe-project の `layout` モジュールに相当する、偽実装だけで使うもの。

export const MANIFEST_PATH = "kataribe.yaml";
export const CONCEPT_PATH = "concept.md";
export const STYLE_PATH = "style.md";
export const WORLD_OVERVIEW_PATH = "world/overview.md";
export const SYNOPSIS_PATH = "plot/synopsis.md";

export function characterPath(id: string): string {
  return `characters/${id}.md`;
}

export function chapterPath(chapterId: string): string {
  return `plot/chapters/${chapterId}.md`;
}

export function scenePath(chapterId: string, sceneId: string): string {
  return `manuscript/${chapterId}/${sceneId}.txt`;
}
