import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BackendError } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../../api/mock/sampleProject";
import { useEditorStore } from "../../../store/editorStore";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { readText, textDocument } from "../../../test/documents";
import { resetAllStores } from "../../../test/resetStores";
import { hasToast, openRowMenu, renderWorkspace } from "../../../test/structureUi";
import { wrapBackend } from "../../../test/wrapBackend";

// 章の追加・削除（番号の振り直し）の画面。サンプル作品は第 1 章「雨の匂い」（本文が 2 つ）と
// 第 2 章「灯台のある岬」（シーン構成なし）を持つ。

beforeEach(resetAllStores);
afterEach(resetAllStores);

const PLAN_MENU = "章立ての";

/** 目次の「あらすじ・章立て」の節に並ぶ章立ての題。 */
function plannedChapterLabels(): string[] {
  const plot = screen.getByRole("heading", { name: "あらすじ・章立て" }).closest("section");
  if (plot === null) {
    throw new Error("あらすじ・章立ての節があるはず");
  }
  return within(plot)
    .getAllByRole("button", { name: /^(雨の匂い|灯台のある岬|序章|終わりの章|新しい章)/ })
    .map((button) => button.querySelector(".project-tree__label")?.textContent ?? "");
}

async function typeChapter(
  user: ReturnType<typeof userEvent.setup>,
  dialog: HTMLElement,
  title: string,
  storyline = "",
): Promise<void> {
  await user.type(within(dialog).getByRole("textbox", { name: "章題" }), title);
  if (storyline !== "") {
    await user.type(within(dialog).getByRole("textbox", { name: "ストーリーライン" }), storyline);
  }
}

describe("章を追加する", () => {
  it("節の見出しの「＋」から、末尾に章を足し、新しい章の章立てを開く", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await user.click(screen.getByRole("button", { name: "章を追加" }));
    const dialog = await screen.findByRole("dialog", { name: "章を追加" });
    expect(within(dialog).getByRole("combobox", { name: "位置" })).toHaveDisplayValue("末尾");
    await typeChapter(user, dialog, "終わりの章", "夜が明ける。");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(plannedChapterLabels()).toEqual(["雨の匂い", "灯台のある岬", "終わりの章"]);
    expect(useWorkspaceStore.getState().currentPath).toBe("plot/chapters/03.md");
    expect(hasToast("第3章「終わりの章」を追加しました。")).toBe(true);
    const { document } = await backend.readDocument("plot/chapters/03.md");
    expect(document).toMatchObject({
      kind: "chapter",
      meta: { title: "終わりの章" },
      body: "夜が明ける。\n",
    });
  });

  it("末尾に足すときは、番号がずれる注意書きを出さない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await user.click(screen.getByRole("button", { name: "章を追加" }));
    const dialog = await screen.findByRole("dialog", { name: "章を追加" });

    expect(within(dialog).queryByText(/番号が 1 つ後ろにずれます/)).not.toBeInTheDocument();
    expect(within(dialog).queryByText(/シーン構成の無い章を途中に足すと/)).not.toBeInTheDocument();
  });

  it("章立ての行の「この前に章を追加」は、その章の前を初めの位置にし、番号のずれを知らせて、振り直す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await openRowMenu(user, "雨の匂い", "この前に章を追加", PLAN_MENU);
    const dialog = await screen.findByRole("dialog", { name: "章を追加" });
    expect(within(dialog).getByRole("combobox", { name: "位置" })).toHaveDisplayValue(
      "雨の匂い の前",
    );
    expect(
      within(dialog).getByText(
        "第1章以降の章は、番号が 1 つ後ろにずれます（本文のフォルダも一緒に移ります）。",
      ),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(
        "シーン構成の無い章を途中に足すと、それより後ろの本文の工程は、その章のシーン構成ができるまで進みません。",
      ),
    ).toBeInTheDocument();
    await typeChapter(user, dialog, "序章");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(plannedChapterLabels()).toEqual(["序章", "雨の匂い", "灯台のある岬"]);
    expect(await readText(backend, "manuscript/02/s01.txt")).toContain("館の扉が開くたび");
    const { document } = await backend.readDocument("plot/chapters/02.md");
    expect(document).toMatchObject({ kind: "chapter", meta: { title: "雨の匂い" } });
  });

  it("「この後に章を追加」は、次の章の前（最後の章なら末尾）を初めの位置にする", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "雨の匂い", "この後に章を追加", PLAN_MENU);
    expect(
      within(await screen.findByRole("dialog", { name: "章を追加" })).getByRole("combobox", {
        name: "位置",
      }),
    ).toHaveDisplayValue("灯台のある岬 の前");
    await user.click(screen.getByRole("button", { name: "やめる" }));

    await openRowMenu(user, "灯台のある岬", "この後に章を追加", PLAN_MENU);
    expect(
      within(await screen.findByRole("dialog", { name: "章を追加" })).getByRole("combobox", {
        name: "位置",
      }),
    ).toHaveDisplayValue("末尾");
  });

  it("位置を選び直すと、注意書きが付いたり消えたりする", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await user.click(screen.getByRole("button", { name: "章を追加" }));
    const dialog = await screen.findByRole("dialog", { name: "章を追加" });
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: "位置" }),
      "灯台のある岬 の前",
    );

    expect(within(dialog).getByText(/第2章以降の章は、番号が 1 つ後ろにずれます/)).toBeVisible();
  });

  it("章題が空の間は追加できず、入力欄の Enter でも追加しない", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await user.click(screen.getByRole("button", { name: "章を追加" }));
    const dialog = await screen.findByRole("dialog", { name: "章を追加" });
    expect(within(dialog).getByRole("button", { name: "追加" })).toBeDisabled();

    await user.type(within(dialog).getByRole("textbox", { name: "章題" }), "新しい章{Enter}");

    expect(screen.getByRole("dialog", { name: "章を追加" })).toBeInTheDocument();
    expect(plannedChapterLabels()).toEqual(["雨の匂い", "灯台のある岬"]);
  });

  it("失敗したら理由をダイアログの中に出し、閉じない", async () => {
    const user = userEvent.setup();
    const backend = wrapBackend(createMockBackend({ delayMs: 0 }), {
      async planStructureEdit() {
        throw new BackendError("invalid_input", "章は 999 までです。これより後ろには足せません。");
      },
    });
    await renderWorkspace(backend);

    await user.click(screen.getByRole("button", { name: "章を追加" }));
    const dialog = await screen.findByRole("dialog", { name: "章を追加" });
    await typeChapter(user, dialog, "新しい章");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent("999 まで");
    expect(screen.getByRole("dialog", { name: "章を追加" })).toBeInTheDocument();
  });
});

