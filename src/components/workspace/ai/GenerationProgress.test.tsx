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
import {
  describeFinishedStep,
  ElapsedSeconds,
  reasoningSummary,
  type StepState,
  StepStatus,
  stepState,
} from "./GenerationProgress";

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

  it("自動で進める間に LLM の設定を変えると、次の工程から新しい LLM の名前を出す", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ delayMs: 30 });
    await createEmptyProject(backend);
    renderWithBackend(<WorkspaceScreen />, backend);

    await user.click(await screen.findByRole("button", { name: "自動で進める" }));
    expect(
      await screen.findByText(
        "使う LLM: OpenAI 互換 API（localhost:1234）・サーバーの既定のモデル",
      ),
    ).toBeInTheDocument();

    const settings = await backend.loadSettings();
    await backend.saveSettings({
      ...settings,
      llm: { ...settings.llm, provider: "claude_code", claude_model: "haiku" },
    });

    expect(
      await screen.findByText("使う LLM: Claude Code（haiku）", {}, { timeout: 3000 }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "自動で進めるのを止める" }));
  });
});

describe("describeFinishedStep", () => {
  const finished: GenerationStepDisplay = {
    label: "企画を生成",
    index: 1,
    total: 1,
    startedAt: 0,
    content: "企画",
    receivedCharacters: 2,
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

  it("画面から消えたら、数え直すタイマーも止める", () => {
    vi.useFakeTimers();
    const { unmount } = render(<ElapsedSeconds since={Date.now()} />);
    expect(vi.getTimerCount()).toBe(1);

    unmount();

    expect(vi.getTimerCount()).toBe(0);
  });
});

const runningStep: GenerationStepDisplay = {
  label: "本文を生成",
  index: 1,
  total: 1,
  startedAt: Date.now(),
  content: "",
  receivedCharacters: 0,
  reasoning: "",
  notices: [],
  finished: false,
  promptTokens: null,
  completionTokens: null,
  elapsedMs: null,
};

describe("stepState", () => {
  it("終わった回は、最後でなくても、生成が止まっていても終わった回とする", () => {
    const finished = { ...runningStep, finished: true };
    expect(stepState(finished, false, true)).toBe("finished");
    expect(stepState(finished, true, false)).toBe("finished");
  });

  it("終わらないまま次の回が始まった回は、生成が止まっていてもやり直した回とする", () => {
    expect(stepState(runningStep, false, true)).toBe("retried");
    expect(stepState(runningStep, false, false)).toBe("retried");
  });

  it("終わっていない最後の回は、生成が続いていれば進行中、止まっていれば止まった回とする", () => {
    expect(stepState(runningStep, true, true)).toBe("inProgress");
    expect(stepState(runningStep, true, false)).toBe("stopped");
  });
});

describe("StepStatus", () => {
  function statusText(step: GenerationStepDisplay, state: StepState) {
    render(<StepStatus step={step} state={state} />);
    return document.body.textContent;
  }

  it("何も届いていない回は、応答を待っていると出す", () => {
    expect(statusText(runningStep, "inProgress")).toMatch(/^経過 \d+ 秒・応答を待っています$/);
  });

  it("思考だけが届いている回は、考えていると出す", () => {
    expect(statusText({ ...runningStep, reasoning: "構成を検討中" }, "inProgress")).toMatch(
      /^経過 \d+ 秒・考えています$/,
    );
  });

  it("本文が届いている回は、受け取った文字数を桁区切りで出す", () => {
    expect(
      statusText({ ...runningStep, content: "……", receivedCharacters: 12_345 }, "inProgress"),
    ).toMatch(/^経過 \d+ 秒・受け取った文字 12,345 字$/);
  });

  it("止まった回は、途中で止まったと出す", () => {
    expect(statusText(runningStep, "stopped")).toBe("途中で止まりました");
  });

  it("やり直した回は、経過時間を数えずにやり直したと出す", () => {
    expect(statusText(runningStep, "retried")).toBe("やり直しました");
  });
});

describe("reasoningSummary", () => {
  const thinking = { ...runningStep, reasoning: "構成を検討中" };

  it("進行中の回で思考だけが届いているあいだは、考え中と出す", () => {
    expect(reasoningSummary(thinking, "inProgress")).toBe("考え中…（思考過程）");
  });

  it("本文が届き始めたら、普通の見出しに戻す", () => {
    expect(
      reasoningSummary({ ...thinking, content: "本文", receivedCharacters: 2 }, "inProgress"),
    ).toBe("思考過程");
  });

  it.each(["stopped", "retried", "finished"] as const)(
    "進行中でない回（%s）は、本文が無くても考え中と出さない",
    (state) => {
      expect(reasoningSummary(thinking, state)).toBe("思考過程");
    },
  );
});
