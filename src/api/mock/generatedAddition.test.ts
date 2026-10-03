import { describe, expect, it } from "vitest";
import { writeChanges } from "../../test/changeSets";
import { BackendError } from "../backend";
import type { GenerationEvent, Task } from "../types";
import { createMockBackend } from "./backend";
import { SAMPLE_PROJECT_FOLDER } from "./sampleProject";

async function openedBackend() {
  const backend = createMockBackend({ delayMs: 0 });
  await backend.openProject(SAMPLE_PROJECT_FOLDER);
  return backend;
}

type OpenedBackend = Awaited<ReturnType<typeof openedBackend>>;

async function generateWithEvents(backend: OpenedBackend, task: Task) {
  const events: GenerationEvent[] = [];
  const changeSet = await backend.generate("job-add", task, (event) => events.push(event));
  return { changeSet, events };
}

/** 失敗した生成の、エラーと、失敗するまでに届いたイベント。 */
async function failedGeneration(backend: OpenedBackend, task: Task) {
  const events: GenerationEvent[] = [];
  const failure = await backend
    .generate("job-failed", task, (event) => events.push(event))
    .then(
      () => null,
      (error: unknown) => error,
    );
  return { failure, events };
}

function startedSteps(events: GenerationEvent[]) {
  return events.flatMap((event) => (event.kind === "step_started" ? [event] : []));
}

// サンプル作品には「佐藤 健二」がいるので、偽の人物の最初の候補は「高橋 美咲」になる。
describe("偽バックエンド / 指示から人物を作って足す", () => {
  it("新しい人物資料を 1 つ書く変更案と、生成を知らせる要約を返す", async () => {
    const backend = await openedBackend();

    const { changeSet } = await generateWithEvents(backend, {
      kind: "add_character",
      instruction: "主人公の幼なじみ",
    });

    const files = writeChanges(changeSet);
    expect(changeSet.files).toHaveLength(1);
    expect(files[0]?.path).toBe("characters/character.md");
    expect(files[0]?.previous).toBeNull();
    expect(files[0]?.content).toContain("name: 高橋 美咲");
    expect(files[0]?.content).toContain("主人公の幼なじみ");
    expect(changeSet.summary).toBe("人物「高橋 美咲」を生成しました（characters/character.md）。");
  });

  it("項目と本文を 2 回に分けて流し、古くなった文書の注意書きを最後の回のあとに出す", async () => {
    const backend = await openedBackend();

    const { events } = await generateWithEvents(backend, {
      kind: "add_character",
      instruction: "主人公の幼なじみ",
    });

    expect(startedSteps(events).map((event) => [event.index, event.total])).toEqual([
      [1, 2],
      [2, 2],
    ]);
    const notices = events.flatMap((event) => (event.kind === "notice" ? [event] : []));
    expect(notices).toHaveLength(1);
    expect(notices[0]?.level).toBe("info");
    expect(notices[0]?.message).toContain("この人物はまだ出てきません");
    const lastStepStart = events.findLastIndex((event) => event.kind === "step_started");
    expect(events.findIndex((event) => event.kind === "notice")).toBeGreaterThan(lastStepStart);
  });

  it("適用してもう 1 人足すと、名前も ID も先の人物と重ならない", async () => {
    const backend = await openedBackend();
    const first = await backend.generate(
      "job-1",
      { kind: "add_character", instruction: "幼なじみ" },
      () => {},
    );
    await backend.applyChangeSet(first);

    const second = await backend.generate(
      "job-2",
      { kind: "add_character", instruction: "港の見張り番" },
      () => {},
    );

    const files = writeChanges(second);
    expect(files[0]?.path).toBe("characters/character-2.md");
    expect(files[0]?.content).toContain("name: 中村 遼");
  });

  it("指示が空（空白だけ）なら、文字を流す前に invalid_input にする", async () => {
    const backend = await openedBackend();

    const { failure, events } = await failedGeneration(backend, {
      kind: "add_character",
      instruction: " \n　",
    });

    expect(failure).toBeInstanceOf(BackendError);
    expect((failure as BackendError).kind).toBe("invalid_input");
    expect(startedSteps(events)).toEqual([]);
  });
});

