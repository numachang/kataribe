import { beforeEach, describe, expect, it } from "vitest";
import { expectChanges, moveChanges, writeChanges } from "../../test/changeSets";
import { readText, textDocument } from "../../test/documents";
import type { Backend } from "../backend";
import type { NewScenePlan, StructureEdit } from "../types";
import { createMockBackend } from "./backend";
import { SAMPLE_PROJECT_FOLDER } from "./sampleProject";

// 人物・章・シーンの並べ替え。サンプル作品は、人物 2 人（霧島 凛・佐藤 健二）、第 1 章（シーン 3 つ。本文は 2 つ）、
// 第 2 章（シーン構成なし）を持つ。

let backend: Backend;

beforeEach(async () => {
  backend = createMockBackend({ delayMs: 0 });
  await backend.openProject(SAMPLE_PROJECT_FOLDER);
});

async function planAndApply(edit: StructureEdit) {
  const plan = await backend.planStructureEdit(edit);
  await backend.applyChangeSet(plan.change_set);
  return plan;
}

function scenePlan(title: string): NewScenePlan {
  return {
    title,
    summary: "",
    pov: null,
    characters: [],
    place: null,
    time: null,
    target_chars: null,
  };
}

/** 目次の節にある項目の題を、並びの順に返す。 */
async function sectionLabels(kind: string, pathPrefix: string): Promise<string[]> {
  const section = (await backend.overview()).sections.find((candidate) => candidate.kind === kind);
  return (section?.entries ?? [])
    .filter((entry) => entry.path?.startsWith(pathPrefix))
    .map((entry) => entry.label);
}

const characterLabels = () => sectionLabels("characters", "characters/");
const plannedChapterLabels = () => sectionLabels("plot", "plot/chapters/");

async function sceneLabels(chapter: string): Promise<string[]> {
  const manuscript = (await backend.overview()).sections.find(
    (section) => section.kind === "manuscript",
  );
  const heading = manuscript?.entries.find((entry) => entry.chapter === chapter);
  return (heading?.children ?? []).map((entry) => entry.label);
}

async function characterOrder(id: string): Promise<number | null> {
  const { document } = await backend.readDocument(`characters/${id}.md`);
  if (document.kind !== "character") {
    throw new Error("テスト: 人物資料として読めるはず");
  }
  return document.meta.order ?? null;
}

