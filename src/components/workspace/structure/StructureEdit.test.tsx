import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../../api/backend";
import { BackendError } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../../api/mock/sampleProject";
import type { StructureEdit } from "../../../api/types";
import { useUiStore } from "../../../store/uiStore";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { readText } from "../../../test/documents";
import { renderWithBackend } from "../../../test/renderWithBackend";
import { resetAllStores } from "../../../test/resetStores";
import { wrapBackend } from "../../../test/wrapBackend";
import { WorkspaceScreen } from "../WorkspaceScreen";

beforeEach(resetAllStores);
afterEach(resetAllStores);

async function renderWorkspace(backend: Backend): Promise<void> {
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
  renderWithBackend(<WorkspaceScreen />, backend);
}

/**
 * かなをローマ字にする本物（Rust）の提案の代わり。偽バックエンドは英数字しか変換しないので、
 * 画面の提案と、ID を省いて追加したときに決まる ID の両方を、同じ表で差し替える。
 */
function romajiSuggestions(inner: Backend) {
  const suggestCharacterId = vi.fn(async (reading: string, name: string) => {
    if (reading === "にいやま ゆき") {
      return "niiyama-yuki";
    }
    return inner.suggestCharacterId(reading, name);
  });
  const backend = wrapBackend(inner, {
    suggestCharacterId,
    async planStructureEdit(edit) {
      if (edit.kind === "add_character" && edit.id === null) {
        const id = await suggestCharacterId(edit.meta.reading ?? "", edit.meta.name);
        return inner.planStructureEdit({ ...edit, id });
      }
      return inner.planStructureEdit(edit);
    },
  });
  return { backend, suggestCharacterId };
}

/** 構成の操作の変更案づくりの呼び出しを記録する。`overrides` で、ほかの呼び出しも差し替えられる。 */
function planned(inner: Backend, overrides: Partial<Backend> = {}) {
  const planStructureEdit = vi.fn(inner.planStructureEdit.bind(inner));
  return { backend: wrapBackend(inner, { planStructureEdit, ...overrides }), planStructureEdit };
}

/** 節の見出しの「＋」を押して、そのダイアログを開く。 */
async function openAddDialog(
  user: ReturnType<typeof userEvent.setup>,
  buttonName: string,
  dialogTitle: string,
) {
  await user.click(screen.getByRole("button", { name: buttonName }));
  return screen.findByRole("dialog", { name: dialogTitle });
}

const openAddCharacterDialog = (user: ReturnType<typeof userEvent.setup>) =>
  openAddDialog(user, "人物を追加", "人物を追加");
const openAddWorldDocumentDialog = (user: ReturnType<typeof userEvent.setup>) =>
  openAddDialog(user, "資料を追加", "世界観の資料を追加");

async function openRowMenu(
  user: ReturnType<typeof userEvent.setup>,
  label: string,
  item: string,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: `「${label}」の操作` }));
  await user.click(screen.getByRole("menuitem", { name: item }));
}

function hasToast(text: string): boolean {
  return useUiStore.getState().toasts.some((toast) => toast.message.includes(text));
}

