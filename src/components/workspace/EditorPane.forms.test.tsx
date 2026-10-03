import { act, fireEvent, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../api/backend";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import type { DocumentFile, EditableDocument } from "../../api/types";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { textDocument } from "../../test/documents";
import { renderWithBackend } from "../../test/renderWithBackend";
import { resetAllStores } from "../../test/resetStores";
import { wrapBackend } from "../../test/wrapBackend";
import { WorkspaceScreen } from "./WorkspaceScreen";

// 人物資料・章立てを、項目のフォームと本文の欄に分けて編集する画面のテスト。

const CHARACTER_PATH = "characters/kirishima-rin.md";
const CHAPTER_PATH = "plot/chapters/01.md";

beforeEach(resetAllStores);
afterEach(() => {
  resetAllStores();
  vi.useRealTimers();
});

async function openDocument(backend: Backend, path: string): Promise<void> {
  const overview = await backend.openProject(SAMPLE_PROJECT_FOLDER);
  useWorkspaceStore.getState().openWorkspace(overview);
  renderWithBackend(<WorkspaceScreen />, backend);
  act(() => useWorkspaceStore.getState().openDocument(path));
  await screen.findByRole("textbox", { name: path });
}

function bodyTextarea(path: string): HTMLTextAreaElement {
  return screen.getByRole("textbox", { name: path }) as HTMLTextAreaElement;
}

async function readCharacter(backend: Backend) {
  const file = await backend.readDocument(CHARACTER_PATH);
  if (file.document.kind !== "character") {
    throw new Error("人物資料として読めるはず");
  }
  return { ...file, document: file.document };
}

async function readChapter(backend: Backend) {
  const file = await backend.readDocument(CHAPTER_PATH);
  if (file.document.kind !== "chapter") {
    throw new Error("章立てとして読めるはず");
  }
  return { ...file, document: file.document };
}

/** readDocument の結果を、パスごとに差し替えた Backend。書き込まれた文書も記録する。 */
function backendWith(
  inner: Backend,
  rewrite: (path: string, file: DocumentFile) => DocumentFile,
): { backend: Backend; written: EditableDocument[] } {
  const written: EditableDocument[] = [];
  const backend = wrapBackend(inner, {
    async readDocument(path) {
      return rewrite(path, await inner.readDocument(path));
    },
    async writeDocument(path, document, expectedHash) {
      written.push(document);
      return inner.writeDocument(path, document, expectedHash);
    },
  });
  return { backend, written };
}

function saveWithShortcut(user: ReturnType<typeof userEvent.setup>) {
  return user.keyboard("{Control>}s{/Control}");
}

describe("人物資料", () => {
  it("開くと項目がフォームに入り、本文の欄に YAML は出ない", async () => {
    const backend = createMockBackend({ delayMs: 0 });

    await openDocument(backend, CHARACTER_PATH);

    expect(screen.getByLabelText("名前")).toHaveValue("霧島 凛");
    expect(screen.getByLabelText("読み")).toHaveValue("きりしま りん");
    expect(screen.getByLabelText("役割")).toHaveValue("主人公");
    expect(screen.getByLabelText("概要")).toHaveValue(
      "元灯台守。幼少期の事故で視力を失った盲目の女性。声の揺れで嘘を聞き分ける。",
    );
    expect(screen.getByLabelText("順番")).toHaveValue("1");
    const body = bodyTextarea(CHARACTER_PATH);
    expect(body.value).toContain("## 外見");
    expect(body.value).not.toContain("---");
    expect(body.value).not.toContain("name:");
    expect(screen.getByRole("heading", { name: "詳細" })).toBeInTheDocument();
  });

  it("名前を直すと自動保存され、読み直すと変わっていて、目次の人物名も変わる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    vi.useFakeTimers();
    fireEvent.change(screen.getByLabelText("名前"), { target: { value: "霧島 凛子" } });
    expect(screen.getByText("未保存の変更があります")).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    vi.useRealTimers();

    expect(screen.getByText("保存済み")).toBeInTheDocument();
    expect((await readCharacter(backend)).document.meta.name).toBe("霧島 凛子");
    expect(await screen.findByRole("button", { name: /^霧島 凛子/ })).toBeInTheDocument();
  });

  it("触らずに本文だけ直して保存すると、送る項目は読んだときと同じ（null も空文字にならない）", async () => {
    const user = userEvent.setup();
    const { backend, written } = backendWith(createMockBackend({ delayMs: 0 }), (path, file) =>
      path === CHARACTER_PATH && file.document.kind === "character"
        ? {
            ...file,
            document: {
              ...file.document,
              meta: { ...file.document.meta, reading: null, order: null },
            },
          }
        : file,
    );
    await openDocument(backend, CHARACTER_PATH);
    const before = (await readCharacter(backend)).document.meta;

    await user.click(bodyTextarea(CHARACTER_PATH));
    await user.type(bodyTextarea(CHARACTER_PATH), "足した一文");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    const sent = written.at(-1);
    expect(sent).toMatchObject({ kind: "character" });
    expect(sent?.kind === "character" && sent.meta).toStrictEqual(before);
    expect(sent?.kind === "character" && sent.body).toContain("足した一文");
  });

  it("順番を空にすると null で保存される。数字でない文字は受け付けず、外すと元の値に戻る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);
    const order = screen.getByLabelText("順番");

    await user.clear(order);
    await user.type(order, "x");
    expect(order).toHaveValue("x");
    await user.tab();
    expect(order).toHaveValue("");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readCharacter(backend)).document.meta.order).toBeNull();
  });

  it("順番は、全角の数字でも打てる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    const order = screen.getByLabelText("順番");
    await user.clear(order);
    await user.type(order, "３");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readCharacter(backend)).document.meta.order).toBe(3);
    await user.tab();
    expect(order).toHaveValue("3");
  });

  it("順番は、Rust の u32 の最大値まで入力できる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    const order = screen.getByLabelText("順番");
    await user.clear(order);
    await user.type(order, "4294967295");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readCharacter(backend)).document.meta.order).toBe(4294967295);
  });

  it("順番が最大値を超える入力は文書に反映せず、外すと最後の有効な値に戻る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    const order = screen.getByLabelText("順番");
    await user.clear(order);
    await user.type(order, "4294967296");
    expect(order).toHaveValue("4294967296");
    await user.tab();
    expect(order).toHaveValue("429496729");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readCharacter(backend)).document.meta.order).toBe(429496729);
  });

  it("読みを空欄から全角スペースで打ち始めても、その文字が消えない", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    const reading = screen.getByLabelText("読み");
    await user.clear(reading);
    await user.type(reading, "　きりしま");
    expect(reading).toHaveValue("　きりしま");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readCharacter(backend)).document.meta.reading).toBe("　きりしま");
  });

  it("読みを空にすると null で保存される", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    await user.clear(screen.getByLabelText("読み"));
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readCharacter(backend)).document.meta.reading).toBeNull();
  });

  it("フォームの入力欄で Ctrl+S を押すと保存される", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    const role = screen.getByLabelText("役割");
    await user.clear(role);
    await user.type(role, "探偵");
    expect(screen.getByText("未保存の変更があります")).toBeInTheDocument();
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readCharacter(backend)).document.meta.role).toBe("探偵");
  });

  it("外で書き換えられた人物資料を保存すると競合になり、再読み込みで新しい値がフォームに出る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);
    const outside = await readCharacter(backend);
    await backend.writeDocument(
      CHARACTER_PATH,
      { ...outside.document, meta: { ...outside.document.meta, name: "外で直した名前" } },
      outside.hash,
    );

    await user.type(screen.getByLabelText("役割"), "（手で追記）");
    await saveWithShortcut(user);

    expect(await screen.findByText("外部で変更されています")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "再読み込み" }));

    expect(await screen.findByLabelText("名前")).toHaveValue("外で直した名前");
    expect(screen.getByLabelText("役割")).toHaveValue("主人公");
    expect(screen.getByText("保存済み")).toBeInTheDocument();
  });

  it("ステータスバーの文字数は、項目を含めず本文だけを数える", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);

    fireEvent.change(bodyTextarea(CHARACTER_PATH), { target: { value: "あいうえお" } });

    expect(await screen.findByText("5 字")).toBeInTheDocument();
  });

  it("縦書きの切り替えは、本文の欄にだけ効き、フォームは横書きのまま", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHARACTER_PATH);
    const verticalClass = "editor-pane__textarea--vertical";

    expect(bodyTextarea(CHARACTER_PATH)).toHaveClass(verticalClass);
    expect(screen.getByLabelText("名前").closest(`.${verticalClass}`)).toBeNull();
    expect(document.querySelectorAll(`.${verticalClass}`)).toHaveLength(1);

    await user.click(screen.getByRole("button", { name: "横書きにする" }));

    expect(document.querySelectorAll(`.${verticalClass}`)).toHaveLength(0);
  });
});