describe("章を削除する", () => {
  it("本文の章見出しのメニューから、ゴミ箱へ移るもの・番号が変わる章を見せてから、移す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);
    const chars = (
      (await readText(backend, "manuscript/01/s01.txt")) +
      (await readText(backend, "manuscript/01/s02.txt"))
    ).replace(/\s/g, "").length;

    await openRowMenu(user, "雨の匂い", "章を削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });

    expect(
      await within(dialog).findByText("第1章「雨の匂い」をゴミ箱へ移します。"),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("plot/chapters/01.md")).toBeInTheDocument();
    expect(within(dialog).getByText("manuscript/01/s01.txt")).toBeInTheDocument();
    expect(within(dialog).getByText("manuscript/01/s02.txt")).toBeInTheDocument();
    expect(within(dialog).getByText(/本文 2 ファイル/)).toHaveTextContent(
      `本文 2 ファイル（計 ${chars} 字）もゴミ箱へ移ります。`,
    );
    expect(within(dialog).getByText("番号が変わる章")).toBeInTheDocument();
    expect(within(dialog).getByText("第2章「灯台のある岬」 → 第1章")).toBeInTheDocument();
    // 確認の段階では、まだ何も変えていない
    expect(plannedChapterLabels()).toEqual(["雨の匂い", "灯台のある岬"]);

    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(plannedChapterLabels()).toEqual(["灯台のある岬"]);
    expect(hasToast("第1章「雨の匂い」をゴミ箱へ移しました。")).toBe(true);
    await expect(backend.readDocument("manuscript/01/s01.txt")).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("章立ての行の「削除」でも消せる。最後の章で、本文も無ければ、番号が変わる章も本文の強調も出さない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "灯台のある岬", "削除", PLAN_MENU);
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });

    expect(
      await within(dialog).findByText("第2章「灯台のある岬」をゴミ箱へ移します。"),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText("番号が変わる章")).not.toBeInTheDocument();
    expect(within(dialog).queryByText(/本文 \d+ ファイル/)).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(plannedChapterLabels()).toEqual(["雨の匂い"]);
  });

  it("確かめたあとに本文のフォルダへファイルが増えたら、消さずに知らせる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await openRowMenu(user, "雨の匂い", "章を削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });
    await within(dialog).findByText("第1章「雨の匂い」をゴミ箱へ移します。");
    await backend.writeDocument("manuscript/01/s03.txt", textDocument("外で書いた本文"), null);
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "作品が変わったため削除しませんでした。",
    );
    expect(await readText(backend, "manuscript/01/s03.txt")).toBe("外で書いた本文");
  });
});