describe("人物を追加する", () => {
  it("読みから提案された ID で、人物資料を作り、目次に出して開く", async () => {
    const user = userEvent.setup();
    const { backend } = romajiSuggestions(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);

    const dialog = await openAddCharacterDialog(user);
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "新山 ゆき");
    await user.type(within(dialog).getByRole("textbox", { name: "読み" }), "にいやま ゆき");
    await waitFor(() =>
      expect(within(dialog).getByRole("textbox", { name: "ID" })).toHaveValue("niiyama-yuki"),
    );
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByRole("button", { name: /^新山 ゆき/ })).toBeInTheDocument();
    expect(
      await screen.findByRole("textbox", { name: "characters/niiyama-yuki.md" }),
    ).toBeInTheDocument();
    expect(useWorkspaceStore.getState().currentPath).toBe("characters/niiyama-yuki.md");
    expect(hasToast("人物「新山 ゆき」を追加しました。")).toBe(true);
  });

  it("触るまでは入力に合わせて ID の提案が変わり、触ったらそれ以降は追従しない", async () => {
    const user = userEvent.setup();
    const { backend } = romajiSuggestions(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    const idInput = within(dialog).getByRole("textbox", { name: "ID" });

    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "Rin Sato");
    await waitFor(() => expect(idInput).toHaveValue("rin-sato"));
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "o");
    await waitFor(() => expect(idInput).toHaveValue("rin-satoo"));

    await user.clear(idInput);
    await user.type(idInput, "my-own-id");
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "x");
    // 提案の問い合わせの待ち時間より長く待っても、触った ID は変わらない
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(idInput).toHaveValue("my-own-id");

    await user.click(within(dialog).getByRole("button", { name: "追加" }));
    await waitFor(() =>
      expect(useWorkspaceStore.getState().currentPath).toBe("characters/my-own-id.md"),
    );
  });

  it("ID を触っていなければ、ID を送らず（null）、決めるのを任せる", async () => {
    const user = userEvent.setup();
    const { backend, planStructureEdit } = planned(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);

    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "Rin Sato");
    await waitFor(() =>
      expect(within(dialog).getByRole("textbox", { name: "ID" })).toHaveValue("rin-sato"),
    );
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(planStructureEdit).toHaveBeenCalled());
    const edit = planStructureEdit.mock.calls[0]?.[0];
    expect(edit).toMatchObject({ kind: "add_character", id: null });
  });

  it("触った ID は、そのまま送る", async () => {
    const user = userEvent.setup();
    const { backend, planStructureEdit } = planned(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);

    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "新山 ゆき");
    await user.type(within(dialog).getByRole("textbox", { name: "ID" }), "yuki");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(planStructureEdit).toHaveBeenCalled());
    expect(planStructureEdit.mock.calls[0]?.[0]).toMatchObject({
      kind: "add_character",
      id: "yuki",
    });
  });

  it("古い入力への提案が遅れて届いても、新しい入力への提案を上書きしない", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const pending: Array<{ name: string; resolve: (id: string) => void }> = [];
    const backend = wrapBackend(inner, {
      suggestCharacterId: (_reading, name) =>
        new Promise<string>((resolve) => pending.push({ name, resolve })),
    });
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    const nameInput = within(dialog).getByRole("textbox", { name: "名前" });

    await user.type(nameInput, "a");
    await waitFor(() => expect(pending).toHaveLength(1));
    await user.type(nameInput, "b");
    await waitFor(() => expect(pending).toHaveLength(2));
    pending[1]?.resolve("ab-new");
    await waitFor(() =>
      expect(within(dialog).getByRole("textbox", { name: "ID" })).toHaveValue("ab-new"),
    );
    pending[0]?.resolve("a-old");

    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(within(dialog).getByRole("textbox", { name: "ID" })).toHaveValue("ab-new");
  });

  it("名前が空の間は追加できない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddCharacterDialog(user);

    expect(within(dialog).getByRole("button", { name: "追加" })).toBeDisabled();
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "  ");
    expect(within(dialog).getByRole("button", { name: "追加" })).toBeDisabled();
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "新山");
    expect(within(dialog).getByRole("button", { name: "追加" })).toBeEnabled();
  });

  it("入力欄で Enter を押しても追加しない（日本語入力の確定の Enter で送らない）", async () => {
    const user = userEvent.setup();
    const { backend, planStructureEdit } = planned(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);

    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "新山 ゆき{Enter}");

    expect(planStructureEdit).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "人物を追加" })).toBeInTheDocument();
  });

  it("使えない ID を書くと、理由を出して追加を無効にし、直すと追加できる", async () => {
    const user = userEvent.setup();
    const { backend, planStructureEdit } = planned(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    const idField = within(dialog).getByRole("textbox", { name: "ID" });
    const addButton = within(dialog).getByRole("button", { name: "追加" });
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "新山");
    expect(addButton).toBeEnabled();

    await user.type(idField, "Rin");
    expect(
      within(dialog).getByText("ID「Rin」は使えません。大文字は使えません。小文字にしてください。"),
    ).toBeInTheDocument();
    expect(idField).toBeInvalid();
    expect(addButton).toBeDisabled();

    await user.clear(idField);
    await user.type(idField, "霧島");
    expect(
      within(dialog).getByText(
        "ID「霧島」は使えません。小文字の英数字とハイフンだけにしてください。",
      ),
    ).toBeInTheDocument();
    expect(addButton).toBeDisabled();

    await user.clear(idField);
    await user.type(idField, "rin-");
    expect(within(dialog).getByText(/先頭と末尾にハイフンは使えません/)).toBeInTheDocument();
    expect(addButton).toBeDisabled();
    expect(planStructureEdit).not.toHaveBeenCalled();

    // 空欄に戻すと自動に戻る
    await user.clear(idField);
    expect(idField).toBeValid();
    expect(addButton).toBeEnabled();

    await user.type(idField, "niiyama");
    expect(within(dialog).queryByText(/使えません/)).not.toBeInTheDocument();
    await user.click(addButton);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(useWorkspaceStore.getState().currentPath).toBe("characters/niiyama.md");
  });

  it("使用済みの ID を指定すると、ダイアログの中に理由を出し、閉じない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddCharacterDialog(user);

    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "別の人");
    await user.type(within(dialog).getByRole("textbox", { name: "ID" }), "kirishima-rin");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "ID「kirishima-rin」はもう使われています。",
    );
    expect(within(dialog).getByRole("button", { name: "追加" })).toBeEnabled();
  });

  it("やめると、何も作らずに閉じる", async () => {
    const user = userEvent.setup();
    const { backend, planStructureEdit } = planned(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "新山 ゆき");

    await user.click(within(dialog).getByRole("button", { name: "やめる" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(planStructureEdit).not.toHaveBeenCalled();
  });
});

describe("世界観の資料を追加する", () => {
  it("題と本文から資料を作り、目次に題で出して開く", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    const dialog = await openAddWorldDocumentDialog(user);
    await user.type(within(dialog).getByRole("textbox", { name: "題" }), "用語集");
    await user.type(within(dialog).getByRole("textbox", { name: "本文" }), "霧：朝に出る。");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByRole("button", { name: /^用語集/ })).toBeInTheDocument();
    expect(await screen.findByRole("textbox", { name: "world/doc.md" })).toHaveValue(
      "# 用語集\n\n霧：朝に出る。\n",
    );
    expect(await readText(backend, "world/doc.md")).toBe("# 用語集\n\n霧：朝に出る。\n");
  });

  it("ファイル名を入れると、その名前で作る。空欄なら null を送る", async () => {
    const user = userEvent.setup();
    const { backend, planStructureEdit } = planned(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);

    const dialog = await openAddWorldDocumentDialog(user);
    await user.type(within(dialog).getByRole("textbox", { name: "題" }), "用語集");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));
    await waitFor(() => expect(planStructureEdit).toHaveBeenCalledTimes(1));
    expect(planStructureEdit.mock.calls[0]?.[0]).toMatchObject({ name: null });

    const second = await openAddWorldDocumentDialog(user);
    await user.type(within(second).getByRole("textbox", { name: "題" }), "地名");
    await user.type(
      within(second).getByRole("textbox", { name: "ファイル名（英数字。空欄なら自動）" }),
      "places",
    );
    await user.click(within(second).getByRole("button", { name: "追加" }));
    await waitFor(() => expect(useWorkspaceStore.getState().currentPath).toBe("world/places.md"));
  });

  it("題が空の間は追加できず、使えない名前なら理由を出す", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddWorldDocumentDialog(user);
    expect(within(dialog).getByRole("button", { name: "追加" })).toBeDisabled();

    await user.type(within(dialog).getByRole("textbox", { name: "題" }), "用語集");
    await user.type(
      within(dialog).getByRole("textbox", { name: "ファイル名（英数字。空欄なら自動）" }),
      "overview",
    );
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent("overview");
  });
});

