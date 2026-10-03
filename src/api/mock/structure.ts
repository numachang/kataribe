import { BackendError } from "../backend";
import type {
  CharacterMeta,
  FileChange,
  NewScenePlan,
  SceneReference,
  StructureEdit,
  StructurePlan,
} from "../types";
import { trashChange, writeChange } from "./fileChange";
import { refersTo } from "./names";
import {
  chapterPath,
  characterPath,
  isValidSlug,
  isValidWorldDocumentName,
  scenePath,
  worldDocumentNameFromPath,
  worldDocumentPath,
} from "./paths";
import { renderChapterFile, renderCharacterFile } from "./render";
import type { MockChapter, MockScene, ProjectState } from "./state";
import { findChapter, findCharacter, findWorldDocument, worldDocumentTitle } from "./state";

// 構成の操作（人物・世界観の資料・シーンの追加と削除）の変更案を作る。kataribe-engine の `structure/` と同じ形の
// 変更案（Write / Trash）と、削除の確認に見せる材料（参照・注意書き）を返す。状態は書き換えない。
// 本物と違い、かなはローマ字にしない（変換表を二重に持たないため）。ID の提案は英数字だけで作る。

const SLUG_MAX_LENGTH = 48;
const CHARACTER_ID_FALLBACK = "character";
const WORLD_DOCUMENT_NAME_FALLBACK = "doc";

function invalidInput(message: string): BackendError {
  return new BackendError("invalid_input", message);
}

function notFound(message: string): BackendError {
  return new BackendError("not_found", message);
}

function nonEmpty(text: string | null | undefined): string | null {
  const trimmed = text?.trim() ?? "";
  return trimmed === "" ? null : trimmed;
}

// ---- 名前（slug）の決め方 ----

/** 英数字以外の並びをハイフン 1 つにした小文字の slug。作れなければ空文字。 */
function slugify(hint: string): string {
  const slug = hint
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug.slice(0, SLUG_MAX_LENGTH).replace(/-+$/, "");
}

/** 使用済みの名前と重ならないよう、`-2`, `-3`, … を付ける。 */
function avoidTaken(slug: string, taken: Set<string>): string {
  if (!taken.has(slug)) {
    return slug;
  }
  for (let suffix = 2; ; suffix += 1) {
    const candidate = `${slug.slice(0, SLUG_MAX_LENGTH - `-${suffix}`.length)}-${suffix}`;
    if (!taken.has(candidate)) {
      return candidate;
    }
  }
}

/** 予約名（Windows のファイル名）なら `-id` を付けた slug にする。予約名でなければそのまま。 */
function ensureUsableSlug(slug: string): string {
  return isValidSlug(slug) ? slug : `${slug}-id`;
}

/** 人物の ID の案。読み、無ければ名前から、英数字だけの slug を作る（かなは変換しない）。 */
export function suggestMockCharacterId(state: ProjectState, reading: string, name: string): string {
  const hint = [reading, name].map(slugify).find((slug) => slug !== "") ?? "";
  const taken = new Set((state.characters ?? []).map((character) => character.id));
  return avoidTaken(ensureUsableSlug(hint || CHARACTER_ID_FALLBACK), taken);
}

// ---- 人物 ----

/** 今の最大の表示順の次の番号。 */
function nextCharacterOrder(state: ProjectState): number {
  const orders = (state.characters ?? []).map((character) => character.order ?? 0);
  return Math.max(0, ...orders) + 1;
}

function planAddCharacter(
  state: ProjectState,
  requestedId: string | null,
  meta: CharacterMeta,
  body: string,
): StructurePlan {
  const name = meta.name.trim();
  if (name === "") {
    throw invalidInput("人物の名前を入力してください。");
  }
  const id = requestedId ?? suggestMockCharacterId(state, meta.reading ?? "", name);
  if (!isValidSlug(id)) {
    throw invalidInput(
      `ID「${id}」は使えません。小文字の英数字とハイフンだけで、48 文字までにしてください。`,
    );
  }
  if (findCharacter(state, id) !== null) {
    throw invalidInput(`ID「${id}」はもう使われています。`);
  }
  const path = characterPath(id);
  const content = renderCharacterFile({
    id,
    name,
    reading: nonEmpty(meta.reading) ?? "",
    role: meta.role.trim(),
    summary: meta.summary.trim(),
    order: meta.order ?? nextCharacterOrder(state),
    detail: body,
  });
  return {
    ...described(state, `人物「${name}」を追加し`, [writeChange(state, path, content)]),
    created: path,
    references: [],
    notices: [],
  };
}

