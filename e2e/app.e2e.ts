// 本物のアプリ（WebView2 + Rust のバックエンド）を WebDriver で操作する E2E テスト。
// 実行方法は README の「E2E テスト」を参照。

import { access, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
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
  SECOND_CHAPTER_SCENE_PATH,
  SECOND_CHAPTER_SCENE_TEXT,
  SECOND_CHAPTER_SCENE_TITLE,
  SECOND_CHAPTER_SECOND_SCENE_TITLE,
  SECOND_CHAPTER_TITLE,
  SECOND_SCENE_PATH,
  SECOND_SCENE_TEXT,
  SECOND_SCENE_TITLE,
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

/** ファイルが条件を満たすまで待つ（自動保存や Ctrl+S の保存は非同期なので）。まだ無いファイルは、できるまで待つ。 */
async function waitForFile(
  relativePath: string,
  condition: (content: string) => boolean,
  message: string,
): Promise<string> {
  await app.wait(
    async () => {
      const content = await readProjectFile(relativePath).catch(() => null);
      return content !== null && condition(content);
    },
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

/** 開いているダイアログ（見出しの名前で探す）。 */
async function dialogNamed(name: string): Promise<WebElement> {
  return waitForElement(By.css(`[role="dialog"][aria-label="${name}"]`));
}

async function dialogField(dialog: WebElement, label: string, tag = "input"): Promise<WebElement> {
  return dialog.findElement(By.xpath(`.//label[span[normalize-space(.)='${label}']]//${tag}`));
}

async function projectFileExists(relativePath: string): Promise<boolean> {
  return access(path.join(fixture.projectFolder, relativePath)).then(
    () => true,
    () => false,
  );
}

test("目次の「人物を追加」で、かなの読みからローマ字の ID を作り、人物資料ができて目次に出て開かれる", async () => {
  await (await waitForElement(By.css('button[aria-label="人物を追加"]'))).click();
  const dialog = await dialogNamed("人物を追加");

  await (await dialogField(dialog, "名前")).sendKeys("霧島 蓮");
  await (await dialogField(dialog, "読み")).sendKeys("きりしま れん");
  await (await dialogField(dialog, "役割")).sendKeys("助手");
  const idInput = await dialogField(dialog, "ID");
  await app.wait(
    async () => (await idInput.getAttribute("value")) === "kirishima-ren",
    SAVE_TIMEOUT_MS,
    "読みからローマ字の ID の提案が出ませんでした。",
  );
  await saveScreenshot("add-character-dialog.png");
  await (await dialog.findElement(By.xpath(".//button[normalize-space(.)='追加']"))).click();

  const saved = await waitForFile(
    "characters/kirishima-ren.md",
    (content) => content.includes("name: 霧島 蓮"),
    "追加した人物の資料が作られませんでした。",
  );
  expect(saved).toContain("name: 霧島 蓮");
  expect(saved).toContain("reading: きりしま れん");
  expect(saved).toContain("role: 助手");
  // 目次に出て、そのまま開かれる（本文の欄の名前は、開いているファイルのパス）
  await waitForElement(buttonWithText("霧島 蓮"));
  await documentEditor("characters/kirishima-ren.md");
  expect(await (await fieldInput("名前")).getAttribute("value")).toBe("霧島 蓮");
  await saveScreenshot("character-added.png");
});

/** 目次の行の操作メニューを開いて、項目を選ぶ。 */
async function chooseRowMenuItem(menuLabel: string, item: string): Promise<void> {
  await (await waitForElement(By.css(`button[aria-label="${menuLabel}"]`))).click();
  await (
    await waitForElement(By.xpath(`//*[@role='menuitem' and normalize-space(.)='${item}']`))
  ).click();
}

/** 目次の章の呼び方（本文の章見出しと章立ての行に出る）。 */
function chapterLabel(number: number, title: string): string {
  return `第${number}章「${title}」`;
}

test("本文のあるシーンを削除すると、確認で本文も移ることを見せ、本文は .kataribe/trash/ に移り、章立てからも外れる", async () => {
  await chooseRowMenuItem(`「2. ${SECOND_SCENE_TITLE}」の操作`, "削除");

  const dialog = await dialogNamed("削除の確認");
  const emphasis = await waitForElement(By.css(".structure-dialog__emphasis"));
  expect(await emphasis.getText()).toContain("本文 1 ファイル（計 7 字）もゴミ箱へ移ります。");
  await saveScreenshot("remove-scene-confirm.png");
  await (
    await dialog.findElement(By.xpath(".//button[normalize-space(.)='ゴミ箱へ移す']"))
  ).click();

  await app.wait(
    async () => !(await projectFileExists(SECOND_SCENE_PATH)),
    SAVE_TIMEOUT_MS,
    "本文がゴミ箱へ移りませんでした。",
  );
  // ゴミ箱へは、日時のフォルダの下に元のパスのまま移る
  const trashRoot = path.join(fixture.projectFolder, ".kataribe", "trash");
  const [stamp] = await readdir(trashRoot);
  expect(stamp).toBeDefined();
  expect(await readFile(path.join(trashRoot, stamp ?? "", SECOND_SCENE_PATH), "utf8")).toBe(
    SECOND_SCENE_TEXT,
  );
  const chapter = await waitForFile(
    "plot/chapters/01.md",
    (content) => !content.includes(SECOND_SCENE_TITLE),
    "章立てからシーンが外れませんでした。",
  );
  expect(chapter).toContain(`title: ${SCENE_TITLE}`);
  expect(chapter).toContain(UNKNOWN_SCENE_FIELD);
  await app.wait(
    async () =>
      (await app.findElements(By.css(`button[aria-label="「2. ${SECOND_SCENE_TITLE}」の操作"]`)))
        .length === 0,
    UI_TIMEOUT_MS,
    "目次からシーンが消えませんでした。",
  );
  await saveScreenshot("scene-removed.png");
});

const FIRST_SCENE_TEXT_AFTER_EDITS = `${INITIAL_SCENE_TEXT}　風が吹いた。`;

test("2 章目のシーンを開いたまま第 1 章の前に章を追加すると、章立てと本文が振り直され、開いていた本文に続けて入力した分が新しいパスに保存される", async () => {
  await (await waitForElement(buttonWithText(SECOND_CHAPTER_SCENE_TITLE))).click();
  const before = await documentEditor(SECOND_CHAPTER_SCENE_PATH);
  expect(await before.getAttribute("value")).toBe(SECOND_CHAPTER_SCENE_TEXT);

  await chooseRowMenuItem(
    `「${chapterLabel(1, CHAPTER_TITLE)}」の章立ての操作`,
    "この前に章を追加",
  );
  const dialog = await dialogNamed("章を追加");
  await (await dialogField(dialog, "章題")).sendKeys("序章");
  await (await dialogField(dialog, "ストーリーライン", "textarea")).sendKeys("物語の始まり。");
  await saveScreenshot("add-chapter-dialog.png");
  await (await dialog.findElement(By.xpath(".//button[normalize-space(.)='追加']"))).click();

  // 章立ては、新しい章が 01、元の章が 02・03 に振り直される
  const prologue = await waitForFile(
    "plot/chapters/01.md",
    (content) => content.includes("title: 序章"),
    "追加した章の章立てが作られませんでした。",
  );
  expect(prologue).toContain("物語の始まり。");
  expect(await readProjectFile("plot/chapters/02.md")).toContain(`title: ${CHAPTER_TITLE}`);
  expect(await readProjectFile("plot/chapters/03.md")).toContain(`title: ${SECOND_CHAPTER_TITLE}`);
  // 本文のフォルダも、章立てと一緒にフォルダごと移る
  expect(await readProjectFile("manuscript/02/s01.txt")).toBe(FIRST_SCENE_TEXT_AFTER_EDITS);
  expect(await readProjectFile("manuscript/03/s01.txt")).toBe(SECOND_CHAPTER_SCENE_TEXT);
  expect(await projectFileExists("manuscript/01/s01.txt")).toBe(false);

  // 開いていた本文は、読み直されずに新しいパスで開いたまま。続けて入力して保存すると新しいパスへ書かれ、
  // 古いパス（今は第 2 章の本文）は壊れない
  const moved = await documentEditor("manuscript/03/s01.txt");
  expect(await moved.getAttribute("value")).toBe(SECOND_CHAPTER_SCENE_TEXT);
  await typeAtEnd(moved, "　続きを書いた。");
  await saveWithShortcut(moved);
  const saved = await waitForFile(
    "manuscript/03/s01.txt",
    (content) => content.includes("続きを書いた。"),
    "改名のあとに入力した文章が、新しいパスに保存されませんでした。",
  );
  expect(saved).toBe(`${SECOND_CHAPTER_SCENE_TEXT}　続きを書いた。`);
  expect(await readProjectFile("manuscript/02/s01.txt")).toBe(FIRST_SCENE_TEXT_AFTER_EDITS);
  await waitForElement(buttonWithText(chapterLabel(1, "序章")));
  await saveScreenshot("chapter-added.png");
});

test("章を削除すると、確認で番号が変わる章と本文も移ることを見せ、章立てと本文のフォルダが .kataribe/trash/ に移り、後ろの章の番号が詰まる", async () => {
  await chooseRowMenuItem(`「${chapterLabel(2, CHAPTER_TITLE)}」の操作`, "章を削除");

  const dialog = await dialogNamed("削除の確認");
  const emphasis = await waitForElement(By.css(".structure-dialog__emphasis"));
  expect(await emphasis.getText()).toContain("本文 1 ファイル（計 14 字）もゴミ箱へ移ります。");
  const renumbered = await dialog.getText();
  expect(renumbered).toContain("番号が変わる章");
  expect(renumbered).toContain(`${chapterLabel(3, SECOND_CHAPTER_TITLE)} → 第2章`);
  await saveScreenshot("remove-chapter-confirm.png");
  await (
    await dialog.findElement(By.xpath(".//button[normalize-space(.)='ゴミ箱へ移す']"))
  ).click();

  await app.wait(
    async () => !(await projectFileExists("plot/chapters/03.md")),
    SAVE_TIMEOUT_MS,
    "後ろの章の番号が詰まりませんでした。",
  );
  // 章立てと本文のフォルダは、日時のフォルダの下に元のパスのまま移る
  const trashRoot = path.join(fixture.projectFolder, ".kataribe", "trash");
  const trashedStamps = await readdir(trashRoot);
  const trashedChapterFolders = await Promise.all(
    trashedStamps.map((stamp) =>
      readFile(path.join(trashRoot, stamp, "plot/chapters/02.md"), "utf8").then(
        () => path.join(trashRoot, stamp),
        () => null,
      ),
    ),
  );
  const chapterTrash = trashedChapterFolders.find((folder) => folder !== null);
  expect(chapterTrash).toBeDefined();
  expect(await readFile(path.join(chapterTrash ?? "", "plot/chapters/02.md"), "utf8")).toContain(
    `title: ${CHAPTER_TITLE}`,
  );
  expect(await readFile(path.join(chapterTrash ?? "", "manuscript/02/s01.txt"), "utf8")).toBe(
    FIRST_SCENE_TEXT_AFTER_EDITS,
  );
  // 後ろの章（元の第 3 章）が、第 2 章になる
  expect(await readProjectFile("plot/chapters/02.md")).toContain(`title: ${SECOND_CHAPTER_TITLE}`);
  expect(await readProjectFile("manuscript/02/s01.txt")).toBe(
    `${SECOND_CHAPTER_SCENE_TEXT}　続きを書いた。`,
  );
  expect(await projectFileExists("manuscript/03")).toBe(false);
  // 開いていた本文は、番号が詰まった新しいパスで開いたまま
  await documentEditor("manuscript/02/s01.txt");
  await waitForElement(buttonWithText(chapterLabel(2, SECOND_CHAPTER_TITLE)));
  await saveScreenshot("chapter-removed.png");
});

/** 目次の節（見出しの名前で探す）に並ぶ項目の題。本文の節なら章見出しとシーンが、並びの順に出る。 */
async function treeLabels(sectionName: string): Promise<string[]> {
  return app.executeScript<string[]>(
    `const section = Array.from(document.querySelectorAll(".project-tree__section"))
       .find((candidate) => candidate.querySelector("h3")?.textContent === arguments[0]);
     return Array.from(section.querySelectorAll(".project-tree__label")).map((label) => label.textContent);`,
    sectionName,
  );
}

async function waitForTreeLabels(
  sectionName: string,
  expected: string[],
  message: string,
): Promise<void> {
  await app.wait(
    async () => (await treeLabels(sectionName)).join("\n") === expected.join("\n"),
    UI_TIMEOUT_MS,
    message,
  );
}

async function waitForToast(text: string): Promise<void> {
  await waitForElement(
    By.xpath(`//*[contains(@class, 'toast-host__item')][contains(., '${text}')]`),
  );
}

test("人物を「上へ移す」と、確認なしに目次の順が変わり、人物資料の順番（order）が書き換わり、開いていた人物資料の順番の欄も読み直される", async () => {
  // ここまでの人物は、順番の無い「霧島 凛子」と、追加で順番 1 になった「霧島 蓮」（順番の無い人物は目次の最後）
  await waitForTreeLabels("登場人物", ["霧島 蓮", "霧島 凛子"], "人物の初めの並びが違います。");
  await (await waitForElement(buttonWithText("霧島 凛子"))).click();
  await documentEditor(CHARACTER_PATH);

  await chooseRowMenuItem("「霧島 凛子」の操作", "上へ移す");

  await waitForToast("人物「霧島 凛子」を 1 番目に移しました。");
  await waitForTreeLabels(
    "登場人物",
    ["霧島 凛子", "霧島 蓮"],
    "人物の並びが入れ替わりませんでした。",
  );
  const moved = await waitForFile(
    CHARACTER_PATH,
    (content) => content.includes("order: 1"),
    "人物資料の order が書き換わりませんでした。",
  );
  // 書き直しても、手で足した項目と本文は残る
  expect(moved).toContain(UNKNOWN_FIELD);
  expect(moved).toContain("口癖は「なるほど」。");
  expect(await readProjectFile("characters/kirishima-ren.md")).toContain("order: 2");
  // 開いていた人物資料は、書き換えられた順番を読み直して見せる
  await app.wait(
    async () => (await (await fieldInput("順番")).getAttribute("value")) === "1",
    UI_TIMEOUT_MS,
    "開いていた人物資料の順番の欄が読み直されませんでした。",
  );
  await saveScreenshot("character-moved.png");
});

test("シーンを「下へ移す」と、章立ての scenes の順が変わり、本文のファイルはそのまま、開いていた本文も開いたまま", async () => {
  await (await waitForElement(buttonWithText(SECOND_CHAPTER_SCENE_TITLE))).click();
  await documentEditor("manuscript/02/s01.txt");
  const textBefore = await readProjectFile("manuscript/02/s01.txt");
  const chapterBefore = await readProjectFile("plot/chapters/02.md");
  expect(chapterBefore.indexOf("id: s01")).toBeLessThan(chapterBefore.indexOf("id: s02"));

  await chooseRowMenuItem(`「1. ${SECOND_CHAPTER_SCENE_TITLE}」の操作`, "下へ移す");

  await waitForToast(`第2章のシーン「${SECOND_CHAPTER_SCENE_TITLE}」を 2 番目に移しました。`);
  const chapter = await waitForFile(
    "plot/chapters/02.md",
    (content) => content.indexOf("id: s02") < content.indexOf("id: s01"),
    "章立ての scenes の順が変わりませんでした。",
  );
  expect(chapter).toContain(`title: ${SECOND_CHAPTER_SECOND_SCENE_TITLE}`);
  // シーンの id と本文のファイルは変わらない
  expect(await readProjectFile("manuscript/02/s01.txt")).toBe(textBefore);
  expect(await projectFileExists("manuscript/02/s02.txt")).toBe(false);
  await app.wait(
    async () =>
      (await treeLabels("本文")).slice(-2).join("\n") ===
      [`1. ${SECOND_CHAPTER_SECOND_SCENE_TITLE}`, `2. ${SECOND_CHAPTER_SCENE_TITLE}`].join("\n"),
    UI_TIMEOUT_MS,
    "目次のシーンの並びが入れ替わりませんでした。",
  );
  await documentEditor("manuscript/02/s01.txt");
  await saveScreenshot("scene-moved.png");
});

test("開いている本文の章を「下へ移す」と、章立てと本文のフォルダが入れ替わり、開いていた本文に続けて入力した分が新しいパスに保存される", async () => {
  // ここまでの章は、本文の無い第 1 章「序章」と、本文のある第 2 章「第二章」。開いているのは第 2 章の本文
  await documentEditor("manuscript/02/s01.txt");
  const secondChapterText = await readProjectFile("manuscript/02/s01.txt");
  expect(await projectFileExists("manuscript/01")).toBe(false);

  await chooseRowMenuItem(`「${chapterLabel(1, "序章")}」の章立ての操作`, "下へ移す");

  await waitForToast("第1章「序章」を第2章へ移しました。");
  // 章立ては入れ替わり、本文のフォルダは、本文のある第二章に付いて 01 へ移る（本文の無い序章の側には作られない）
  await waitForFile(
    "plot/chapters/01.md",
    (content) => content.includes(`title: ${SECOND_CHAPTER_TITLE}`),
    "章立てが入れ替わりませんでした。",
  );
  expect(await readProjectFile("plot/chapters/02.md")).toContain("title: 序章");
  expect(await readProjectFile("manuscript/01/s01.txt")).toBe(secondChapterText);
  expect(await projectFileExists("manuscript/02")).toBe(false);
  await waitForElement(buttonWithText(chapterLabel(1, SECOND_CHAPTER_TITLE)));
  await waitForElement(buttonWithText(chapterLabel(2, "序章")));

  // 開いていた本文は、読み直されずに新しいパスで開いたまま。続けて入力して保存すると、新しいパスへ書かれる
  const moved = await documentEditor("manuscript/01/s01.txt");
  expect(await moved.getAttribute("value")).toBe(secondChapterText);
  await typeAtEnd(moved, "　入れ替えのあとに書いた。");
  await saveWithShortcut(moved);
  const saved = await waitForFile(
    "manuscript/01/s01.txt",
    (content) => content.includes("入れ替えのあとに書いた。"),
    "入れ替えのあとに入力した文章が、新しいパスに保存されませんでした。",
  );
  expect(saved).toBe(`${secondChapterText}　入れ替えのあとに書いた。`);
  expect(await projectFileExists("manuscript/02")).toBe(false);
  await saveScreenshot("chapter-moved.png");
});
