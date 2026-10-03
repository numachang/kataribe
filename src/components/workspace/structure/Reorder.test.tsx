import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BackendError } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import type { OverviewEntry } from "../../../api/types";
import { useEditorStore } from "../../../store/editorStore";
import { useUiStore } from "../../../store/uiStore";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { readText } from "../../../test/documents";
import { resetAllStores } from "../../../test/resetStores";
import { hasToast, openRowMenu, renderWorkspace } from "../../../test/structureUi";
import { wrapBackend } from "../../../test/wrapBackend";

// 人物・章・シーンの並べ替えの画面。確認のダイアログは挟まず、メニューの「上へ移す」「下へ移す」でそのまま反映する。
// サンプル作品は、人物 2 人（霧島 凛・佐藤 健二）、第 1 章「雨の匂い」（シーン 3 つ。本文は 2 つ）、
// 第 2 章「灯台のある岬」（シーン構成なし）を持つ。

beforeEach(resetAllStores);
afterEach(resetAllStores);

const PLAN_MENU = "章立ての";

function sectionOf(sectionLabel: string): HTMLElement {
  const section = within(screen.getByRole("navigation"))
    .getByRole("heading", { name: sectionLabel })
    .closest("section");
  if (section === null) {
    throw new Error(`「${sectionLabel}」の節があるはず`);
  }
  return section;
}

/** 目次の節の、いちばん上の階層の項目の題（本文の節なら章見出し）。 */
function topLevelLabels(sectionLabel: string): string[] {
  return Array.from(
    sectionOf(sectionLabel).querySelectorAll(
      ":scope > ul > li > .project-tree__row .project-tree__label",
    ),
  ).map((label) => label.textContent ?? "");
}

/** 本文の節の、シーンの題（章をまたいで並びの順）。 */
function sceneLabels(): string[] {
  return Array.from(
    sectionOf("本文").querySelectorAll(".project-tree__children .project-tree__label"),
  ).map((label) => label.textContent ?? "");
}

const characterLabels = () => topLevelLabels("登場人物");
const plannedLabels = () =>
  topLevelLabels("あらすじ・章立て").filter((label) => label !== "あらすじ");

describe("人物を並べ替える", () => {
  it("「下へ移す」で、確認なしに順番が入れ替わり、人物資料の順番の項目が書き直され、トーストで知らせる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await openRowMenu(user, "霧島 凛", "下へ移す");

    await waitFor(() => expect(characterLabels()).toEqual(["佐藤 健二", "霧島 凛"]));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(hasToast("人物「霧島 凛」を 2 番目に移しました。")).toBe(true);
    const { document } = await backend.readDocument("characters/sato-kenji.md");
    expect(document).toMatchObject({ kind: "character", meta: { order: 1 } });
  });

  it("入れ替えたあとのメニューは、新しい並びに合う（先頭だった人物は末尾になり、「上へ」だけが出る）", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    await openRowMenu(user, "霧島 凛", "下へ移す");
    await waitFor(() => expect(characterLabels()).toEqual(["佐藤 健二", "霧島 凛"]));

    await user.click(screen.getByRole("button", { name: "「霧島 凛」の操作" }));

    expect(
      within(screen.getByRole("menu"))
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual(["上へ移す", "削除"]);
  });

  it("開いている人物資料は、書き換えられた順番を読み直して見せる", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    await user.click(screen.getByRole("button", { name: /^霧島 凛/ }));
    await screen.findByRole("textbox", { name: "characters/kirishima-rin.md" });
    expect(screen.getByLabelText("順番")).toHaveDisplayValue("1");

    await openRowMenu(user, "霧島 凛", "下へ移す");

    await waitFor(() => expect(screen.getByLabelText("順番")).toHaveDisplayValue("2"));
  });

  it("開いている人物資料に保存できない編集が残っていれば、並べ替えずに知らせる", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const applyChangeSet = vi.fn(inner.applyChangeSet.bind(inner));
    const backend = wrapBackend(inner, {
      applyChangeSet,
      async writeDocument() {
        throw new BackendError("io", "ディスクに書けません。");
      },
    });
    await renderWorkspace(backend);
    await user.click(screen.getByRole("button", { name: /^霧島 凛/ }));
    await screen.findByRole("textbox", { name: "characters/kirishima-rin.md" });
    fireEvent.change(screen.getByLabelText("名前"), { target: { value: "霧島 凛子" } });

    await openRowMenu(user, "霧島 凛", "下へ移す");

    await waitFor(() =>
      expect(hasToast("保存できていない編集があるため、変更しませんでした。")).toBe(true),
    );
    expect(applyChangeSet).not.toHaveBeenCalled();
    expect(characterLabels()).toEqual(["霧島 凛", "佐藤 健二"]);
  });
});