/** 人物の名前を、視点人物または登場人物として挙げているシーン。 */
function scenesMentioning(state: ProjectState, characterName: string): SceneReference[] {
  const references: SceneReference[] = [];
  for (const chapter of state.chapters ?? []) {
    for (const scene of chapter.scenes ?? []) {
      const asPov = refersTo(characterName, scene.pov);
      const asCharacter = scene.characters.some((name) => refersTo(characterName, name));
      if (asPov || asCharacter) {
        references.push({
          chapter: chapter.id,
          chapter_title: chapter.title,
          scene: scene.id,
          scene_title: scene.title,
          as_pov: asPov,
          as_character: asCharacter,
        });
      }
    }
  }
  return references;
}

function planRemoveCharacter(state: ProjectState, id: string): StructurePlan {
  const character = findCharacter(state, id);
  if (character === null) {
    throw notFound(`人物資料 ${characterPath(id)} がありません。`);
  }
  return {
    ...described(state, `人物「${character.name}」をゴミ箱へ移し`, [
      trashChange(state, characterPath(id)),
    ]),
    created: null,
    references: scenesMentioning(state, character.name),
    notices: [],
  };
}

// ---- 世界観の資料 ----

/** 資料のファイル名。指定があれば検証して使い、無ければ題から作る（英数字だけの題のときだけ。それ以外は `doc`）。 */
function resolveWorldDocumentName(
  state: ProjectState,
  requested: string | null,
  title: string,
): string {
  const specified = nonEmpty(requested);
  if (specified !== null) {
    if (!isValidWorldDocumentName(specified)) {
      throw invalidInput(
        `ファイル名「${specified}」は使えません。小文字の英数字とハイフンだけで、48 文字までにしてください（「overview」も使えません）。`,
      );
    }
    return specified;
  }
  const isAsciiTitle = /^[\x20-\x7e]+$/.test(title);
  const hint = isAsciiTitle ? slugify(title) : "";
  const taken = new Set(state.worldDocuments.map((document) => document.name));
  return avoidTaken(ensureUsableSlug(hint || WORLD_DOCUMENT_NAME_FALLBACK), taken);
}

/** 世界観の資料の中身。題を見出しにし、本文があれば空行を挟んで続ける。 */
function renderWorldDocument(title: string, body: string): string {
  const trimmedBody = body.replace(/^[\r\n]+/, "").trimEnd();
  return trimmedBody === "" ? `# ${title}\n` : `# ${title}\n\n${trimmedBody}\n`;
}

function planAddWorldDocument(
  state: ProjectState,
  requestedName: string | null,
  rawTitle: string,
  body: string,
): StructurePlan {
  const title = rawTitle.trim();
  if (title === "") {
    throw invalidInput("資料の題を入力してください。");
  }
  if (/[\r\n]/.test(title)) {
    throw invalidInput("資料の題は 1 行で入力してください。");
  }
  const name = resolveWorldDocumentName(state, requestedName, title);
  if (findWorldDocument(state, name) !== null) {
    throw invalidInput(`ファイル名「${name}」はもう使われています。`);
  }
  const path = worldDocumentPath(name);
  return {
    ...described(state, `世界観の資料「${title}」を追加し`, [
      writeChange(state, path, renderWorldDocument(title, body)),
    ]),
    created: path,
    references: [],
    notices: [],
  };
}

function planRemoveWorldDocument(state: ProjectState, path: string): StructurePlan {
  const name = worldDocumentNameFromPath(path);
  if (name === null) {
    throw invalidInput(
      `${path} は消せる世界観の資料ではありません（消せるのは、world/ 直下の概要以外の Markdown です）。`,
    );
  }
  const document = findWorldDocument(state, name);
  if (document === null) {
    throw notFound(`世界観の資料 ${path} がありません。`);
  }
  const label = worldDocumentTitle(document);
  return {
    ...described(state, `世界観の資料「${label}」をゴミ箱へ移し`, [trashChange(state, path)]),
    created: null,
    references: [],
    notices: [],
  };
}

// ---- シーン ----

function chapterNumber(chapterId: string): number {
  return Number.parseInt(chapterId, 10);
}

function requireChapter(state: ProjectState, chapterId: string): MockChapter {
  const chapter = findChapter(state, chapterId);
  if (chapter === null) {
    throw notFound(`第${chapterNumber(chapterId)}章（${chapterPath(chapterId)}）がありません。`);
  }
  return chapter;
}

