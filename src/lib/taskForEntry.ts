import type { EntryKind, Task } from "../api/types";

const CHARACTER_PATH_PATTERN = /^characters\/(.+)\.md$/;
const SCENE_PATH_PATTERN = /^manuscript\/(.+)\/(.+)\.txt$/;

/**
 * 目次の項目に対応する生成タスクを求める。
 * 章立てファイル自体（chapter）を直接生成するタスクはない（工程タブの「章立て」「シーン構成」から行う）。
 */
export function taskForEntry(path: string, kind: EntryKind): Task | null {
  switch (kind) {
    case "concept":
      return { kind: "concept" };
    case "style":
      return { kind: "style" };
    case "world":
      return { kind: "world" };
    case "synopsis":
      return { kind: "synopsis" };
    case "character": {
      const match = CHARACTER_PATH_PATTERN.exec(path);
      const id = match?.[1];
      return id !== undefined ? { kind: "character", id } : null;
    }
    case "scene": {
      const match = SCENE_PATH_PATTERN.exec(path);
      const chapter = match?.[1];
      const scene = match?.[2];
      return chapter !== undefined && scene !== undefined
        ? { kind: "draft", chapter, scene }
        : null;
    }
    case "manifest":
    case "chapter":
    case "other":
      return null;
  }
}