describe("章立て", () => {
  it("開くと章題とシーンのカードが出て、本文の欄にはストーリーラインが出る", async () => {
    const backend = createMockBackend({ delayMs: 0 });

    await openDocument(backend, CHAPTER_PATH);

    expect(screen.getByLabelText("章題")).toHaveValue("雨の匂い");
    const cards = document.querySelectorAll("details.scene-card");
    expect(cards).toHaveLength(3);
    expect(cards[0]).toHaveTextContent("s01 招かれざる客");
    expect(cards[0]).not.toHaveAttribute("open");
    const storyline = bodyTextarea(CHAPTER_PATH);
    expect(storyline.value).toContain("柱時計の狂い");
    expect(storyline.value).not.toContain("scenes:");
    expect(screen.getByRole("heading", { name: "ストーリーライン" })).toBeInTheDocument();
  });

  it("シーンのタイトルを直すと保存され、目次にも反映される", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHAPTER_PATH);
    const card = within(screen.getByText("s01 招かれざる客").closest("details") as HTMLElement);

    await user.click(screen.getByText("s01 招かれざる客"));
    const title = card.getByLabelText("タイトル");
    await user.clear(title);
    await user.type(title, "招かれた客");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    const saved = await readChapter(backend);
    expect(saved.document.meta.scenes?.[0]?.title).toBe("招かれた客");
    expect(saved.document.meta.scenes?.[1]?.title).toBe("遺言状の間");
    expect(await screen.findByText("招かれた客")).toBeInTheDocument();
  });

  it("同じ id のシーンが並んでいても、直したシーンだけが変わる", async () => {
    const user = userEvent.setup();
    const { backend, written } = backendWith(createMockBackend({ delayMs: 0 }), (path, file) => {
      if (path !== CHAPTER_PATH || file.document.kind !== "chapter") {
        return file;
      }
      const scenes = (file.document.meta.scenes ?? []).map((scene) => ({ ...scene, id: "s01" }));
      return { ...file, document: { ...file.document, meta: { ...file.document.meta, scenes } } };
    });
    await openDocument(backend, CHAPTER_PATH);
    const first = within(screen.getByText("s01 招かれざる客").closest("details") as HTMLElement);
    const second = within(screen.getByText("s01 遺言状の間").closest("details") as HTMLElement);
    await user.click(screen.getByText("s01 招かれざる客"));
    await user.click(screen.getByText("s01 遺言状の間"));

    const title = first.getByLabelText("タイトル");
    await user.clear(title);
    await user.type(title, "招かれた客");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect(second.getByLabelText("タイトル")).toHaveValue("遺言状の間");
    const sent = written.at(-1);
    const sentTitles =
      sent?.kind === "chapter" ? sent.meta.scenes?.map((scene) => scene.title) : [];
    expect(sentTitles?.slice(0, 2)).toEqual(["招かれた客", "遺言状の間"]);
  });

  it("シーンの項目が入り、ビートは番号付きで読むだけ", async () => {
    const { backend } = backendWith(createMockBackend({ delayMs: 0 }), (path, file) => {
      if (path !== CHAPTER_PATH || file.document.kind !== "chapter") {
        return file;
      }
      const scenes = (file.document.meta.scenes ?? []).map((scene) =>
        scene.id === "s01" ? { ...scene, beats: ["依頼人が現れる。", "雨音が強まる。"] } : scene,
      );
      return { ...file, document: { ...file.document, meta: { ...file.document.meta, scenes } } };
    });
    await openDocument(backend, CHAPTER_PATH);

    const card = within(screen.getByText("s01 招かれざる客").closest("details") as HTMLElement);

    expect(card.getByLabelText("タイトル")).toHaveValue("招かれざる客");
    expect(card.getByLabelText("視点")).toHaveValue("霧島 凛");
    expect(card.getByLabelText("登場人物")).toHaveValue("霧島 凛、佐藤 健二");
    expect(card.getByLabelText("場所")).toHaveValue("水無月館 玄関ホール");
    expect(card.getByLabelText("時間")).toHaveValue("嵐の夜、開封の三時間前");
    expect(card.getByLabelText("目標文字数")).toHaveValue("1800");
    const beats = card.getAllByRole("listitem").map((item) => item.textContent);
    expect(beats).toEqual(["依頼人が現れる。", "雨音が強まる。"]);
    expect(card.getByText(/ここでは直せません/)).toBeInTheDocument();
    // ビートのないシーンには、ビートの欄を出さない
    const secondCard = within(screen.getByText("s02 遺言状の間").closest("details") as HTMLElement);
    expect(secondCard.queryByRole("listitem")).not.toBeInTheDocument();
  });

  it("登場人物の欄で「、」を打っても消えず、続けて名前を打てる。保存される配列は名前ごとに分かれる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHAPTER_PATH);
    const card = within(screen.getByText("s03 消えた甥").closest("details") as HTMLElement);
    await user.click(screen.getByText("s03 消えた甥"));

    const characters = card.getByLabelText("登場人物");
    await user.clear(characters);
    await user.type(characters, "霧島 凛、");
    expect(characters).toHaveValue("霧島 凛、");
    await user.type(characters, "佐藤 健二");
    expect(characters).toHaveValue("霧島 凛、佐藤 健二");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect((await readChapter(backend)).document.meta.scenes?.[2]?.characters).toEqual([
      "霧島 凛",
      "佐藤 健二",
    ]);
  });

  it("登場人物の欄は、フォーカスを外すと「、」でそろえて表示し直す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHAPTER_PATH);
    const card = within(screen.getByText("s03 消えた甥").closest("details") as HTMLElement);
    await user.click(screen.getByText("s03 消えた甥"));

    const characters = card.getByLabelText("登場人物");
    await user.clear(characters);
    await user.type(characters, "霧島 凛,  佐藤 健二、、");
    await user.tab();

    expect(characters).toHaveValue("霧島 凛、佐藤 健二");
  });

  it("目標文字数を空にすると null で保存される。触っていないシーンは変わらない", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHAPTER_PATH);
    const card = within(screen.getByText("s01 招かれざる客").closest("details") as HTMLElement);
    await user.click(screen.getByText("s01 招かれざる客"));

    await user.clear(card.getByLabelText("目標文字数"));
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    const scenes = (await readChapter(backend)).document.meta.scenes ?? [];
    expect(scenes[0]?.target_chars).toBeNull();
    expect(scenes[1]?.target_chars).toBe(2000);
  });

  it("視点・場所・時間を空にすると null で保存される", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHAPTER_PATH);
    const card = within(screen.getByText("s01 招かれざる客").closest("details") as HTMLElement);
    await user.click(screen.getByText("s01 招かれざる客"));

    await user.clear(card.getByLabelText("視点"));
    await user.clear(card.getByLabelText("場所"));
    await user.clear(card.getByLabelText("時間"));
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    const [first] = (await readChapter(backend)).document.meta.scenes ?? [];
    expect(first).toMatchObject({ pov: null, place: null, time: null });
  });

  it("シーンの本文（原稿）は、章立てを保存しても残る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await openDocument(backend, CHAPTER_PATH);

    await user.type(screen.getByLabelText("章題"), "（改）");
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    const manuscript = await backend.readDocument("manuscript/01/s01.txt");
    expect(manuscript.document).toMatchObject({ kind: "text" });
    expect(manuscript.document.kind === "text" && manuscript.document.content).toContain(
      "館の扉が開くたび",
    );
  });
});

