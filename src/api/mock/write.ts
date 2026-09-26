import { mergeSceneDrafts, parseChapterFile, parseCharacterFile } from "./parse";
import {
  CONCEPT_PATH,
  MANIFEST_PATH,
  STYLE_PATH,
  SYNOPSIS_PATH,
  WORLD_OVERVIEW_PATH,
} from "./paths";
import type { ProjectState } from "./state";
import { findChapter, findCharacter } from "./state";

const CHARACTER_PATH_PATTERN = /^characters\/(.+)\.md$/;
const CHAPTER_PATH_PATTERN = /^plot\/chapters\/(.+)\.md$/;
const SCENE_PATH_PATTERN = /^manuscript\/(.+)\/(.+)\.txt$/;

/**
 * パスに応じて内容を解釈し、状態を書き換えずに新しい状態を返す。
 * 既存のファイルを書き直したときも、新しいファイルを作ったときも同じ経路を通る。
 */
export function writeMockFile(state: ProjectState, path: string, content: string): ProjectState {
  if (path === MANIFEST_PATH) {
    // 作品情報は作成時に固定しており、画面から書式ごと書き換えることは想定していない。
    return state;
  }
  if (path === CONCEPT_PATH) {
    return { ...state, concept: content };
  }
  if (path === STYLE_PATH) {
    return { ...state, style: content };
  }
  if (path === WORLD_OVERVIEW_PATH) {
    return { ...state, world: content };
  }
  if (path === SYNOPSIS_PATH) {
    return { ...state, synopsis: content };
  }

  const characterMatch = CHARACTER_PATH_PATTERN.exec(path);
  if (characterMatch?.[1] !== undefined) {
    const id = characterMatch[1];
    const existing = findCharacter(state, id);
    const order = existing?.order ?? (state.characters?.length ?? 0) + 1;
    const parsed = parseCharacterFile(content, id, order);
    const characters = state.characters ?? [];
    const next = existing
      ? characters.map((character) => (character.id === id ? parsed : character))
      : [...characters, parsed];
    return { ...state, characters: next };
  }

  const chapterMatch = CHAPTER_PATH_PATTERN.exec(path);
  if (chapterMatch?.[1] !== undefined) {
    const id = chapterMatch[1];
    const existing = findChapter(state, id);
    const parsed = parseChapterFile(content, id);
    const scenes = parsed.parsedScenes
      ? mergeSceneDrafts(parsed.parsedScenes, existing?.scenes ?? null)
      : null;
    const chapter = { id: parsed.id, title: parsed.title, storyline: parsed.storyline, scenes };
    const chapters = state.chapters ?? [];
    const next = existing
      ? chapters.map((candidate) => (candidate.id === id ? chapter : candidate))
      : [...chapters, chapter];
    return { ...state, chapters: next };
  }

  const sceneMatch = SCENE_PATH_PATTERN.exec(path);
  if (sceneMatch?.[1] !== undefined && sceneMatch[2] !== undefined) {
    const chapterId = sceneMatch[1];
    const sceneId = sceneMatch[2];
    const chapters = state.chapters ?? [];
    const next = chapters.map((chapter) => {
      if (chapter.id !== chapterId || !chapter.scenes) {
        return chapter;
      }
      return {
        ...chapter,
        scenes: chapter.scenes.map((scene) =>
          scene.id === sceneId ? { ...scene, draft: content } : scene,
        ),
      };
    });
    return { ...state, chapters: next };
  }

  return state;
}
