import type { Manifest, ProjectSettings } from "../types";

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
  /** 表示順。null なら順番を決めていない（目次では最後に並ぶ）。 */
  order: number | null;
  /** 「外見」「口調」などの本文。null なら Cast 工程が作った骨組みのまま（未生成）。空文字は、利用者が空にした本文で、未生成ではない。 */
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

/** world/ 直下に足した、概要以外の世界観の資料。 */
export interface MockWorldDocument {
  /** ファイル名（拡張子なし）。 */
  name: string;
  /** ファイルの全文（先頭の見出しが題）。 */
  content: string;
}

export interface ProjectState {
  folder: string;
  manifest: Manifest;
  /** 作品ごとの設定（kataribe.yaml の settings）。 */
  settings: ProjectSettings;
  concept: string | null;
  style: string | null;
  world: string | null;
  /** 世界観の概要（world/overview.md）以外の資料。 */
  worldDocuments: MockWorldDocument[];
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

/** 章を番号の小さい順に並べる（章の順序は番号で決まるので、足したり改名したりしたあとも保つ）。 */
export function sortedByNumber(chapters: MockChapter[]): MockChapter[] {
  return [...chapters].sort(
    (left, right) => Number.parseInt(left.id, 10) - Number.parseInt(right.id, 10),
  );
}

/**
 * 人物を目次に並べる順（表示順、無ければ最後）。同じ値は、本物の目次の `sort_for_display` と同じく、読み込んだ順のまま
 * （＝ファイル名の順。`rin-a.md` は `rin.md` より前になるので、id の順とは限らない）。
 */
export function sortedForDisplay(characters: MockCharacter[]): MockCharacter[] {
  const orderOf = (character: MockCharacter): number => character.order ?? Number.MAX_SAFE_INTEGER;
  const fileNameOf = (character: MockCharacter): string => `${character.id}.md`;
  return [...characters].sort((left, right) => {
    const byOrder = orderOf(left) - orderOf(right);
    if (byOrder !== 0) {
      return byOrder;
    }
    // localeCompare ではなく、ファイルシステムが並べる順（コードポイント順）にそろえる
    const leftName = fileNameOf(left);
    const rightName = fileNameOf(right);
    return leftName < rightName ? -1 : leftName > rightName ? 1 : 0;
  });
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

export function findWorldDocument(state: ProjectState, name: string): MockWorldDocument | null {
  return state.worldDocuments.find((document) => document.name === name) ?? null;
}

/** 世界観の資料の題。先頭の見出し（`# 題`）、無ければファイル名。 */
export function worldDocumentTitle(document: MockWorldDocument): string {
  const heading = /^#\s+(.+)$/m.exec(document.content)?.[1]?.trim();
  return heading ?? document.name;
}