describe("人物を並べ替える", () => {
  const moveCharacter = (id: string, position: number): StructureEdit => ({
    kind: "move_character",
    path: `characters/${id}.md`,
    position,
  });

  it("order が変わる人物資料だけを書き直す変更案を返し、要約は何番目に移すかを言う", async () => {
    const plan = await backend.planStructureEdit(moveCharacter("sato-kenji", 0));

    expect(plan.change_set.summary).toBe("人物「佐藤 健二」を 1 番目に移します。");
    expect(plan.completed_summary).toBe("人物「佐藤 健二」を 1 番目に移しました。");
    expect(plan.created).toBeNull();
    expect(writeChanges(plan.change_set).map((file) => file.path)).toEqual([
      "characters/sato-kenji.md",
      "characters/kirishima-rin.md",
    ]);
  });

  it("適用すると、目次の順が変わり、order が 1, 2, … に振り直される", async () => {
    await planAndApply(moveCharacter("sato-kenji", 0));

    expect(await characterLabels()).toEqual(["佐藤 健二", "霧島 凛"]);
    expect(await characterOrder("sato-kenji")).toBe(1);
    expect(await characterOrder("kirishima-rin")).toBe(2);
  });

  it("人物資料の本文と項目は、order 以外そのまま残る", async () => {
    const before = await backend.readDocument("characters/kirishima-rin.md");

    await planAndApply(moveCharacter("kirishima-rin", 1));

    const after = await backend.readDocument("characters/kirishima-rin.md");
    expect(after.document).toMatchObject({
      kind: "character",
      body: (before.document as { body: string }).body,
      meta: { name: "霧島 凛", role: "主人公", order: 2 },
    });
  });

  it("order の欠番や重複があっても、並べ替えたあとは 1, 2, 3… にそろう", async () => {
    await planAndApply({
      kind: "add_character",
      id: "mizuno",
      meta: { name: "水野", reading: null, role: "", summary: "", order: 10 },
      body: "",
    });

    await planAndApply(moveCharacter("mizuno", 0));

    expect(await characterLabels()).toEqual(["水野", "霧島 凛", "佐藤 健二"]);
    expect(await characterOrder("mizuno")).toBe(1);
    expect(await characterOrder("kirishima-rin")).toBe(2);
    expect(await characterOrder("sato-kenji")).toBe(3);
  });

  it("今と同じ位置・範囲外の位置は invalid_input", async () => {
    await expect(
      backend.planStructureEdit(moveCharacter("kirishima-rin", 0)),
    ).rejects.toMatchObject({
      kind: "invalid_input",
      message: "人物「霧島 凛」はすでに 1 番目です。",
    });
    await expect(
      backend.planStructureEdit(moveCharacter("kirishima-rin", 2)),
    ).rejects.toMatchObject({ kind: "invalid_input" });
  });

  it("人物資料でないパスは invalid_input、無い人物資料は not_found", async () => {
    await expect(
      backend.planStructureEdit({ kind: "move_character", path: "world/overview.md", position: 0 }),
    ).rejects.toMatchObject({ kind: "invalid_input" });
    await expect(backend.planStructureEdit(moveCharacter("nobody", 0))).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("計画のあとに人物資料が外で書き換わったら、競合で何も変えない", async () => {
    const plan = await backend.planStructureEdit(moveCharacter("sato-kenji", 0));
    const { hash } = await backend.readDocument("characters/sato-kenji.md");
    await backend.writeDocument(
      "characters/sato-kenji.md",
      textDocument("---\nname: 佐藤 健二\norder: 2\n---\n外で書いた本文"),
      hash,
    );

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await characterLabels()).toEqual(["霧島 凛", "佐藤 健二"]);
    expect(await characterOrder("kirishima-rin")).toBe(1);
  });
});

describe("シーンを並べ替える", () => {
  const moveScene = (scene: string, position: number): StructureEdit => ({
    kind: "move_scene",
    chapter: "01",
    scene,
    position,
  });

  it("章立てだけを書き直す変更案を返す", async () => {
    const plan = await backend.planStructureEdit(moveScene("s01", 1));

    expect(plan.change_set.summary).toBe("第1章のシーン「招かれざる客」を 2 番目に移します。");
    expect(plan.completed_summary).toBe("第1章のシーン「招かれざる客」を 2 番目に移しました。");
    expect(plan.created).toBeNull();
    expect(plan.change_set.files.map((file) => file.kind)).toEqual(["write"]);
    expect(writeChanges(plan.change_set).map((file) => file.path)).toEqual(["plot/chapters/01.md"]);
  });

  it("適用すると、シーンの並びが変わり、シーンの id と本文はそのまま", async () => {
    const draftBefore = await readText(backend, "manuscript/01/s01.txt");

    await planAndApply(moveScene("s01", 2));

    expect(await sceneLabels("01")).toEqual(["遺言状の間", "消えた甥", "招かれざる客"]);
    expect(await readText(backend, "manuscript/01/s01.txt")).toBe(draftBefore);
    const { document } = await backend.readDocument("plot/chapters/01.md");
    expect(document).toMatchObject({ kind: "chapter" });
    expect(
      (document as { meta: { scenes: Array<{ id: string }> } }).meta.scenes.map(
        (scene) => scene.id,
      ),
    ).toEqual(["s02", "s03", "s01"]);
  });

  it("後ろのシーンを前へ移せる", async () => {
    await planAndApply(moveScene("s03", 0));

    expect(await sceneLabels("01")).toEqual(["消えた甥", "招かれざる客", "遺言状の間"]);
  });

  it("今と同じ位置・範囲外の位置は invalid_input", async () => {
    await expect(backend.planStructureEdit(moveScene("s02", 1))).rejects.toMatchObject({
      kind: "invalid_input",
      message: "第1章のシーン「遺言状の間」はすでに 2 番目です。",
    });
    await expect(backend.planStructureEdit(moveScene("s02", 3))).rejects.toMatchObject({
      kind: "invalid_input",
    });
  });

  it("無いシーン・シーン構成の無い章・無い章は not_found", async () => {
    await expect(backend.planStructureEdit(moveScene("s99", 0))).rejects.toMatchObject({
      kind: "not_found",
    });
    await expect(
      backend.planStructureEdit({ kind: "move_scene", chapter: "02", scene: "s01", position: 0 }),
    ).rejects.toMatchObject({ kind: "not_found" });
    await expect(
      backend.planStructureEdit({ kind: "move_scene", chapter: "09", scene: "s01", position: 0 }),
    ).rejects.toMatchObject({ kind: "not_found" });
  });

  it("計画のあとに章立てが外で書き換わったら、競合で何も変えない", async () => {
    const plan = await backend.planStructureEdit(moveScene("s01", 1));
    await planAndApply({
      kind: "add_scene",
      chapter: "01",
      before: null,
      scene: scenePlan("外で足した場面"),
    });

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await sceneLabels("01")).toEqual([
      "招かれざる客",
      "遺言状の間",
      "消えた甥",
      "外で足した場面",
    ]);
  });
});

