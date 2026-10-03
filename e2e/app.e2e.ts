// 本物のアプリ（WebView2 + Rust のバックエンド）を WebDriver で操作する E2E テスト。
// 実行方法は README の「E2E テスト」を参照。

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { By, Key, until, type WebDriver, type WebElement } from "selenium-webdriver";
import { type AppSession, launchApp } from "./support/appSession";
import {
  CHAPTER_TITLE,
  CHARACTER_BODY,
  CHARACTER_NAME,
  CHARACTER_PATH,
  createFixture,
  type Fixture,
  HAND_WRITTEN_COMMENT,
  INITIAL_SCENE_TEXT,
  PROJECT_FOLDER_NAME,
  SCENE_PATH,
  SCENE_TITLE,
  UNKNOWN_FIELD,
  UNKNOWN_SCENE_FIELD,
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

/** 開いている文書の本文の欄（front matter のある文書では、その後ろだけを出す欄）。 */
async function documentEditor(relativePath: string): Promise<WebElement> {
  return waitForElement(By.css(`textarea[aria-label="${relativePath}"]`));
}

async function sceneEditor(): Promise<WebElement> {
  return documentEditor(SCENE_PATH);
}

/** front matter の項目の入力欄。`scope` を渡すと、その要素の中から探す（シーンのカードなど）。 */
async function fieldInput(label: string, scope = ""): Promise<WebElement> {
  return waitForElement(By.xpath(`${scope}//label[span[normalize-space(.)='${label}']]//input`));
}

async function readProjectFile(relativePath: string): Promise<string> {
  return readFile(path.join(fixture.projectFolder, relativePath), "utf8");
}

/** 文書の欄の末尾に文字を打つ。 */
async function typeAtEnd(element: WebElement, text: string): Promise<void> {
  await app.executeScript(
    "arguments[0].focus(); arguments[0].setSelectionRange(arguments[0].value.length, arguments[0].value.length);",
    element,
  );
  await element.sendKeys(text);
}

async function saveWithShortcut(element: WebElement): Promise<void> {
  await element.sendKeys(Key.chord(Key.CONTROL, "s"));
}

/** ファイルが条件を満たすまで待つ（自動保存や Ctrl+S の保存は非同期なので）。 */
async function waitForFile(
  relativePath: string,
  condition: (content: string) => boolean,
  message: string,
): Promise<string> {
  await app.wait(
    async () => condition(await readProjectFile(relativePath)),
    SAVE_TIMEOUT_MS,
    message,
  );
  return readProjectFile(relativePath);
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
  await typeAtEnd(editor, "　風が吹いた。");
  await saveWithShortcut(editor);

  const saved = await waitForFile(
    SCENE_PATH,
    (content) => content.includes("風が吹いた。"),
    "入力した文章がファイルに保存されませんでした。",
  );
  expect(saved).toBe(`${INITIAL_SCENE_TEXT}　風が吹いた。`);
});

test("設定の「この作品」で変えた生成単位が、作品の kataribe.yaml に保存される", async () => {
  const readManifest = () => readFile(path.join(fixture.projectFolder, "kataribe.yaml"), "utf8");
  await (await waitForElement(By.xpath("//button[normalize-space(.)='設定']"))).click();
  await (
    await waitForElement(By.xpath("//button[@role='tab' and contains(., 'この作品')]"))
  ).click();
  await (await waitForElement(By.css('input[aria-label="生成単位をこの作品で変える"]'))).click();
  const draftUnit = await waitForElement(By.css('select[aria-label="生成単位"]'));
  await draftUnit.findElement(By.css('option[value="scene"]')).click();
  await saveScreenshot("settings-project-scope.png");
  await (await waitForElement(By.xpath("//button[normalize-space(.)='保存']"))).click();

  await app.wait(
    async () => (await readManifest()).includes("draft_unit: scene"),
    SAVE_TIMEOUT_MS,
    "作品の設定が kataribe.yaml に保存されませんでした。",
  );
  expect(await readManifest()).toContain("title: E2E の作品");
});

test("人物資料を開くと、front matter は項目の欄に出て、本文の欄には出ない", async () => {
  await (await waitForElement(buttonWithText(CHARACTER_NAME))).click();

  const body = await documentEditor(CHARACTER_PATH);
  expect(await body.getAttribute("value")).toBe(CHARACTER_BODY);
  expect(await (await fieldInput("名前")).getAttribute("value")).toBe(CHARACTER_NAME);
  expect(await (await fieldInput("役割")).getAttribute("value")).toBe("主人公");
  await saveScreenshot("character-form.png");
});

test("人物資料の本文だけを直して保存すると、手で書いたコメントも含めて front matter が書かれたまま残る", async () => {
  const before = await readProjectFile(CHARACTER_PATH);
  expect(before).toContain(HAND_WRITTEN_COMMENT);
  const body = await documentEditor(CHARACTER_PATH);
  await typeAtEnd(body, "口癖は「なるほど」。");
  await saveWithShortcut(body);

  const saved = await waitForFile(
    CHARACTER_PATH,
    (content) => content.includes("なるほど"),
    "人物資料の本文が保存されませんでした。",
  );
  expect(saved).toBe(`${before}口癖は「なるほど」。`);
});

test("人物資料の名前を項目の欄で直して保存すると、front matter に反映され、手で足した項目も残る", async () => {
  const name = await fieldInput("名前");
  await name.sendKeys(Key.END, "子");
  await saveWithShortcut(name);

  const saved = await waitForFile(
    CHARACTER_PATH,
    (content) => content.includes("name: 霧島 凛子"),
    "項目の欄で直した名前が保存されませんでした。",
  );
  expect(saved).toContain(UNKNOWN_FIELD);
  expect(saved).toContain("口癖は「なるほど」。");
  // 目次の人物名も新しい名前になる
  await waitForElement(buttonWithText("霧島 凛子"));
});

test("章立てを開くと章題とシーンのカードが出て、シーンの項目を直すと手で足した項目も残る", async () => {
  await (await waitForElement(buttonWithText(CHAPTER_TITLE))).click();

  const body = await documentEditor("plot/chapters/01.md");
  expect(await body.getAttribute("value")).toBe("雨の夜の章。\n");
  expect(await (await fieldInput("章題")).getAttribute("value")).toBe(CHAPTER_TITLE);

  const sceneCard = "//details[summary[contains(., 's01')]]";
  await (await waitForElement(By.xpath(`${sceneCard}/summary`))).click();
  expect(await (await fieldInput("タイトル", sceneCard)).getAttribute("value")).toBe(SCENE_TITLE);
  await saveScreenshot("chapter-form.png");

  const place = await fieldInput("場所", sceneCard);
  await place.sendKeys("古い洋館");
  await saveWithShortcut(place);

  const saved = await waitForFile(
    "plot/chapters/01.md",
    (content) => content.includes("place: 古い洋館"),
    "シーンの場所が保存されませんでした。",
  );
  expect(saved).toContain(UNKNOWN_SCENE_FIELD);
  expect(saved).toContain("pov: 霧島 凛");
  expect(saved).toContain("雨の夜の章。");
});