function sceneNumber(sceneId: string): number {
  return Number.parseInt(sceneId.slice(1), 10);
}

/** 使われている番号と重ならない、最小の番号のシーン id（`s01`, `s02`, …）。 */
function nextSceneId(scenes: MockScene[]): string {
  const used = new Set(scenes.map((scene) => sceneNumber(scene.id)));
  let number = 1;
  while (used.has(number)) {
    number += 1;
  }
  return `s${String(number).padStart(2, "0")}`;
}

function toMockScene(id: string, plan: NewScenePlan): MockScene {
  return {
    id,
    title: plan.title.trim(),
    summary: plan.summary.trim(),
    pov: nonEmpty(plan.pov) ?? "",
    characters: plan.characters.map(nonEmpty).filter((name): name is string => name !== null),
    place: nonEmpty(plan.place) ?? "",
    time: nonEmpty(plan.time) ?? "",
    targetChars: plan.target_chars !== null && plan.target_chars > 0 ? plan.target_chars : null,
    beats: null,
    draft: null,
  };
}

/** 章立てを書き直す変更（今の章立てを条件にする）。 */
function rewriteChapter(
  state: ProjectState,
  chapter: MockChapter,
  scenes: MockScene[],
): FileChange {
  return writeChange(state, chapterPath(chapter.id), renderChapterFile({ ...chapter, scenes }));
}

function planAddScene(
  state: ProjectState,
  chapterId: string,
  before: string | null,
  scene: NewScenePlan,
): StructurePlan {
  const title = scene.title.trim();
  if (title === "") {
    throw invalidInput("シーンの題を入力してください。");
  }
  const chapter = requireChapter(state, chapterId);
  const scenes = chapter.scenes ?? [];
  const position =
    before === null ? scenes.length : scenes.findIndex((candidate) => candidate.id === before);
  if (position === -1) {
    throw notFound(`シーン ${before} が第${chapterNumber(chapterId)}章にありません。`);
  }
  const added = toMockScene(nextSceneId(scenes), scene);
  const nextScenes = [...scenes.slice(0, position), added, ...scenes.slice(position)];
  return {
    ...described(state, `第${chapterNumber(chapterId)}章にシーン「${title}」を追加し`, [
      rewriteChapter(state, chapter, nextScenes),
    ]),
    created: chapterPath(chapterId),
    references: [],
    notices: [],
  };
}

function planRemoveScene(state: ProjectState, chapterId: string, sceneId: string): StructurePlan {
  const chapter = requireChapter(state, chapterId);
  const scenes = chapter.scenes ?? [];
  const removed = scenes.find((scene) => scene.id === sceneId);
  if (removed === undefined) {
    throw notFound(`シーン ${sceneId} が第${chapterNumber(chapterId)}章にありません。`);
  }
  const files = [
    rewriteChapter(
      state,
      chapter,
      scenes.filter((scene) => scene.id !== sceneId),
    ),
  ];
  const hasText = removed.draft !== null;
  if (hasText) {
    files.push(trashChange(state, scenePath(chapterId, sceneId)));
  }
  return {
    ...described(
      state,
      `第${chapterNumber(chapterId)}章のシーン「${removed.title}」を削除し`,
      files,
    ),
    created: null,
    references: [],
    notices: [],
  };
}

// ---- 入口 ----

/**
 * 変更の言い回し（「ます」「ました」の前まで）から、適用する前の説明（「〜します。」）と
 * 適用したあとの知らせ（「〜しました。」）を作る（本物の Wording と同じ）。
 */
function described(
  state: ProjectState,
  stem: string,
  files: FileChange[],
): Pick<StructurePlan, "change_set" | "completed_summary"> {
  return {
    change_set: { summary: `${stem}ます。`, files, project_root: state.folder },
    completed_summary: `${stem}ました。`,
  };
}

/** 構成の操作の変更案を作る。状態は書き換えない。 */
export function planMockStructureEdit(state: ProjectState, edit: StructureEdit): StructurePlan {
  switch (edit.kind) {
    case "add_character":
      return planAddCharacter(state, edit.id, edit.meta, edit.body);
    case "remove_character":
      return planRemoveCharacter(state, edit.id);
    case "add_world_document":
      return planAddWorldDocument(state, edit.name, edit.title, edit.body);
    case "remove_world_document":
      return planRemoveWorldDocument(state, edit.path);
    case "add_scene":
      return planAddScene(state, edit.chapter, edit.before, edit.scene);
    case "remove_scene":
      return planRemoveScene(state, edit.chapter, edit.scene);
  }
}
