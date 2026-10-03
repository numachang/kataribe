import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import type { ChangeSet, Task } from "../../../api/types";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { resetAllStores } from "../../../test/resetStores";
import { renderWorkspace } from "../../../test/structureUi";
import { wrapBackend } from "../../../test/wrapBackend";

// 追加のダイアログの「AI に作らせる」。生成のセッション（偽バックエンドの generate）まで通して確かめる。

beforeEach(resetAllStores);
afterEach(resetAllStores);

type User = ReturnType<typeof userEvent.setup>;

/** 生成の呼び出しを記録する。`implementation` で、生成のふるまいを差し替えられる。 */
function recordingGenerate(
  inner: Backend,
  implementation: Backend["generate"] = inner.generate.bind(inner),
) {
  const generate = vi.fn(implementation);
  return { backend: wrapBackend(inner, { generate }), generate };
}

function generatedTasks(generate: { mock: { calls: Array<[string, Task, unknown]> } }): Task[] {
  return generate.mock.calls.map(([, task]) => task);
}

async function openAddDialog(user: User, buttonName: string, dialogTitle: string) {
  await user.click(screen.getByRole("button", { name: buttonName }));
  return screen.findByRole("dialog", { name: dialogTitle });
}

const openAddCharacterDialog = (user: User) => openAddDialog(user, "人物を追加", "人物を追加");
const openAddWorldDocumentDialog = (user: User) =>
  openAddDialog(user, "資料を追加", "世界観の資料を追加");

async function chooseGenerated(user: User, dialog: HTMLElement) {
  await user.click(within(dialog).getByRole("radio", { name: "AI に作らせる" }));
}

describe("切り替え", () => {
  it("開いたときは「自分で書く」で、指示の欄は出ない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));

    const dialog = await openAddCharacterDialog(user);

    expect(within(dialog).getByRole("radio", { name: "自分で書く" })).toBeChecked();
    expect(within(dialog).getByRole("radio", { name: "AI に作らせる" })).not.toBeChecked();
    expect(within(dialog).queryByRole("textbox", { name: "指示" })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "追加" })).toBeInTheDocument();
  });

  it("「AI に作らせる」を選ぶと、指示の欄と開始のボタンに替わる", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddCharacterDialog(user);

    await chooseGenerated(user, dialog);

    expect(within(dialog).getByRole("textbox", { name: "指示" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "生成を始める" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "追加" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "名前" })).not.toBeInTheDocument();
    expect(
      within(dialog).getByText("AI パネルで進み具合を見て、変更案を確かめてから適用します。"),
    ).toBeInTheDocument();
  });

  it("矢印キーでも切り替えられ、自分で書いた内容は切り替えても残る", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddCharacterDialog(user);
    await user.type(within(dialog).getByRole("textbox", { name: "名前" }), "新山 ゆき");

    within(dialog).getByRole("radio", { name: "自分で書く" }).focus();
    await user.keyboard("{ArrowRight}");
    expect(within(dialog).getByRole("radio", { name: "AI に作らせる" })).toBeChecked();
    await user.keyboard("{ArrowLeft}");

    expect(within(dialog).getByRole("textbox", { name: "名前" })).toHaveValue("新山 ゆき");
  });

  it("世界観の資料のダイアログでも、AI のときはファイル名の欄が残り、題と本文は出ない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddWorldDocumentDialog(user);

    await chooseGenerated(user, dialog);

    expect(
      within(dialog).getByRole("textbox", { name: "ファイル名（英数字。空欄なら自動）" }),
    ).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "題" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "本文" })).not.toBeInTheDocument();
  });
});