describe("偽バックエンド / 指示から世界観の資料を作って足す", () => {
  it("新しい資料を 1 つ書く変更案を返す。ファイル名の指定が無ければ自動で決める", async () => {
    const backend = await openedBackend();

    const { changeSet } = await generateWithEvents(backend, {
      kind: "add_world_document",
      name: null,
      instruction: "港町の天気の言い伝え",
    });

    const files = writeChanges(changeSet);
    expect(changeSet.files).toHaveLength(1);
    expect(files[0]?.path).toBe("world/doc.md");
    expect(files[0]?.previous).toBeNull();
    expect(files[0]?.content.startsWith("# 港町の天気の言い伝え\n")).toBe(true);
    expect(changeSet.summary).toBe(
      "世界観の資料「港町の天気の言い伝え」を生成しました（world/doc.md）。",
    );
  });

  it("ファイル名を指定すれば、その名前で作る", async () => {
    const backend = await openedBackend();

    const { changeSet } = await generateWithEvents(backend, {
      kind: "add_world_document",
      name: "weather",
      instruction: "天気の言い伝え",
    });

    expect(writeChanges(changeSet)[0]?.path).toBe("world/weather.md");
  });

  it("見出しから始まる本文を流し、生成済みの文書には反映されないことを知らせる", async () => {
    const backend = await openedBackend();

    const { events } = await generateWithEvents(backend, {
      kind: "add_world_document",
      name: null,
      instruction: "天気の言い伝え",
    });

    const streamed = events.flatMap((event) => (event.kind === "content" ? [event.text] : []));
    expect(streamed.join("").startsWith("# 天気の言い伝え")).toBe(true);
    const notices = events.flatMap((event) => (event.kind === "notice" ? [event] : []));
    expect(notices[0]?.message).toContain("生成済みの文書には反映されません");
  });

  it("指示が空なら、文字を流す前に invalid_input にする", async () => {
    const backend = await openedBackend();

    const { failure, events } = await failedGeneration(backend, {
      kind: "add_world_document",
      name: null,
      instruction: "",
    });

    expect((failure as BackendError).kind).toBe("invalid_input");
    expect(startedSteps(events)).toEqual([]);
  });

  it("使えない名前と使用済みの名前は、文字を流す前に invalid_input にする", async () => {
    const backend = await openedBackend();
    await backend.applyChangeSet(
      await backend.generate(
        "job-first",
        { kind: "add_world_document", name: "weather", instruction: "天気" },
        () => {},
      ),
    );

    for (const name of ["Weather Notes", "overview", "weather"]) {
      const { failure, events } = await failedGeneration(backend, {
        kind: "add_world_document",
        name,
        instruction: "別の資料",
      });

      expect(failure, name).toBeInstanceOf(BackendError);
      expect((failure as BackendError).kind, name).toBe("invalid_input");
      expect(startedSteps(events), name).toEqual([]);
    }
  });
});

describe("偽バックエンド / 指示から足す生成は工程の一覧に出ない", () => {
  it("人物を足しても、工程の種類は増えない", async () => {
    const backend = await openedBackend();
    const kindsBefore = (await backend.pipeline()).map((step) => step.task.kind);
    await backend.applyChangeSet(
      await backend.generate("job-1", { kind: "add_character", instruction: "幼なじみ" }, () => {}),
    );

    const kindsAfter = (await backend.pipeline()).map((step) => step.task.kind);

    expect(kindsAfter).not.toContain("add_character");
    expect(kindsAfter).not.toContain("add_world_document");
    expect(new Set(kindsAfter)).toEqual(new Set(kindsBefore));
  });
});