describe("章を並べ替える", () => {
  const moveChapter = (chapter: string, position: number): StructureEdit => ({
    kind: "move_chapter",
    chapter,
    position,
  });

  /** 第 2 章に、本文のあるシーンを 1 つ作る。 */
  async function giveSecondChapterAScene(draft = "第二章の本文"): Promise<void> {
    await planAndApply({
      kind: "add_scene",
      chapter: "02",
      before: null,
      scene: scenePlan("岬へ"),
    });
    await backend.writeDocument("manuscript/02/s01.txt", textDocument(draft), null);
  }

  it("隣と入れ替えると、2 つの章の章立てと本文のフォルダを入れ替える変更案を返す", async () => {
    await giveSecondChapterAScene();

    const plan = await backend.planStructureEdit(moveChapter("01", 1));

    expect(plan.change_set.summary).toBe("第1章「雨の匂い」を第2章へ移します。");
    expect(plan.completed_summary).toBe("第1章「雨の匂い」を第2章へ移しました。");
    expect(plan.created).toBeNull();
    expect(plan.renumbered).toEqual([
      { from: "01", to: "02", title: "雨の匂い" },
      { from: "02", to: "01", title: "灯台のある岬" },
    ]);
    expect(moveChanges(plan.change_set)).toEqual([
      { kind: "move", from: "plot/chapters/02.md", to: "plot/chapters/01.md" },
      { kind: "move", from: "manuscript/02", to: "manuscript/01" },
      { kind: "move", from: "plot/chapters/01.md", to: "plot/chapters/02.md" },
      { kind: "move", from: "manuscript/01", to: "manuscript/02" },
    ]);
  });

  it("適用すると、章立てと本文が入れ替わり、本文は章に付いて動く", async () => {
    await giveSecondChapterAScene();

    await planAndApply(moveChapter("01", 1));

    expect(await plannedChapterLabels()).toEqual(["灯台のある岬", "雨の匂い"]);
    expect(await readText(backend, "manuscript/01/s01.txt")).toBe("第二章の本文");
    expect(await readText(backend, "manuscript/02/s01.txt")).toContain("館の扉が開くたび");
    expect(await sceneLabels("02")).toEqual(["招かれざる客", "遺言状の間", "消えた甥"]);
  });

  it("本文のフォルダの無い章は、本文が無いことを確かめる（外で本文ができて取り残されないように）", async () => {
    const plan = await backend.planStructureEdit(moveChapter("01", 1));

    expect(expectChanges(plan.change_set)).toEqual([
      { kind: "expect", path: "manuscript/02", base_hash: null },
    ]);
  });

  it("3 つの章の先頭へ移すと、動く範囲の章が 1 つずつ後ろへ回る（循環の移動）", async () => {
    await giveSecondChapterAScene();
    await planAndApply({ kind: "add_chapter", before: null, title: "終わりの章", storyline: "" });

    const plan = await planAndApply(moveChapter("03", 0));

    expect(plan.change_set.summary).toBe("第3章「終わりの章」を第1章へ移します。");
    expect(plan.renumbered).toEqual([
      { from: "01", to: "02", title: "雨の匂い" },
      { from: "02", to: "03", title: "灯台のある岬" },
      { from: "03", to: "01", title: "終わりの章" },
    ]);
    expect(await plannedChapterLabels()).toEqual(["終わりの章", "雨の匂い", "灯台のある岬"]);
    expect(await readText(backend, "manuscript/02/s01.txt")).toContain("館の扉が開くたび");
    expect(await readText(backend, "manuscript/03/s01.txt")).toBe("第二章の本文");
  });

  it("先頭の章を末尾へ移すと、動く範囲の章が 1 つずつ前へ詰まる", async () => {
    await planAndApply({ kind: "add_chapter", before: null, title: "終わりの章", storyline: "" });

    await planAndApply(moveChapter("01", 2));

    expect(await plannedChapterLabels()).toEqual(["灯台のある岬", "終わりの章", "雨の匂い"]);
    expect(await readText(backend, "manuscript/03/s01.txt")).toContain("館の扉が開くたび");
  });

  it("番号が抜けていても、動く範囲の番号の集合をそのまま割り当て、抜けた番号は詰めない", async () => {
    await backend.writeDocument(
      "plot/chapters/05.md",
      textDocument("---\ntitle: 後の章\n---\n"),
      null,
    );

    const plan = await planAndApply(moveChapter("05", 0));

    expect(plan.renumbered).toEqual([
      { from: "01", to: "02", title: "雨の匂い" },
      { from: "02", to: "05", title: "灯台のある岬" },
      { from: "05", to: "01", title: "後の章" },
    ]);
    expect(await plannedChapterLabels()).toEqual(["後の章", "雨の匂い", "灯台のある岬"]);
    expect(await readText(backend, "manuscript/02/s01.txt")).toContain("館の扉が開くたび");
  });

  it("動く範囲の外の章は動かさない", async () => {
    await planAndApply({ kind: "add_chapter", before: null, title: "終わりの章", storyline: "" });

    const plan = await backend.planStructureEdit(moveChapter("02", 2));

    expect(plan.renumbered.map((chapter) => chapter.from)).toEqual(["02", "03"]);
  });

  it("今と同じ位置・範囲外の位置は invalid_input、無い章は not_found", async () => {
    await expect(backend.planStructureEdit(moveChapter("01", 0))).rejects.toMatchObject({
      kind: "invalid_input",
      message: "第1章「雨の匂い」はすでに 1 番目です。",
    });
    await expect(backend.planStructureEdit(moveChapter("01", 2))).rejects.toMatchObject({
      kind: "invalid_input",
    });
    await expect(backend.planStructureEdit(moveChapter("09", 0))).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("計画のあとに、本文の無い章へ外で本文ができたら、競合で何も変えない", async () => {
    const plan = await backend.planStructureEdit(moveChapter("01", 1));
    await giveSecondChapterAScene("外で書いた本文");

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await plannedChapterLabels()).toEqual(["雨の匂い", "灯台のある岬"]);
    expect(await readText(backend, "manuscript/02/s01.txt")).toBe("外で書いた本文");
  });
});
