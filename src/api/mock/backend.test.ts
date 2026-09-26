import { describe, expect, it } from "vitest";
import type { GenerationEvent } from "../types";
import { createMockBackend } from "./backend";
import { SAMPLE_PROJECT_FOLDER } from "./sampleProject";

describe("createMockBackend / サンプル作品", () => {
  it("サンプル作品を開くと目次と工程が一貫している", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    const overview = await backend.openProject(SAMPLE_PROJECT_FOLDER);

    expect(overview.title).toBe("月霧の館");
    expect(overview.total_chars).toBeGreaterThan(0);

    const manuscript = overview.sections.find((section) => section.kind === "manuscript");
    expect(manuscript?.entries).toHaveLength(2);
    expect(manuscript?.entries[0]?.children.map((entry) => entry.exists)).toEqual([
      true,
      true,
      false,
    ]);

    const pipeline = await backend.pipeline();
    const draftSteps = pipeline.filter((step) => step.task.kind === "draft");
    expect(draftSteps).toHaveLength(3);
    expect(draftSteps.filter((step) => step.state === "done")).toHaveLength(2);
    expect(draftSteps.filter((step) => step.state === "ready")).toHaveLength(1);

    const scenePlanForChapter2 = pipeline.find(
      (step) => step.task.kind === "scene_plan" && step.task.chapter === "02",
    );
    expect(scenePlanForChapter2?.state).toBe("ready");
  });

  it("開いていない状態でファイルを読もうとするとエラーになる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await expect(backend.readFile("concept.md")).rejects.toMatchObject({ kind: "not_found" });
  });
});

describe("createMockBackend / listModels", () => {
  it("llm を渡さなくてもモデル一覧を返す", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    const models = await backend.listModels();
    expect(models.length).toBeGreaterThan(0);
  });

  it("llm を渡すと、保存前の入力中の接続先を使って試せる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    const models = await backend.listModels({ base_url: "http://localhost:9999/v1", model: "" });
    expect(models.length).toBeGreaterThan(0);
  });

  it("llm の接続先が空だと invalid_input で失敗する", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await expect(backend.listModels({ base_url: "", model: "" })).rejects.toMatchObject({
      kind: "invalid_input",
    });
  });
});

describe("createMockBackend / 新規作成", () => {
  it("新しい作品は何も生成されていない状態で始まる", async () => {
    const backend = createMockBackend({ delayMs: 0, pickFolder: async () => "C:\\projects\\新作" });
    const folder = await backend.pickFolder();
    expect(folder).toBe("C:\\projects\\新作");

    const overview = await backend.createProject(folder ?? "", {
      title: "無題の物語",
      author: null,
      genre: "fantasy",
      genre_note: null,
      rating: "general",
      target_length: 20000,
      idea: "旅する少女の話",
    });

    expect(overview.title).toBe("無題の物語");
    expect(overview.total_chars).toBe(0);

    const pipeline = await backend.pipeline();
    expect(pipeline[0]).toMatchObject({ task: { kind: "concept" }, state: "ready" });
    expect(pipeline.find((step) => step.task.kind === "style")).toMatchObject({ state: "blocked" });
  });

  it("既に使われているフォルダを指定すると invalid_input で失敗する", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await expect(
      backend.createProject(SAMPLE_PROJECT_FOLDER, {
        title: "重複",
        author: null,
        genre: "mystery",
        genre_note: null,
        rating: "general",
        target_length: 1000,
        idea: "…",
      }),
    ).rejects.toMatchObject({ kind: "invalid_input" });
  });
});

describe("createMockBackend / ファイルの読み書きと競合", () => {
  it("正しいハッシュで上書きできる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);

    const file = await backend.readFile("concept.md");
    const newHash = await backend.writeFile("concept.md", "書き直した企画", file.hash);
    expect(newHash).not.toBe(file.hash);

    const reloaded = await backend.readFile("concept.md");
    expect(reloaded.content).toBe("書き直した企画");
  });

  it("古いハッシュで上書きしようとすると conflict になる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);

    const file = await backend.readFile("concept.md");
    await backend.writeFile("concept.md", "誰かが先に書き換えた", file.hash);

    await expect(backend.writeFile("concept.md", "自分の変更", file.hash)).rejects.toMatchObject({
      kind: "conflict",
    });
  });

  it("新規ファイルのつもりで既存ファイルへ書き込むと conflict になる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);

    await expect(backend.writeFile("concept.md", "上書き", null)).rejects.toMatchObject({
      kind: "conflict",
    });
  });
});

describe("createMockBackend / 生成", () => {
  it("企画を生成すると ChangeSet が返り、適用すると目次に反映される", async () => {
    const backend = createMockBackend({
      delayMs: 0,
      pickFolder: async () => "C:\\projects\\新作2",
    });
    const folder = await backend.pickFolder();
    await backend.createProject(folder ?? "", {
      title: "無題の物語",
      author: null,
      genre: "fantasy",
      genre_note: null,
      rating: "general",
      target_length: 20000,
      idea: "旅する少女の話",
    });

    const events: GenerationEvent[] = [];
    const changeSet = await backend.generate("job-1", { kind: "concept" }, (event) =>
      events.push(event),
    );

    expect(events.some((event) => event.kind === "step_started")).toBe(true);
    expect(events.some((event) => event.kind === "content")).toBe(true);
    expect(events.some((event) => event.kind === "step_finished")).toBe(true);
    expect(changeSet.files).toHaveLength(1);
    expect(changeSet.files[0]?.previous).toBeNull();

    const overviewAfterApply = await backend.applyChangeSet(changeSet);
    const planning = overviewAfterApply.sections.find((section) => section.kind === "planning");
    const conceptEntry = planning?.entries.find((entry) => entry.kind === "concept");
    expect(conceptEntry?.exists).toBe(true);

    const pipeline = await backend.pipeline();
    expect(pipeline.find((step) => step.task.kind === "concept")).toMatchObject({ state: "done" });
  });

  it("生成を中止すると cancelled エラーになる", async () => {
    const backend = createMockBackend({ delayMs: 20 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);

    const promise = backend.generate("job-cancel", { kind: "concept" }, () => {});
    await new Promise((resolve) => setTimeout(resolve, 5));
    await backend.cancelGeneration("job-cancel");

    await expect(promise).rejects.toMatchObject({ kind: "cancelled" });
  });

  it("書き直し指示（revise）で既存の本文を変更できる", async () => {
    const backend = createMockBackend({ delayMs: 0 });
    await backend.openProject(SAMPLE_PROJECT_FOLDER);

    const before = await backend.readFile("manuscript/01/s01.txt");
    const changeSet = await backend.generate(
      "job-revise",
      { kind: "revise", path: "manuscript/01/s01.txt", instruction: "もっと不穏な雰囲気にして" },
      () => {},
    );

    expect(changeSet.files[0]?.previous).toBe(before.content);
    expect(changeSet.files[0]?.content).not.toBe(before.content);
  });
});
