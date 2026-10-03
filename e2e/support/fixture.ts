// E2E テスト用の作品フォルダと設定ファイル。利用者の本物の設定に触れないよう、一時フォルダに作る。

import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

export const PROJECT_FOLDER_NAME = "e2e-project";
export const SCENE_TITLE = "雨の夜";
export const SCENE_PATH = "manuscript/01/s01.txt";
export const INITIAL_SCENE_TEXT = "　雨が降っていた。\n";
/** 本文のある 2 つ目のシーン。削除したとき、本文がゴミ箱へ移ることを確かめるのに使う。 */
export const SECOND_SCENE_TITLE = "雨上がり";
export const SECOND_SCENE_PATH = "manuscript/01/s02.txt";
export const SECOND_SCENE_TEXT = "　雨が上がった。\n";
export const CHAPTER_TITLE = "第一章";
export const CHARACTER_NAME = "霧島 凛";
export const CHARACTER_PATH = "characters/kirishima-rin.md";
/** 人物資料の本文（front matter より後ろ）。 */
export const CHARACTER_BODY = "## 口調\n一人称は「わたし」。\n";
/** 手で書いたコメントと、アプリが知らない項目。保存し直しても失われないことを確かめる。 */
export const HAND_WRITTEN_COMMENT = "# 手で書いたコメント";
export const UNKNOWN_FIELD = "memo: 手で足した項目";
export const UNKNOWN_SCENE_FIELD = "mood: 静か";

export interface Fixture {
  root: string;
  projectFolder: string;
  settingsPath: string;
}

const PROJECT_FILES: Record<string, string> = {
  "kataribe.yaml": [
    "format: 1",
    "title: E2E の作品",
    "genre: general",
    "rating: general",
    "target_length: 3000",
    "idea: 動作確認のための作品。",
    "",
  ].join("\n"),
  "style.md": "# 文体ガイド\n",
  "plot/chapters/01.md": [
    "---",
    `title: ${CHAPTER_TITLE}`,
    "scenes:",
    "  - id: s01",
    `    title: ${SCENE_TITLE}`,
    "    summary: 雨の夜の場面。",
    "    pov: 霧島 凛",
    "    characters: [霧島 凛]",
    `    ${UNKNOWN_SCENE_FIELD}`,
    "  - id: s02",
    `    title: ${SECOND_SCENE_TITLE}`,
    "    summary: 雨が上がった朝の場面。",
    "---",
    "雨の夜の章。",
    "",
  ].join("\n"),
  [SCENE_PATH]: INITIAL_SCENE_TEXT,
  [SECOND_SCENE_PATH]: SECOND_SCENE_TEXT,
  [CHARACTER_PATH]: [
    "---",
    HAND_WRITTEN_COMMENT,
    `name: ${CHARACTER_NAME}`,
    "role: 主人公",
    "summary: 盲目の少女探偵。",
    UNKNOWN_FIELD,
    "---",
    CHARACTER_BODY,
  ].join("\n"),
};

/** 作品フォルダと、それを「最近の作品」に入れ、縦書きを既定にした設定ファイルを作る。 */
export async function createFixture(): Promise<Fixture> {
  const root = await mkdtemp(path.join(tmpdir(), "kataribe-e2e-"));
  const projectFolder = path.join(root, PROJECT_FOLDER_NAME);
  for (const [relativePath, content] of Object.entries(PROJECT_FILES)) {
    const filePath = path.join(projectFolder, relativePath);
    await mkdir(path.dirname(filePath), { recursive: true });
    await writeFile(filePath, content, "utf8");
  }
  const settingsPath = path.join(root, "settings.json");
  const settings = { editor: { vertical: true }, recent_projects: [projectFolder] };
  await writeFile(settingsPath, JSON.stringify(settings), "utf8");
  return { root, projectFolder, settingsPath };
}
