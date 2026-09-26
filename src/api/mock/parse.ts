import type { MockChapter, MockCharacter, MockScene } from "./state";

// render.ts の逆変換。ユーザーが生の本文（front matter を含む）をそのまま書き換えられるように、
// 保存されたテキストから構造化データを復元する。書式は render.ts が作るものと一致させる。

interface FrontMatterDocument {
  fields: string[];
  body: string;
}

function splitFrontMatter(content: string): FrontMatterDocument | null {
  if (!content.startsWith("---\n")) {
    return null;
  }
  const closingMarker = "\n---\n";
  const closingIndex = content.indexOf(closingMarker, 4);
  if (closingIndex === -1) {
    return null;
  }
  const rawFrontMatter = content.slice(4, closingIndex);
  const body = content.slice(closingIndex + closingMarker.length);
  return { fields: rawFrontMatter.split("\n"), body };
}

function parseSimpleFields(lines: string[]): Map<string, string> {
  const fields = new Map<string, string>();
  for (const line of lines) {
    const match = /^(\w+): (.*)$/.exec(line);
    if (match) {
      const [, key, value] = match;
      if (key !== undefined && value !== undefined) {
        fields.set(key, value);
      }
    }
  }
  return fields;
}

/** characters/<id>.md を解析し、既存の人物情報へマージする。 */
export function parseCharacterFile(content: string, id: string, order: number): MockCharacter {
  const document = splitFrontMatter(content);
  if (!document) {
    return { id, name: id, reading: "", role: "", summary: "", order, detail: content };
  }
  const fields = parseSimpleFields(document.fields);
  return {
    id,
    name: fields.get("name") ?? id,
    reading: fields.get("reading") ?? "",
    role: fields.get("role") ?? "",
    summary: fields.get("summary") ?? "",
    order: fields.get("order") ? Number(fields.get("order")) : order,
    detail: document.body.length > 0 ? document.body : null,
  };
}

function parseCharacterList(value: string): string[] {
  const trimmed = value.trim().replace(/^\[/, "").replace(/\]$/, "");
  if (trimmed.length === 0) {
    return [];
  }
  return trimmed.split(",").map((name) => name.trim());
}

function parseSceneBlocks(lines: string[]): MockScene[] {
  const scenes: MockScene[] = [];
  let current: Partial<MockScene> & { id?: string } = {};

  function flush(): void {
    if (current.id !== undefined) {
      scenes.push({
        id: current.id,
        title: current.title ?? "",
        summary: current.summary ?? "",
        pov: current.pov ?? "",
        characters: current.characters ?? [],
        place: current.place ?? "",
        time: current.time ?? "",
        targetChars: current.targetChars ?? 0,
        beats: null,
        draft: null,
      });
    }
  }

  for (const line of lines) {
    const newSceneMatch = /^ {2}- id: (.+)$/.exec(line);
    if (newSceneMatch?.[1] !== undefined) {
      flush();
      current = { id: newSceneMatch[1] };
      continue;
    }
    const fieldMatch = /^ {4}(\w+): (.*)$/.exec(line);
    if (!fieldMatch) {
      continue;
    }
    const [, key, value] = fieldMatch;
    if (key === undefined || value === undefined) {
      continue;
    }
    if (key === "title") current.title = value;
    else if (key === "summary") current.summary = value;
    else if (key === "pov") current.pov = value;
    else if (key === "place") current.place = value;
    else if (key === "time") current.time = value;
    else if (key === "characters") current.characters = parseCharacterList(value);
    else if (key === "target_chars") current.targetChars = Number(value);
  }
  flush();
  return scenes;
}

/** plot/chapters/<NN>.md を解析する。既存のシーンの draft・beats は呼び出し側でマージすること。 */
export function parseChapterFile(
  content: string,
  id: string,
): Omit<MockChapter, "scenes"> & { parsedScenes: MockScene[] | null } {
  const document = splitFrontMatter(content);
  if (!document) {
    return { id, title: id, storyline: content, parsedScenes: null };
  }
  const scenesIndex = document.fields.indexOf("scenes:");
  const topLevelLines =
    scenesIndex === -1 ? document.fields : document.fields.slice(0, scenesIndex);
  const fields = parseSimpleFields(topLevelLines);
  const parsedScenes =
    scenesIndex === -1 ? null : parseSceneBlocks(document.fields.slice(scenesIndex + 1));
  return {
    id,
    title: fields.get("title") ?? id,
    storyline: document.body,
    parsedScenes,
  };
}

/** 解析されたシーンに、既存の本文・ビートを id で突き合わせて引き継ぐ。 */
export function mergeSceneDrafts(
  parsedScenes: MockScene[],
  previousScenes: MockScene[] | null,
): MockScene[] {
  return parsedScenes.map((scene) => {
    const previous = previousScenes?.find((candidate) => candidate.id === scene.id);
    return previous ? { ...scene, draft: previous.draft, beats: previous.beats } : scene;
  });
}
