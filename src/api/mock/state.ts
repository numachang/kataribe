import type { Manifest } from "../types";

// 偽バックエンドがメモリ上に持つ「作品」の内部表現。
// 本物の作品フォルダと違い、Markdown のテキストではなく構造化されたデータとして持ち、
// ファイルの内容はこの状態から `render.ts` が組み立てる（読み書きのたびに文字列を
// パースし直さずに済むようにするため）。

export interface MockCharacter {
  id: string;
  name: string;
  reading: string;
  role: string;
  summary: string;
  order: number;
  /** 「外見」「口調」などの本文。null なら Cast 工程が作った骨組みのまま。 */
  detail: string | null;
}

export interface MockScene {
  id: string;
  title: string;
  summary: string;
  pov: string;
  characters: string[];
  place: string;
  time: string;
  /** 目安の文字数。null なら目標を定めていないシーン。 */
  targetChars: number | null;
  /** ビート単位の生成に使う展開の一覧。null ならビート分割していない。 */
  beats: string[] | null;
  /** 本文。null ならまだ生成していない。 */
  draft: string | null;
}

export interface MockChapter {
  id: string;
  title: string;
  storyline: string;
  /** シーン構成。null なら ScenePlan 工程がまだ実行されていない。 */
  scenes: MockScene[] | null;
}

export interface ProjectState {
  folder: string;
  manifest: Manifest;
  concept: string | null;
  style: string | null;
  world: string | null;
  /** 登場人物。null なら Cast 工程がまだ実行されていない。 */
  characters: MockCharacter[] | null;
  synopsis: string | null;
  /** 章立て。null なら Outline 工程がまだ実行されていない。 */
  chapters: MockChapter[] | null;
}

export function findCharacter(state: ProjectState, id: string): MockCharacter | null {
  return state.characters?.find((character) => character.id === id) ?? null;
}

export function findChapter(state: ProjectState, chapterId: string): MockChapter | null {
  return state.chapters?.find((chapter) => chapter.id === chapterId) ?? null;
}

export function findScene(
  state: ProjectState,
  chapterId: string,
  sceneId: string,
): MockScene | null {
  return findChapter(state, chapterId)?.scenes?.find((scene) => scene.id === sceneId) ?? null;
}

/** 状態を書き換えず、一部だけ変更した新しい状態を返す。 */
export function withChapter(
  state: ProjectState,
  chapterId: string,
  update: (chapter: MockChapter) => MockChapter,
): ProjectState {
  if (!state.chapters) {
    return state;
  }
  return {
    ...state,
    chapters: state.chapters.map((chapter) =>
      chapter.id === chapterId ? update(chapter) : chapter,
    ),
  };
}

export function withScene(
  state: ProjectState,
  chapterId: string,
  sceneId: string,
  update: (scene: MockScene) => MockScene,
): ProjectState {
  return withChapter(state, chapterId, (chapter) => ({
    ...chapter,
    scenes: chapter.scenes?.map((scene) => (scene.id === sceneId ? update(scene) : scene)) ?? null,
  }));
}

export function withCharacter(
  state: ProjectState,
  id: string,
  update: (character: MockCharacter) => MockCharacter,
): ProjectState {
  if (!state.characters) {
    return state;
  }
  return {
    ...state,
    characters: state.characters.map((character) =>
      character.id === id ? update(character) : character,
    ),
  };
}