describe("シーンを並べ替える", () => {
  it("「下へ移す」で、章立ての順が入れ替わり、本文はそのまま。トーストで知らせる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);
    const draftBefore = await readText(backend, "manuscript/01/s01.txt");

    await openRowMenu(user, "招かれざる客", "下へ移す");

    await waitFor(() =>
      expect(sceneLabels()).toEqual([
        "遺言状の間",
        "招かれざる客",
        "消えた甥",
        "シーン構成が未生成です",
      ]),
    );
    expect(hasToast("第1章のシーン「招かれざる客」を 2 番目に移しました。")).toBe(true);
    expect(await readText(backend, "manuscript/01/s01.txt")).toBe(draftBefore);
  });

  it("本文を開いて編集中のシーンを動かしても、本文は開いたまま続けて編集できる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);
    await user.click(await screen.findByRole("button", { name: /^招かれざる客/ }));
    await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" });

    await openRowMenu(user, "招かれざる客", "下へ移す");
    await waitFor(() => expect(hasToast("2 番目に移しました。")).toBe(true));

    const editor = await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" });
    fireEvent.change(editor, { target: { value: "動かしたあとに書いた本文" } });
    fireEvent.keyDown(editor, { key: "s", ctrlKey: true });
    await screen.findByText("保存済み");
    expect(await readText(backend, "manuscript/01/s01.txt")).toBe("動かしたあとに書いた本文");
  });
});

describe("章を並べ替える", () => {
  it("章立ての行の「下へ移す」で、章立ても本文の章見出しも入れ替わり、トーストで知らせる", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 0 });
    await renderWorkspace(backend);

    await openRowMenu(user, "雨の匂い", "下へ移す", PLAN_MENU);

    await waitFor(() => expect(plannedLabels()).toEqual(["灯台のある岬", "雨の匂い"]));
    expect(topLevelLabels("本文")).toEqual(["灯台のある岬", "雨の匂い"]);
    expect(hasToast("第1章「雨の匂い」を第2章へ移しました。")).toBe(true);
    expect(await readText(backend, "manuscript/02/s01.txt")).toContain("館の扉が開くたび");
    await expect(backend.readDocument("manuscript/01/s01.txt")).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("本文の章見出しの「章を上へ移す」でも並べ替えられる", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "灯台のある岬", "章を上へ移す");

    await waitFor(() => expect(plannedLabels()).toEqual(["灯台のある岬", "雨の匂い"]));
    expect(hasToast("第2章「灯台のある岬」を第1章へ移しました。")).toBe(true);
  });

  it("開いていた本文は、改名されても読み直さずに続けて編集でき、新しいパスへ保存する", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const readDocument = vi.fn(inner.readDocument.bind(inner));
    const backend = wrapBackend(inner, { readDocument });
    await renderWorkspace(backend);
    await user.click(await screen.findByRole("button", { name: /^招かれざる客/ }));
    await screen.findByRole("textbox", { name: "manuscript/01/s01.txt" });
    const readsBefore = readDocument.mock.calls.length;

    await openRowMenu(user, "雨の匂い", "下へ移す", PLAN_MENU);

    const editor = await screen.findByRole("textbox", { name: "manuscript/02/s01.txt" });
    expect(useWorkspaceStore.getState().currentPath).toBe("manuscript/02/s01.txt");
    expect(useEditorStore.getState().path).toBe("manuscript/02/s01.txt");
    expect(readDocument.mock.calls.length).toBe(readsBefore);

    fireEvent.change(editor, { target: { value: "入れ替えのあとに書き足した本文" } });
    fireEvent.keyDown(editor, { key: "s", ctrlKey: true });
    await screen.findByText("保存済み");
    expect(await readText(inner, "manuscript/02/s01.txt")).toBe("入れ替えのあとに書き足した本文");
    await expect(inner.readDocument("manuscript/01/s01.txt")).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("開いていた章立ても、改名されたパスで開いたまま", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    await user.click(screen.getAllByRole("button", { name: /^雨の匂い/ })[0] as HTMLElement);
    await screen.findByRole("textbox", { name: "plot/chapters/01.md" });

    await openRowMenu(user, "雨の匂い", "下へ移す", PLAN_MENU);

    expect(await screen.findByRole("textbox", { name: "plot/chapters/02.md" })).toBeInTheDocument();
    expect(useWorkspaceStore.getState().currentPath).toBe("plot/chapters/02.md");
  });
});