describe("人物を AI に作らせる", () => {
  it("指示が空（空白だけ）の間は始められない", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddCharacterDialog(user);
    await chooseGenerated(user, dialog);
    const start = within(dialog).getByRole("button", { name: "生成を始める" });
    expect(start).toBeDisabled();

    await user.type(within(dialog).getByRole("textbox", { name: "指示" }), "  ");
    expect(start).toBeDisabled();

    await user.type(within(dialog).getByRole("textbox", { name: "指示" }), "幼なじみ");
    expect(start).toBeEnabled();
  });

  it("始めると、指示（前後の空白を除く）を持つ生成が始まり、ダイアログが閉じ、AI パネルに進み具合が出る", async () => {
    const user = userEvent.setup();
    const { backend, generate } = recordingGenerate(createMockBackend({ delayMs: 5 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    await chooseGenerated(user, dialog);
    await user.type(
      within(dialog).getByRole("textbox", { name: "指示" }),
      "  主人公の幼なじみ。口は悪い。 ",
    );

    await user.click(within(dialog).getByRole("button", { name: "生成を始める" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(generatedTasks(generate)).toEqual([
      { kind: "add_character", instruction: "主人公の幼なじみ。口は悪い。" },
    ]);
    expect(await screen.findByText("AI に作らせて追加: 人物")).toBeInTheDocument();
  });

  it("指示の欄で Enter を押しても始まらず、改行になる（日本語入力の確定の Enter で始めないため）", async () => {
    const user = userEvent.setup();
    const { backend, generate } = recordingGenerate(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    await chooseGenerated(user, dialog);
    const instruction = within(dialog).getByRole("textbox", { name: "指示" });

    await user.type(instruction, "幼なじみ{Enter}口は悪い");

    expect(instruction).toHaveValue("幼なじみ\n口は悪い");
    expect(generate).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("生成して適用すると、作った人物資料が目次に出て、開く", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    await renderWorkspace(inner);
    const dialog = await openAddCharacterDialog(user);
    await chooseGenerated(user, dialog);
    await user.type(within(dialog).getByRole("textbox", { name: "指示" }), "主人公の幼なじみ");
    await user.click(within(dialog).getByRole("button", { name: "生成を始める" }));

    await user.click(await screen.findByRole("button", { name: "適用" }));

    await waitFor(() =>
      expect(useWorkspaceStore.getState().currentPath).toBe("characters/character.md"),
    );
    expect(await screen.findByRole("button", { name: /^高橋 美咲/ })).toBeInTheDocument();
    expect(await screen.findByRole("textbox", { name: "characters/character.md" })).toBeTruthy();
  });
});

describe("世界観の資料を AI に作らせる", () => {
  it("ファイル名と指示を持つ生成が始まる。ファイル名が空欄なら null になる", async () => {
    const user = userEvent.setup();
    const { backend, generate } = recordingGenerate(createMockBackend({ delayMs: 5 }));
    await renderWorkspace(backend);

    const dialog = await openAddWorldDocumentDialog(user);
    await chooseGenerated(user, dialog);
    await user.type(within(dialog).getByRole("textbox", { name: "指示" }), "天気の言い伝え");
    await user.click(within(dialog).getByRole("button", { name: "生成を始める" }));
    await waitFor(() => expect(generate).toHaveBeenCalledTimes(1));
    expect(generatedTasks(generate)[0]).toEqual({
      kind: "add_world_document",
      name: null,
      instruction: "天気の言い伝え",
    });
    expect(await screen.findByText("AI に作らせて追加: 世界観の資料")).toBeInTheDocument();
    await screen.findByRole("button", { name: "適用" });
    await user.click(screen.getByRole("button", { name: "破棄" }));

    const second = await openAddWorldDocumentDialog(user);
    await chooseGenerated(user, second);
    await user.type(
      within(second).getByRole("textbox", { name: "ファイル名（英数字。空欄なら自動）" }),
      "weather",
    );
    await user.type(within(second).getByRole("textbox", { name: "指示" }), "天気の言い伝え");
    await user.click(within(second).getByRole("button", { name: "生成を始める" }));

    await waitFor(() => expect(generate).toHaveBeenCalledTimes(2));
    expect(generatedTasks(generate)[1]).toEqual({
      kind: "add_world_document",
      name: "weather",
      instruction: "天気の言い伝え",
    });
  });

  it("使えない名前なら、AI パネルに失敗が出て、ダイアログは閉じている", async () => {
    const user = userEvent.setup();
    await renderWorkspace(createMockBackend({ delayMs: 0 }));
    const dialog = await openAddWorldDocumentDialog(user);
    await chooseGenerated(user, dialog);
    await user.type(
      within(dialog).getByRole("textbox", { name: "ファイル名（英数字。空欄なら自動）" }),
      "overview",
    );
    await user.type(within(dialog).getByRole("textbox", { name: "指示" }), "天気の言い伝え");
    await user.click(within(dialog).getByRole("button", { name: "生成を始める" }));

    expect(await screen.findByText("生成に失敗しました")).toBeInTheDocument();
    expect(screen.getByText(/ファイル名「overview」は使えません/)).toBeInTheDocument();
  });
});

describe("始められないとき", () => {
  it("目次を変更している間は、始められず、その理由を出す", async () => {
    const user = userEvent.setup();
    const { backend, generate } = recordingGenerate(createMockBackend({ delayMs: 0 }));
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    await chooseGenerated(user, dialog);
    await user.type(within(dialog).getByRole("textbox", { name: "指示" }), "幼なじみ");

    act(() => useWorkspaceStore.getState().beginStructureEdit());

    expect(within(dialog).getByRole("button", { name: "生成を始める" })).toBeDisabled();
    expect(within(dialog).getByRole("status")).toHaveTextContent(
      "目次を変更している間は、AI に作らせて追加できません。",
    );
    expect(generate).not.toHaveBeenCalled();

    act(() => useWorkspaceStore.getState().endStructureEdit());
    expect(within(dialog).getByRole("button", { name: "生成を始める" })).toBeEnabled();
    expect(within(dialog).queryByRole("status")).not.toBeInTheDocument();
  });

  it("ほかの生成が進んでいる間は、始められず、その理由を出す", async () => {
    const user = userEvent.setup();
    const inner = createMockBackend({ delayMs: 0 });
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { backend, generate } = recordingGenerate(inner, async (jobId, task, onEvent) => {
      await gate;
      return inner.generate(jobId, task, onEvent);
    });
    await renderWorkspace(backend);
    const dialog = await openAddCharacterDialog(user);
    await chooseGenerated(user, dialog);
    await user.type(within(dialog).getByRole("textbox", { name: "指示" }), "幼なじみ");

    // ダイアログを開いたまま、AI パネルから工程の生成を始める
    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));

    await waitFor(() =>
      expect(within(dialog).getByRole("status")).toHaveTextContent(
        "生成している間は、AI に作らせて追加できません。",
      ),
    );
    expect(within(dialog).getByRole("button", { name: "生成を始める" })).toBeDisabled();
    await act(async () => release());
    await screen.findByRole("button", { name: "適用" });
    expect(generate).toHaveBeenCalledTimes(1);
    expect(generatedTasks(generate)[0]?.kind).not.toBe("add_character");
  });

  it("変更案の確認の間は目次の「＋」が押せないので、ダイアログを開けない", async () => {
    const user = userEvent.setup();
    const changeSet: ChangeSet = {
      summary: "確認待ちの変更案",
      files: [],
      project_root: "",
    };
    const inner = createMockBackend({ delayMs: 0 });
    await renderWorkspace(wrapBackend(inner, { generate: async () => changeSet }));
    await user.click(screen.getByRole("button", { name: "次の工程を実行" }));
    await screen.findByText("確認待ちの変更案");

    expect(screen.getByRole("button", { name: "人物を追加" })).toBeDisabled();
  });
});