describe("開いている文書の追従", () => {
  async function openFirstScene(user: ReturnType<typeof userEvent.setup>): Promise<void> {
    await user.click(await screen.findByRole("button", { name: /^招かれざる客/ }));
    await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" });
  }

  it("章を足して本文のフォルダが改名されても、開いていた本文は読み直さずに続けて編集でき、新しいパスへ保存する", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const readDocument = vi.fn(inner.readDocument.bind(inner));
    const backend = wrapBackend(inner, { readDocument });
    await renderWorkspace(backend);
    await openFirstScene(user);
    const readsBefore = readDocument.mock.calls.length;

    await openRowMenu(user, "雨の匂い", "この前に章を追加", PLAN_MENU);
    const dialog = await screen.findByRole("dialog", { name: "章を追加" });
    await typeChapter(user, dialog, "序章");
    await user.click(within(dialog).getByRole("button", { name: "追加" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    // 作った章立てではなく、編集していた本文が、新しいパスで開いたまま
    const editor = await screen.findByRole("textbox", { name: "manuscript/02/s01.txt" });
    expect(useWorkspaceStore.getState().currentPath).toBe("manuscript/02/s01.txt");
    expect(useEditorStore.getState().path).toBe("manuscript/02/s01.txt");
    expect(readDocument.mock.calls.length).toBe(readsBefore);

    fireEvent.change(editor, { target: { value: "改名のあとに書き足した本文" } });
    fireEvent.keyDown(editor, { key: "s", ctrlKey: true });
    await screen.findByText("保存済み");

    expect(await readText(inner, "manuscript/02/s01.txt")).toBe("改名のあとに書き足した本文");
    await expect(inner.readDocument("manuscript/01/s01.txt")).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("前の章を消して番号が詰まっても、開いていた章立てと本文は、新しいパスで開いたまま", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);
    await backend.applyChangeSet(
      (
        await backend.planStructureEdit({
          kind: "add_scene",
          chapter: "02",
          before: null,
          scene: {
            title: "岬へ",
            summary: "",
            pov: null,
            characters: [],
            place: null,
            time: null,
            target_chars: null,
          },
        })
      ).change_set,
    );
    await backend.writeDocument("manuscript/02/s01.txt", textDocument("第二章の本文"), null);
    await renderWorkspace(backend);
    await user.click(await screen.findByRole("button", { name: /^岬へ/ }));
    await screen.findByRole("textbox", { name: "manuscript/02/s01.txt" });

    await openRowMenu(user, "雨の匂い", "章を削除");
    const dialog = await screen.findByRole("dialog", { name: "削除の確認" });
    await within(dialog).findByText("第1章「雨の匂い」をゴミ箱へ移します。");
    await user.click(within(dialog).getByRole("button", { name: "ゴミ箱へ移す" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    expect(await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" })).toHaveValue(
      "第二章の本文",
    );
    expect(useWorkspaceStore.getState().currentPath).toBe("manuscript/01/s01.txt");
  });
});

describe("生成のセッションが落ち着いていない間", () => {
  it("生成している間は、章の追加・削除・並べ替えも無効にし、理由を title に出す", async () => {
    const user = userEvent.setup();
    const backend = wrapBackend(createMockBackend({ delayMs: 0 }), {
      generate: () => new Promise(() => {}),
    });
    await renderWorkspace(backend);

    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByRole("button", { name: "中止" });

    const addChapter = screen.getByRole("button", { name: "章を追加" });
    expect(addChapter).toBeDisabled();
    expect(addChapter).toHaveAttribute(
      "title",
      "生成している間は、追加・削除・並べ替えできません。",
    );
    await user.click(screen.getByRole("button", { name: "「雨の匂い」の章立ての操作" }));
    for (const item of ["この前に章を追加", "この後に章を追加", "下へ移す", "削除"]) {
      expect(screen.getByRole("menuitem", { name: item })).toBeDisabled();
    }
  });
});