describe("シーンを追加する", () => {
  it("章の見出しのメニューから、章の最後にシーンを足し、章立てを開く", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await openRowMenu(user, "雨の匂い", "シーンを追加");
    const dialog = await screen.findByRole("dialog", { name: "シーンを追加" });
    expect(within(dialog).getByRole("combobox", { name: "位置" })).toHaveDisplayValue("章の最後");
    await user.type(within(dialog).getByRole("textbox", { name: "タイトル" }), "雨上がり");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByRole("button", { name: /^雨上がり/ })).toBeInTheDocument();
    expect(useWorkspaceStore.getState().currentPath).toBe("plot/chapters/01.md");
    expect(await screen.findByRole("textbox", { name: "plot/chapters/01.md" })).toBeInTheDocument();
  });

  it("シーンのメニューの「この前に」は、そのシーンの前を初めの位置にして足す", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "遺言状の間", "この前にシーンを追加");
    const dialog = await screen.findByRole("dialog", { name: "シーンを追加" });
    expect(within(dialog).getByRole("combobox", { name: "位置" })).toHaveDisplayValue(
      "遺言状の間 の前",
    );
    await user.type(within(dialog).getByRole("textbox", { name: "タイトル" }), "割り込み");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const labels = (
      await screen.findAllByRole("button", { name: /^(招かれざる客|割り込み|遺言状の間)/ })
    ).map((button) => button.textContent ?? "");
    expect(labels.findIndex((label) => label.startsWith("割り込み"))).toBe(
      labels.findIndex((label) => label.startsWith("遺言状の間")) - 1,
    );
  });

  it("「この後に」は、次のシーンの前（最後のシーンなら章の最後）を初めの位置にする", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "遺言状の間", "この後にシーンを追加");
    expect(
      within(await screen.findByRole("dialog", { name: "シーンを追加" })).getByRole("combobox", {
        name: "位置",
      }),
    ).toHaveDisplayValue("消えた甥 の前");
    await userEvent.setup().click(screen.getByRole("button", { name: "やめる" }));

    await openRowMenu(user, "消えた甥", "この後にシーンを追加");
    expect(
      within(await screen.findByRole("dialog", { name: "シーンを追加" })).getByRole("combobox", {
        name: "位置",
      }),
    ).toHaveDisplayValue("章の最後");
  });

  it("タイトルが空の間は追加できない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "雨の匂い", "シーンを追加");
    const dialog = await screen.findByRole("dialog", { name: "シーンを追加" });

    expect(within(dialog).getByRole("button", { name: "追加" })).toBeDisabled();
  });
});