describe("並べ替えの失敗と、操作できない間", () => {
  it("失敗したら、理由をエラーのトーストで知らせ、目次は変えない", async () => {
    const user = userEvent.setup();
    const backend = wrapBackend(createMockBackend({ delayMs: 0 }), {
      async applyChangeSet() {
        throw new BackendError(
          "conflict",
          "作品が確かめたあとに変更されたため、適用しませんでした。",
        );
      },
    });
    await renderWorkspace(backend);

    await openRowMenu(user, "霧島 凛", "下へ移す");

    await waitFor(() => expect(hasToast("適用しませんでした。")).toBe(true));
    expect(useUiStore.getState().toasts.every((toast) => toast.kind === "error")).toBe(true);
    expect(characterLabels()).toEqual(["霧島 凛", "佐藤 健二"]);
  });

  it("動かしている間は、続けて操作できない（古い目次の位置で動かさないため）", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const backend = wrapBackend(inner, {
      async applyChangeSet(changeSet) {
        await gate;
        return inner.applyChangeSet(changeSet);
      },
    });
    await renderWorkspace(backend);

    await openRowMenu(user, "霧島 凛", "下へ移す");
    await user.click(screen.getByRole("button", { name: "「佐藤 健二」の操作" }));

    const item = screen.getByRole("menuitem", { name: "上へ移す" });
    expect(item).toBeDisabled();
    expect(item).toHaveAttribute(
      "title",
      "目次を変更している間は、追加・削除・並べ替えできません。",
    );
    release();
    await waitFor(() => expect(characterLabels()).toEqual(["佐藤 健二", "霧島 凛"]));
  });

  it("失敗したあとは、また操作できる（動かしている間の印が残らない）", async () => {
    const user = userEvent.setup();
    const backend = wrapBackend(createMockBackend({ delayMs: 0 }), {
      async applyChangeSet() {
        throw new BackendError("conflict", "適用しませんでした。");
      },
    });
    await renderWorkspace(backend);
    await openRowMenu(user, "霧島 凛", "下へ移す");
    await waitFor(() => expect(hasToast("適用しませんでした。")).toBe(true));

    await user.click(screen.getByRole("button", { name: "「佐藤 健二」の操作" }));

    expect(screen.getByRole("menuitem", { name: "上へ移す" })).toBeEnabled();
    expect(useWorkspaceStore.getState().activeStructureEdits).toBe(0);
  });
});

