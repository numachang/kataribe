// 本物のアプリ（WebView2 + Rust のバックエンド）を WebDriver で操作する E2E テスト。
// 実行方法は README の「E2E テスト」を参照。

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { By, Key, until, type WebDriver, type WebElement } from "selenium-webdriver";
import { type AppSession, launchApp } from "./support/appSession";
import {
  createFixture,
  type Fixture,
  INITIAL_SCENE_TEXT,
  PROJECT_FOLDER_NAME,
  SCENE_PATH,
  SCENE_TITLE,
} from "./support/fixture";

const UI_TIMEOUT_MS = 10_000;
const SAVE_TIMEOUT_MS = 5_000;
const ARTIFACTS_FOLDER = "e2e/artifacts";

let fixture: Fixture;
let session: AppSession;
let app: WebDriver;

beforeAll(async () => {
  fixture = await createFixture();
  session = await launchApp(["--settings", fixture.settingsPath]);
  app = session.app;
});

afterAll(async () => {
  await session?.close();
  if (fixture) {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

function buttonWithText(text: string): By {
  return By.xpath(`//button[contains(., '${text}')]`);
}

async function waitForElement(locator: By): Promise<WebElement> {
  const element = await app.wait(until.elementLocated(locator), UI_TIMEOUT_MS);
  await app.wait(until.elementIsVisible(element), UI_TIMEOUT_MS);
  return element;
}

async function sceneEditor(): Promise<WebElement> {
  return waitForElement(By.css(`textarea[aria-label="${SCENE_PATH}"]`));
}

async function readSceneFile(): Promise<string> {
  return readFile(path.join(fixture.projectFolder, SCENE_PATH), "utf8");
}

async function saveScreenshot(name: string): Promise<void> {
  await mkdir(ARTIFACTS_FOLDER, { recursive: true });
  await writeFile(path.join(ARTIFACTS_FOLDER, name), await app.takeScreenshot(), "base64");
}

test("開始画面の「最近の作品」から作品を開き、シーンを選ぶとエディタに本文が出る", async () => {
  await (await waitForElement(buttonWithText(PROJECT_FOLDER_NAME))).click();
  await (await waitForElement(buttonWithText(SCENE_TITLE))).click();

  const editor = await sceneEditor();
  expect(await editor.getAttribute("value")).toBe(INITIAL_SCENE_TEXT);
});

test("縦書きの設定では、エディタの textarea が WebView2 で縦に組まれる", async () => {
  const editor = await sceneEditor();
  expect(await editor.getAttribute("class")).toContain("editor-pane__textarea--vertical");

  // 同じ見た目の textarea に長い文章を入れ、あふれる向きで組み方向を確かめる。
  // 縦書き（vertical-rl）なら行は右から左へ並び、横方向にあふれて縦方向にはあふれない。
  const layout = await app.executeScript<{
    writingMode: string;
    overflowsHorizontally: boolean;
    overflowsVertically: boolean;
  }>(
    `const editor = arguments[0];
     const probe = editor.cloneNode();
     probe.value = "縦書きの行。\\n".repeat(300);
     editor.parentElement.appendChild(probe);
     const result = {
       writingMode: getComputedStyle(probe).writingMode,
       overflowsHorizontally: probe.scrollWidth > probe.clientWidth,
       overflowsVertically: probe.scrollHeight > probe.clientHeight + 1,
     };
     probe.remove();
     return result;`,
    editor,
  );

  expect(layout).toEqual({
    writingMode: "vertical-rl",
    overflowsHorizontally: true,
    overflowsVertically: false,
  });
  await saveScreenshot("workspace-vertical.png");
});

test("縦書きのまま入力した文章が、Ctrl+S で作品フォルダのファイルに保存される", async () => {
  const editor = await sceneEditor();
  await app.executeScript(
    "arguments[0].focus(); arguments[0].setSelectionRange(arguments[0].value.length, arguments[0].value.length);",
    editor,
  );
  await editor.sendKeys("　風が吹いた。");
  await editor.sendKeys(Key.chord(Key.CONTROL, "s"));

  await app.wait(
    async () => (await readSceneFile()).includes("風が吹いた。"),
    SAVE_TIMEOUT_MS,
    "入力した文章がファイルに保存されませんでした。",
  );
  expect(await readSceneFile()).toBe(`${INITIAL_SCENE_TEXT}　風が吹いた。`);
});