describe("削除の確認", () => {
  it("人物を消すときは、ゴミ箱へ移るものと、その人物を挙げているシーンを見せてから、移す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await openRowMenu(user, "佐藤 健二", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });

    expect(
      await within(dialog).findByText("人物「佐藤 健二」をゴミ箱へ移します。"),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("characters/sato-kenji.md")).toBeInTheDocument();
    expect(within(dialog).getByText("この人物を挙げているシーン")).toBeInTheDocument();
    expect(
      within(dialog).getByText(/第1章「雨の匂い」 招かれざる客（登場人物）/),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(/第1章「雨の匂い」 遺言状の間（登場人物）/),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText("ゴミ箱は作品フォルダの .kataribe/trash/ です。"),
    ).toBeInTheDocument();
    // 確認の段階では、まだ何も変えていない
    await expect(backend.readDocument("characters/sato-kenji.md")).resolves.toBeDefined();

    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /^佐藤 健二/ })).not.toBeInTheDocument();
    await expect(backend.readDocument("characters/sato-kenji.md")).rejects.toMatchObject({
      kind: "not_found",
    });
    expect(hasToast("人物「佐藤 健二」をゴミ箱へ移しました。")).toBe(true);
  });

  it("本文のあるシーンを消すときは、本文もゴミ箱へ移ることを、字数つきで強調する", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);
    const sceneChars = (await readText(backend, "manuscript/01/s01.txt")).replace(/\s/g, "").length;

    await openRowMenu(user, "招かれざる客", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });

    const emphasis = await within(dialog).findByText(/本文 1 ファイル/);
    expect(emphasis).toHaveTextContent(
      `本文 1 ファイル（計 ${sceneChars} 字）もゴミ箱へ移ります。`,
    );
    expect(within(dialog).getByText("manuscript/01/s01.txt")).toBeInTheDocument();
  });

  it("本文の無いシーンを消すときは、本文についての強調を出さない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "消えた甥", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });

    expect(
      await within(dialog).findByText("第1章のシーン「消えた甥」を削除します。"),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText(/本文 \d+ ファイル/)).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /^消えた甥/ })).not.toBeInTheDocument();
  });

  it("やめると、何も変えずに閉じる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await openRowMenu(user, "佐藤 健二", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });
    await within(dialog).findByText("人物「佐藤 健二」をゴミ箱へ移します。");
    await user.click(within(dialog).getByRole("button", { name: "やめる" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await expect(backend.readDocument("characters/sato-kenji.md")).resolves.toBeDefined();
  });

  it("確かめたあとに作品が変わって競合したら、削除せずにそのことを知らせ、もう一度確かめられる", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    let conflicts = 1;
    const { backend, planStructureEdit } = planned(inner, {
      async applyChangeSet(changeSet) {
        if (conflicts > 0) {
          conflicts -= 1;
          throw new BackendError("conflict", "作品が外で変わっています。");
        }
        return inner.applyChangeSet(changeSet);
      },
    });
    await renderWorkspace(backend);

    await openRowMenu(user, "佐藤 健二", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });
    await within(dialog).findByText("人物「佐藤 健二」をゴミ箱へ移します。");
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "作品が変わったため削除しませんでした。",
    );
    expect(within(dialog).queryByRole("button", { name: "ゴミ箱へ移す" })).not.toBeInTheDocument();
    expect(planStructureEdit).toHaveBeenCalledTimes(1);

    await user.click(within(dialog).getByRole("button", { name: "もう一度確かめる" }));

    await waitFor(() => expect(planStructureEdit).toHaveBeenCalledTimes(2));
    expect(await within(dialog).findByRole("button", { name: "ゴミ箱へ移す" })).toBeEnabled();
    expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("ファイル名が ID の規則に合わない人物資料にも「削除」が出て、実際に消せる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);
    await backend.applyChangeSet({
      summary: "人物資料を置きます。",
      project_root: SAMPLE_PROJECT_FOLDER,
      files: [
        {
          kind: "write",
          path: "characters/Rin.md",
          content: "---\nname: リン\n---\n",
          previous: null,
          base_hash: null,
        },
      ],
    });
    await renderWorkspace(backend);

    await openRowMenu(user, "リン", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });
    expect(await within(dialog).findByText("characters/Rin.md")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /^リン/ })).not.toBeInTheDocument();
    expect(hasToast("人物「リン」をゴミ箱へ移しました。")).toBe(true);
  });

  it("消す対象が見つからなければ、理由を出し、移すボタンは使えない", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const backend = wrapBackend(inner, {
      async planStructureEdit(_edit: StructureEdit) {
        throw new BackendError("not_found", "人物資料 characters/sato-kenji.md がありません。");
      },
    });
    await renderWorkspace(backend);

    await openRowMenu(user, "佐藤 健二", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });

    expect(await within(dialog).findByRole("alert")).toHaveTextContent("がありません");
    expect(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" })).toBeDisabled();
  });
});

