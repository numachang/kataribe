import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Backend } from "../../api/backend";
import { BackendProvider } from "../../api/context";
import { createMockBackend } from "../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../api/mock/sampleProject";
import type { OverviewEntry, ProjectOverview } from "../../api/types";
import { GenerationSessionProvider } from "../../features/generation/GenerationSessionProvider";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { resetAllStores } from "../../test/resetStores";
import { ProjectTree } from "./ProjectTree";

beforeEach(resetAllStores);
afterEach(resetAllStores);

function renderTree(backend: Backend = createMockBackend({ delayMs: 0 })) {
  return render(
    <BackendProvider backend={backend}>
      <GenerationSessionProvider>
        <ProjectTree />
      </GenerationSessionProvider>
    </BackendProvider>,
  );
}

function leaf(overrides: Partial<OverviewEntry>): OverviewEntry {
  return {
    path: null,
    label: "項目",
    kind: "other",
    chapter: null,
    scene: null,
    exists: true,
    chars: 0,
    target_chars: null,
    error: null,
    children: [],
    ...overrides,
  };
}

describe("目次の文字数表示", () => {
  it("目標文字数が 0 のシーンは分母を出さない（8 / 0 字にならない）", () => {
    const overview: ProjectOverview = {
      root: "C:\\test",
      title: "テスト作品",
      target_length: 30000,
      total_chars: 8,
      sections: [
        {
          kind: "manuscript",
          label: "本文",
          entries: [
            leaf({
              path: "manuscript/01/s01.txt",
              label: "シーン",
              kind: "scene",
              chars: 8,
              target_chars: 0,
            }),
          ],
        },
      ],
    };
    useWorkspaceStore.getState().openWorkspace(overview);
    renderTree();

    expect(screen.getByText("8 字")).toBeInTheDocument();
    expect(screen.queryByText(/\/ 0 字/)).not.toBeInTheDocument();
  });

  it("作品全体の目標文字数が 0 のときも分母を出さない", () => {
    const overview: ProjectOverview = {
      root: "C:\\test",
      title: "テスト作品",
      target_length: 0,
      total_chars: 120,
      sections: [],
    };
    useWorkspaceStore.getState().openWorkspace(overview);
    renderTree();

    expect(screen.getByText("120 字")).toBeInTheDocument();
    expect(screen.queryByText(/\/ 0 字/)).not.toBeInTheDocument();
  });
});

async function openSampleProject(backend: Backend): Promise<void> {
  useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
}

/** 行の操作メニューを開き、項目の名前を返す。章立ての行は `kind` に「章立ての」を渡す。 */
async function openMenuOf(label: string, kind = ""): Promise<string[]> {
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: `「${label}」の${kind}操作` }));
  return within(screen.getByRole("menu"))
    .getAllByRole("menuitem")
    .map((item) => item.textContent ?? "");
}

describe("目次の行のメニュー", () => {
  it("人物の行には、並べ替えと削除が出る。先頭には「上へ」、末尾には「下へ」を出さない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderTree(backend);

    expect(await openMenuOf("霧島 凛")).toEqual(["下へ移す", "削除"]);
    expect(await openMenuOf("佐藤 健二")).toEqual(["上へ移す", "削除"]);
  });

  it("本文の章見出しには、シーンの追加と章の並べ替えと削除が出る", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderTree(backend);

    expect(await openMenuOf("雨の匂い")).toEqual(["シーンを追加", "章を下へ移す", "章を削除"]);
    expect(await openMenuOf("灯台のある岬")).toEqual(["シーンを追加", "章を上へ移す", "章を削除"]);
    // 同じ「雨の匂い」でも、章立てのファイルの行（プロットの節）は別のメニュー（「章立て」の操作）
    expect(screen.getAllByRole("button", { name: "「雨の匂い」の操作" })).toHaveLength(1);
  });

  it("章立てのファイルの行には、この前・この後に章を追加と、並べ替えと削除が出る", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderTree(backend);

    expect(await openMenuOf("雨の匂い", "章立ての")).toEqual([
      "この前に章を追加",
      "この後に章を追加",
      "下へ移す",
      "削除",
    ]);
    expect(await openMenuOf("灯台のある岬", "章立ての")).toEqual([
      "この前に章を追加",
      "この後に章を追加",
      "上へ移す",
      "削除",
    ]);
  });

  it("シーンの行には、前に追加・後に追加・並べ替え・削除が出る（先頭と末尾は片側だけ）", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderTree(backend);

    expect(await openMenuOf("招かれざる客")).toEqual([
      "この前にシーンを追加",
      "この後にシーンを追加",
      "下へ移す",
      "削除",
    ]);
    expect(await openMenuOf("遺言状の間")).toEqual([
      "この前にシーンを追加",
      "この後にシーンを追加",
      "上へ移す",
      "下へ移す",
      "削除",
    ]);
    expect(await openMenuOf("消えた甥")).toEqual([
      "この前にシーンを追加",
      "この後にシーンを追加",
      "上へ移す",
      "削除",
    ]);
  });

  it("足した世界観の資料の行には削除が出て、世界観の概要には何も出ない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    const plan = await backend.planStructureEdit({
      kind: "add_world_document",
      name: "glossary",
      title: "用語集",
      body: "",
    });
    useWorkspaceStore.getState().setOverview(await backend.applyChangeSet(plan.change_set));
    renderTree(backend);

    expect(await openMenuOf("用語集")).toEqual(["削除"]);
    expect(screen.queryByRole("button", { name: "「世界観」の操作" })).not.toBeInTheDocument();
  });

  it("企画・あらすじなど、足したり消したりできない項目には、メニューを出さない", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderTree(backend);

    for (const label of ["作品情報", "企画", "文体", "あらすじ"]) {
      expect(screen.queryByRole("button", { name: `「${label}」の操作` })).not.toBeInTheDocument();
    }
  });

  it("世界観・登場人物・あらすじと章立ての節の見出しに、追加の「＋」が付く", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await openSampleProject(backend);
    renderTree(backend);

    expect(screen.getByRole("button", { name: "資料を追加" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "人物を追加" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "章を追加" })).toBeEnabled();
    expect(screen.getAllByRole("button", { name: /を追加$/ })).toHaveLength(3);
  });
});
