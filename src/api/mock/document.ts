import { BackendError } from "../backend";
import type { ChapterMeta, CharacterMeta, EditableDocument, ScenePlan } from "../types";
import { mergeSceneDrafts } from "./parse";
import { chapterIdFromPath, characterIdFromPath } from "./paths";
import { characterDetailFromBody, PLACEHOLDER_CHARACTER_BODY, readMockFile } from "./render";
import type { MockChapter, MockCharacter, MockScene, ProjectState } from "./state";
import { findChapter, findCharacter } from "./state";
import { writeMockFile } from "./write";

// 偽バックエンドの文書の読み書き。人物資料・章立ては、状態（MockCharacter / MockChapter）から
// 項目に分けて返し、保存も状態を直接更新する（Markdown の文字列を経由しない）。
// 状態では空文字で持つ項目（読み・視点・場所・時間）は、「無い」と空文字を区別せず、空文字を null で返す。
// 本物は front matter の `reading: ""` を空文字のまま返すが、偽の front matter の書式はそこまで再現しない。

function emptyToNull(value: string): string | null {
  return value === "" ? null : value;
}

function toCharacterMeta(character: MockCharacter): CharacterMeta {
  return {
    name: character.name,
    reading: emptyToNull(character.reading),
    role: character.role,
    summary: character.summary,
    order: character.order,
  };
}

function toScenePlan(scene: MockScene): ScenePlan {
  const plan: ScenePlan = {
    id: scene.id,
    title: scene.title,
    summary: scene.summary,
    pov: emptyToNull(scene.pov),
    place: emptyToNull(scene.place),
    time: emptyToNull(scene.time),
    target_chars: scene.targetChars,
  };
  // 本物は、空の characters と beats を省く
  if (scene.characters.length > 0) {
    plan.characters = scene.characters;
  }
  if (scene.beats !== null && scene.beats.length > 0) {
    plan.beats = scene.beats;
  }
  return plan;
}

function toChapterMeta(chapter: MockChapter): ChapterMeta {
  const scenes = chapter.scenes ?? [];
  return scenes.length > 0
    ? { title: chapter.title, scenes: scenes.map(toScenePlan) }
    : { title: chapter.title };
}

/** パスに対応する文書を、画面で編集する形で読む。存在しなければ null。 */
export function readMockDocument(state: ProjectState, path: string): EditableDocument | null {
  const characterId = characterIdFromPath(path);
  const character = characterId === null ? null : findCharacter(state, characterId);
  if (character) {
    return {
      kind: "character",
      meta: toCharacterMeta(character),
      body: character.detail ?? PLACEHOLDER_CHARACTER_BODY,
    };
  }
  const chapterId = chapterIdFromPath(path);
  const chapter = chapterId === null ? null : findChapter(state, chapterId);
  if (chapter) {
    return { kind: "chapter", meta: toChapterMeta(chapter), body: chapter.storyline };
  }
  const content = readMockFile(state, path);
  return content === null ? null : { kind: "text", content };
}

function writeCharacter(
  state: ProjectState,
  id: string,
  meta: CharacterMeta,
  body: string,
): ProjectState {
  const character: MockCharacter = {
    id,
    name: meta.name,
    reading: meta.reading ?? "",
    role: meta.role,
    summary: meta.summary,
    order: meta.order ?? null,
    detail: characterDetailFromBody(body),
  };
  const characters = state.characters ?? [];
  const exists = findCharacter(state, id) !== null;
  return {
    ...state,
    characters: exists
      ? characters.map((candidate) => (candidate.id === id ? character : candidate))
      : [...characters, character],
  };
}

/** 本文（draft）とビートは画面が持たないので、あとで同じ id の保存済みのシーンから引き継ぐ。 */
function toMockScene(plan: ScenePlan): MockScene {
  return {
    id: plan.id,
    title: plan.title,
    summary: plan.summary,
    pov: plan.pov ?? "",
    characters: plan.characters ?? [],
    place: plan.place ?? "",
    time: plan.time ?? "",
    targetChars: plan.target_chars ?? null,
    beats: null,
    draft: null,
  };
}

function writeChapter(
  state: ProjectState,
  id: string,
  meta: ChapterMeta,
  body: string,
): ProjectState {
  const existing = findChapter(state, id);
  const plans = meta.scenes ?? [];
  const previousScenes = existing?.scenes ?? null;
  // シーンがまだ無い章（scenes が null）に空の一覧を保存しても、null のままにする
  const scenes =
    plans.length === 0 && previousScenes === null
      ? null
      : mergeSceneDrafts(plans.map(toMockScene), previousScenes);
  const chapter: MockChapter = { id, title: meta.title, storyline: body, scenes };
  const chapters = state.chapters ?? [];
  return {
    ...state,
    chapters: existing
      ? chapters.map((candidate) => (candidate.id === id ? chapter : candidate))
      : [...chapters, chapter],
  };
}

function kindMismatch(path: string, document: EditableDocument): BackendError {
  const label = document.kind === "character" ? "人物資料" : "章立て";
  return new BackendError("invalid_input", `「${path}」には${label}を保存できません。`);
}

/** 文書を状態へ書き込み、書き換えずに新しい状態を返す。パスの種類に合わない文書は invalid_input。 */
export function writeMockDocument(
  state: ProjectState,
  path: string,
  document: EditableDocument,
): ProjectState {
  if (document.kind === "text") {
    return writeMockFile(state, path, document.content);
  }
  if (document.kind === "character") {
    const id = characterIdFromPath(path);
    if (id === null) {
      throw kindMismatch(path, document);
    }
    return writeCharacter(state, id, document.meta, document.body);
  }
  const id = chapterIdFromPath(path);
  if (id === null) {
    throw kindMismatch(path, document);
  }
  return writeChapter(state, id, document.meta, document.body);
}