describe("開いている文書を消したとき", () => {
  it("ゴミ箱へ移った文書は閉じて、そのことを知らせる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);
    await user.click(screen.getByRole("button", { name: /^佐藤 健二/ }));
    await screen.findByRole("textbox", { name: "characters/sato-kenji.md" });

    await openRowMenu(user, "佐藤 健二", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });
    await within(dialog).findByText("人物「佐藤 健二」をゴミ箱へ移します。");
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByText("左の目次から文書を選んでください。")).toBeInTheDocument();
    expect(useWorkspaceStore.getState().currentPath).toBeNull();
    expect(
      hasToast("開いていた「characters/sato-kenji.md」はゴミ箱へ移したので、閉じました。"),
    ).toBe(true);
  });

  it("消すシーンの本文を開いていても、同じように閉じる", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    await user.click(screen.getByRole("button", { name: /^招かれざる客/ }));
    await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" });

    await openRowMenu(user, "招かれざる客", "削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });
    await within(dialog).findByText(/本文 1 ファイル/);
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));

    await waitFor(() => expect(useWorkspaceStore.getState().currentPath).toBeNull());
    expect(
      screen.queryByRole("textbox", { name: "manuscript/01/s01.txt" }),
    ).not.toBeInTheDocument();
  });

  it("章立てを開いたままシーンを足すと、足したシーンが開いている章立てに反映される", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    await user.click(screen.getAllByRole("button", { name: /^雨の匂い/ })[0] as HTMLElement);
    await screen.findByRole("textbox", { name: "plot/chapters/01.md" });

    await openRowMenu(user, "雨の匂い", "シーンを追加");
    const dialog = await screen.findByRole("dialog", { name: "シーンを追加" });
    await user.type(within(dialog).getByRole("textbox", { name: "タイトル" }), "雨上がり");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByText(/^s04\s*雨上がり/)).toBeInTheDocument();
  });
});