describe("並べ替えている間の生成", () => {
  // 並べ替えの適用を止めておき、その間に生成を始められないことと、終われば始められることを確かめる
  function gatedBackend() {
    const inner = createMockBackend({ delayMs: 0 });
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const backend = wrapBackend(inner, {
      async applyChangeSet(changeSet) {
        await gate;
        return inner.applyChangeSet(changeSet);
      },
    });
    return { backend, release: () => release() };
  }

  const BLOCKED_REASON = "目次を変更している間は、生成できません。";

  it("「工程」タブの生成・「次の工程を実行」・「自動で進める」は、並べ替えが終わるまで押せない", async () => {
    const user = userEvent.setup();
    const { backend, release } = gatedBackend();
    await renderWorkspace(backend);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "次の工程を実行" })).toBeEnabled(),
    );

    await openRowMenu(user, "霧島 凛", "下へ移す");

    for (const name of ["次の工程を実行", "自動で進める"]) {
      const button = screen.getByRole("button", { name });
      expect(button).toBeDisabled();
      expect(button).toHaveAttribute("title", BLOCKED_REASON);
    }
    for (const button of screen.getAllByRole("button", { name: "生成" })) {
      expect(button).toBeDisabled();
    }
    release();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "次の工程を実行" })).toBeEnabled(),
    );
    expect(screen.getByRole("button", { name: "自動で進める" })).toBeEnabled();
  });

  it("まだ無い本文の「空の本文から書き始める」と「この文書」タブの生成は、並べ替えが終わるまで押せない", async () => {
    const user = userEvent.setup();
    const { backend, release } = gatedBackend();
    await renderWorkspace(backend);
    await user.click(await screen.findByRole("button", { name: /^消えた甥/ }));
    await user.click(screen.getByRole("tab", { name: "この文書" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "本文を生成" })).toBeEnabled());

    await openRowMenu(user, "霧島 凛", "下へ移す");

    const startWriting = screen.getByRole("button", { name: "空の本文から書き始める" });
    expect(startWriting).toBeDisabled();
    expect(startWriting).toHaveAttribute("title", "目次を変更している間は、本文を作成できません。");
    const generate = screen.getByRole("button", { name: "本文を生成" });
    expect(generate).toBeDisabled();
    expect(generate).toHaveAttribute("title", BLOCKED_REASON);
    release();
    await waitFor(() => expect(startWriting).toBeEnabled());
    expect(generate).toBeEnabled();
  });
});

describe("並べ替えの注意書き", () => {
  it("変更案に注意書きがあれば、適用の要約に続けて知らせる", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const backend = wrapBackend(inner, {
      async planStructureEdit(edit) {
        const plan = await inner.planStructureEdit(edit);
        return { ...plan, notices: ["読めない人物資料は、順番を変えずに後ろへ並べたままです。"] };
      },
    });
    await renderWorkspace(backend);

    await openRowMenu(user, "霧島 凛", "下へ移す");

    await waitFor(() => expect(hasToast("人物「霧島 凛」を 2 番目に移しました。")).toBe(true));
    const messages = useUiStore.getState().toasts.map((toast) => toast.message);
    expect(messages).toEqual([
      "人物「霧島 凛」を 2 番目に移しました。",
      "読めない人物資料は、順番を変えずに後ろへ並べたままです。",
    ]);
  });

  it("注意書きが無ければ、要約のトーストだけを出す", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    await openRowMenu(user, "霧島 凛", "下へ移す");

    await waitFor(() => expect(hasToast("2 番目に移しました。")).toBe(true));
    expect(useUiStore.getState().toasts).toHaveLength(1);
  });
});

describe("読めない章立てが混じる目次", () => {
  it("本文の章見出しからの章の並べ替えは、読めない章立ても数えた位置を送る", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    const planStructureEdit = vi.fn(async () => {
      throw new BackendError("invalid_input", "このテストでは適用しません。");
    });
    const backend = wrapBackend(inner, {
      // 本物の目次と同じく、本文の節は読めない章（00）を出さず、プロットの節は出す
      async overview() {
        const overview = await inner.overview();
        const sections = overview.sections.map((section) =>
          section.kind === "plot"
            ? {
                ...section,
                entries: [
                  ...section.entries.slice(0, 1),
                  {
                    ...(section.entries[1] as OverviewEntry),
                    path: "plot/chapters/00.md",
                    label: "壊れた章",
                    chapter: "00",
                    error: "YAML を読めません",
                  },
                  ...section.entries.slice(1),
                ],
              }
            : section,
        );
        return { ...overview, sections };
      },
      planStructureEdit,
    });
    await renderWorkspace(backend);
    // 章 00（読めない）・01・02 のうち、本文の節に出ているのは 01 と 02
    useWorkspaceStore.getState().setOverview(await backend.overview());

    await openRowMenu(user, "灯台のある岬", "章を上へ移す");

    await waitFor(() =>
      expect(planStructureEdit).toHaveBeenCalledWith({
        kind: "move_chapter",
        chapter: "02",
        position: 1,
      }),
    );
  });
});