describe("front matter を読めなかった文書", () => {
  const BROKEN_YAML = "---\nname: [\nrole: 主人公\n---\n本文\n";

  function brokenCharacter(inner: Backend) {
    return backendWith(inner, (path, file) =>
      path === CHARACTER_PATH
        ? {
            ...file,
            document: textDocument(BROKEN_YAML),
            parse_error: `${CHARACTER_PATH} の 2 行目を読めません`,
          }
        : file,
    );
  }

  it("理由を出し、ファイル全体を本文の欄で開く（フォームは出さない）", async () => {
    const { backend } = brokenCharacter(createMockBackend({ delayMs: 0 }));

    await openDocument(backend, CHARACTER_PATH);

    expect(
      screen.getByText("front matter を読めないため、ファイルをそのまま表示しています。"),
    ).toBeInTheDocument();
    expect(screen.getByText(`${CHARACTER_PATH} の 2 行目を読めません`)).toBeInTheDocument();
    expect(bodyTextarea(CHARACTER_PATH).value).toBe(BROKEN_YAML);
    expect(screen.queryByLabelText("名前")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "詳細" })).not.toBeInTheDocument();
  });

  it("直して保存できる", async () => {
    const user = userEvent.setup();
    const { backend, written } = brokenCharacter(createMockBackend({ delayMs: 0 }));
    await openDocument(backend, CHARACTER_PATH);

    const fixed = "---\nname: 霧島 凛\nrole: 主人公\nsummary: 概要\n---\n本文\n";
    fireEvent.change(bodyTextarea(CHARACTER_PATH), { target: { value: fixed } });
    await saveWithShortcut(user);

    await screen.findByText("保存済み");
    expect(written.at(-1)).toEqual(textDocument(fixed));
  });

  it("YAML の無い普通の文書には、理由も本文の見出しも出さない", async () => {
    const backend = createMockBackend({ delayMs: 0 });

    await openDocument(backend, "concept.md");

    expect(screen.queryByText(/front matter を読めない/)).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "詳細" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "ストーリーライン" })).not.toBeInTheDocument();
  });
});

describe("ルビのプレビュー", () => {
  it("本文（.txt）だけで使え、人物資料・章立てには出ない", async () => {
    const backend = createMockBackend({ delayMs: 0 });

    await openDocument(backend, CHARACTER_PATH);

    expect(screen.queryByRole("button", { name: "ルビ・傍点プレビュー" })).not.toBeInTheDocument();
  });
});