describe("生成のセッションが落ち着いていない間", () => {
  it("生成している間は、追加と削除を無効にし、理由を title に出す", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const backend = wrapBackend(inner, { generate: () => new Promise(() => {}) });
    await renderWorkspace(backend);

    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByRole("button", { name: "中止" });

    const addCharacter = screen.getByRole("button", { name: "人物を追加" });
    expect(addCharacter).toBeDisabled();
    expect(addCharacter).toHaveAttribute("title", "生成している間は、追加・削除できません。");
    expect(screen.getByRole("button", { name: "資料を追加" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "「霧島 凛」の操作" }));
    expect(screen.getByRole("menuitem", { name: "削除" })).toBeDisabled();
    expect(screen.getByRole("menuitem", { name: "削除" })).toHaveAttribute(
      "title",
      "生成している間は、追加・削除できません。",
    );
  });

  it("生成した変更案を確認している間も無効で、適用か破棄をすると使える", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByRole("button", { name: "破棄" });
    expect(screen.getByRole("button", { name: "人物を追加" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "人物を追加" })).toHaveAttribute(
      "title",
      expect.stringContaining("確認している間"),
    );

    await user.click(screen.getByRole("button", { name: "破棄" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "人物を追加" })).toBeEnabled());
  });
});

describe("まだ無い本文を、空で作って書き始める操作", () => {
  async function selectUnwrittenScene(user: ReturnType<typeof userEvent.setup>) {
    await user.click(await screen.findByRole("button", { name: /^消えた甥/ }));
    return screen.findByRole("button", { name: "空の本文から書き始める" });
  }

  it("生成している間は押せず、理由を title に出す（生成した本文の書き先がずれないように）", async () => {
    const user = userEvent.setup();
    const backend = wrapBackend(createMockBackend({ delayMs: 0 }), {
      generate: () => new Promise(() => {}),
    });
    await renderWorkspace(backend);
    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByRole("button", { name: "中止" });

    const startButton = await selectUnwrittenScene(user);

    expect(startButton).toBeDisabled();
    expect(startButton).toHaveAttribute("title", "生成している間は、本文を作成できません。");
  });

  it("生成した変更案を確認している間も押せず、適用か破棄をすると押せる", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByRole("button", { name: "破棄" });

    const startButton = await selectUnwrittenScene(user);
    expect(startButton).toBeDisabled();
    expect(startButton).toHaveAttribute("title", expect.stringContaining("確認している間"));

    await user.click(screen.getByRole("button", { name: "破棄" }));

    await waitFor(() => expect(startButton).toBeEnabled());
    expect(startButton).not.toHaveAttribute("title");
  });
});
