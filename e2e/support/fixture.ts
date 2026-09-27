// E2E テスト用の作品フォルダと設定ファイル。利用者の本物の設定に触れないよう、一時フォルダに作る。

import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

export const PROJECT_FOLDER_NAME = "e2e-project";
export const SCENE_TITLE = "雨の夜";
export const SCENE_PATH = "manuscript/01/s01.txt";
export const INITIAL_SCENE_TEXT = "　雨が降っていた。\n";

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
    "title: 第一章",
    "scenes:",
    "  - id: s01",
    `    title: ${SCENE_TITLE}`,
    "    summary: 雨の夜の場面。",
    "---",
    "雨の夜の章。",
    "",
  ].join("\n"),
  [SCENE_PATH]: INITIAL_SCENE_TEXT,
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
