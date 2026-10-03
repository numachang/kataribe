import type { Manifest, ProjectSettings } from "../types";
import {
  CONCEPT_PATH,
  chapterPath,
  characterPath,
  MANIFEST_PATH,
  STYLE_PATH,
  SYNOPSIS_PATH,
  scenePath,
  WORLD_OVERVIEW_PATH,
} from "./paths";
import type { MockChapter, MockCharacter, MockScene, ProjectState } from "./state";

// ProjectState の各要素を、実際に作品フォルダへ保存されるテキストへ変換する。
// docs/architecture.md §2 のファイル形式に合わせる（偽実装の見た目を本物に近づけるため）。

function renderFrontMatter(fields: Array<[string, string]>): string {
  const body = fields.map(([key, value]) => `${key}: ${value}`).join("\n");
  return `---\n${body}\n---\n`;
}

export function renderManifest(manifest: Manifest, settings: ProjectSettings = {}): string {
  const idea = manifest.idea
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n");
  const lines = [
    `format: ${manifest.format}`,
    `title: ${manifest.title}`,
    `author: ${manifest.author ?? ""}`,
    `genre: ${manifest.genre}`,
    `genre_note: ${manifest.genre_note ?? ""}`,
    `rating: ${manifest.rating}`,
    `target_length: ${manifest.target_length}`,
    "idea: |",
    idea,
  ];
  const settingLines = Object.entries(settings)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `  ${key}: ${String(value)}`);
  if (settingLines.length > 0) {
    lines.push("settings:", ...settingLines);
  }
  return `${lines.join("\n")}\n`;
}

/** 詳細をまだ生成していない人物の、本文の代わりに書く文。 */
export const PLACEHOLDER_CHARACTER_BODY = "（この人物の詳しい設定はまだ生成していません）\n";

/**
 * 人物資料の本文から、状態の `detail` を決める。プレースホルダーのままの本文は「未生成」（null）に戻し、
 * 保存で本文に化けないようにする。空の本文は未生成ではなく空のまま持つ（本物と同じく、保存した本文がそのまま読み直せる）。
 */
export function characterDetailFromBody(body: string): string | null {
  return body === PLACEHOLDER_CHARACTER_BODY ? null : body;
}

export function renderCharacterFile(character: MockCharacter): string {
  const fields: Array<[string, string]> = [
    ["name", character.name],
    ["reading", character.reading],
    ["role", character.role],
    ["summary", character.summary],
  ];
  // 順番を決めていない人物は order 自体を書かない
  if (character.order !== null) {
    fields.push(["order", String(character.order)]);
  }
  const frontMatter = renderFrontMatter(fields);
  const body = character.detail ?? PLACEHOLDER_CHARACTER_BODY;
  return `${frontMatter}${body}`;
}

function renderSceneFrontMatterLines(scene: MockScene): string[] {
  const lines = [
    `  - id: ${scene.id}`,
    `    title: ${scene.title}`,
    `    summary: ${scene.summary}`,
    `    pov: ${scene.pov}`,
    `    characters: [${scene.characters.join(", ")}]`,
    `    place: ${scene.place}`,
    `    time: ${scene.time}`,
  ];
  // 目標のないシーンは target_chars 自体を書かない（読み込み時は null に復元される）。
  if (scene.targetChars !== null) {
    lines.push(`    target_chars: ${scene.targetChars}`);
  }
  return lines;
}

export function renderChapterFile(chapter: MockChapter): string {
  const scenesBlock = chapter.scenes
    ? ["scenes:", ...chapter.scenes.flatMap((scene) => renderSceneFrontMatterLines(scene))].join(
        "\n",
      )
    : null;
  const frontMatterBody = scenesBlock
    ? `title: ${chapter.title}\n${scenesBlock}`
    : `title: ${chapter.title}`;
  return `---\n${frontMatterBody}\n---\n${chapter.storyline}`;
}

/** 状態を書き換えずに、パスに対応するファイルの内容を読む。存在しなければ null。 */
export function readMockFile(state: ProjectState, path: string): string | null {
  if (path === MANIFEST_PATH) {
    return renderManifest(state.manifest, state.settings);
  }
  if (path === CONCEPT_PATH) {
    return state.concept;
  }
  if (path === STYLE_PATH) {
    return state.style;
  }
  if (path === WORLD_OVERVIEW_PATH) {
    return state.world;
  }
  if (path === SYNOPSIS_PATH) {
    return state.synopsis;
  }

  for (const character of state.characters ?? []) {
    if (characterPath(character.id) === path) {
      return renderCharacterFile(character);
    }
  }
  for (const chapter of state.chapters ?? []) {
    if (chapterPath(chapter.id) === path) {
      return renderChapterFile(chapter);
    }
    for (const scene of chapter.scenes ?? []) {
      if (scenePath(chapter.id, scene.id) === path) {
        return scene.draft;
      }
    }
  }
  return null;
}
