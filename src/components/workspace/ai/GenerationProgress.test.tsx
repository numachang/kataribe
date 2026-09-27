import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import { SAMPLE_PROJECT_FOLDER } from "../../../api/mock/sampleProject";
import type { GenerationStepDisplay } from "../../../features/generation/eventAccumulator";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { renderWithBackend } from "../../../test/renderWithBackend";
import { resetAllStores } from "../../../test/resetStores";
import { WorkspaceScreen } from "../WorkspaceScreen";
import { describeFinishedStep, ElapsedSeconds } from "./GenerationProgress";

beforeEach(resetAllStores);
afterEach(resetAllStores);

async function createEmptyProject(backend: Backend): Promise<void> {
  const overview = await backend.createProject("C:\\projects\\test-thinking", {
    title: "テスト作品",
    author: null,
    genre: "fantasy",
    genre_note: null,
    rating: "general",
    target_length: 10000,
    idea: "旅する少女の物語",
  });
  useWorkspaceStore.getState().openWorkspace(overview);
  await useWorkspaceStore.getState().refreshPipeline(backend);
}

describe("生成中の「考え中…」表示", () => {
  it("思考の断片だけが届いている間は「考え中…」と分かり、本文が流れ始めると普通の見出しに戻る", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 30 });
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "次の工程を実行" }));

    expect(
      await screen.findByText("考え中…（思考過程）", {}, { timeout: 2000 }),
    ).toBeInTheDocument();

    await waitFor(
      () => {
        expect(screen.getByText("思考過程")).toBeInTheDocument();
      },
      { timeout: 2000 },
    );
  });
});

describe("生成中の進み具合", () => {
  it("使っている LLM と、工程全体のうち済んだ数を出す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 30 });
    await createEmptyProject(backend);
    const total = useWorkspaceStore.getState().pipeline.length;
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "次の工程を実行" }));

    expect(
      await screen.findByText(
        "使う LLM: OpenAI 互換 API（localhost:1234）・サーバーの既定のモデル",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(`工程 0/${total} 済み`)).toBeInTheDocument();
  });

  it("生成中の回は経過時間を数え、終わった回はかかった時間とトークン数を出す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 30 });
    // サンプル作品の次の工程は、3 回に分かれる本文の生成（終わった回が、次の回の間も見える）
    useWorkspaceStore.getState().openWorkspace(await backend.openProject(SAMPLE_PROJECT_FOLDER));
    await useWorkspaceStore.getState().refreshPipeline(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "次の工程を実行" }));

    expect(await screen.findByText(/^経過 \d+ 秒・/)).toBeInTheDocument();
    expect(
      await screen.findByText(
        /^完了（.+ 秒、入力 .+ トークン・出力 .+ トークン）$/,
        {},
        { timeout: 3000 },
      ),
    ).toBeInTheDocument();
  });
});

describe("describeFinishedStep", () => {
  const finished: GenerationStepDisplay = {
    label: "企画を生成",
    index: 1,
    total: 1,
    startedAt: 0,
    content: "企画",
    reasoning: "",
    notices: [],
    finished: true,
    promptTokens: 1235,
    completionTokens: 1853,
    elapsedMs: 23_150,
  };

  it("かかった時間とトークン数を、桁区切りで出す", () => {
    expect(describeFinishedStep(finished)).toBe(
      "完了（23.1 秒、入力 1,235 トークン・出力 1,853 トークン）",
    );
  });

  it("トークン数が届かなかったときは、時間だけを出す", () => {
    expect(describeFinishedStep({ ...finished, promptTokens: null, completionTokens: null })).toBe(
      "完了（23.1 秒）",
    );
  });
});

describe("ElapsedSeconds", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("1 秒ごとに経過秒数を数え直す", () => {
    vi.useFakeTimers();
    const startedAt = Date.now();
    render(<ElapsedSeconds since={startedAt} />);
    expect(document.body.textContent).toBe("経過 0 秒");

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(document.body.textContent).toBe("経過 3 秒");
  });
});
