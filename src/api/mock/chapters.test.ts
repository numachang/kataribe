import { beforeEach, describe, expect, it } from "vitest";
import { expectChanges, moveChanges, trashChanges, writeChanges } from "../../test/changeSets";
import { readText, textDocument } from "../../test/documents";
import type { Backend } from "../backend";
import type { ChangeSet, StructureEdit } from "../types";
import { createMockBackend } from "./backend";
import { SAMPLE_PROJECT_FOLDER } from "./sampleProject";

// 章の追加・削除（番号の振り直し）と、変更案の移動・状態の確認の適用。サンプル作品は第 1 章（本文が 2 つ）と
// 第 2 章（シーン構成なし）を持つ。

let backend: Backend;

beforeEach(async () => {
  backend = createMockBackend({ delayMs: 0 });
  await backend.openProject(SAMPLE_PROJECT_FOLDER);
});

const addChapter = (before: string | null, title = "新しい章", storyline = ""): StructureEdit => ({
  kind: "add_chapter",
  before,
  title,
  storyline,
});

const removeChapter = (chapter: string): StructureEdit => ({ kind: "remove_chapter", chapter });

async function planAndApply(edit: StructureEdit) {
  const plan = await backend.planStructureEdit(edit);
  const overview = await backend.applyChangeSet(plan.change_set);
  return { plan, overview };
}

/** 目次の「あらすじ・章立て」の節にある章の（章の番号、章題）。 */
async function plannedChapters(): Promise<Array<[string | null, string]>> {
  const plot = (await backend.overview()).sections.find((section) => section.kind === "plot");
  return (plot?.entries ?? [])
    .filter((entry) => entry.path?.startsWith("plot/chapters/"))
    .map((entry) => [entry.chapter, entry.label]);
}

/** 第 2 章に、本文のあるシーンを 1 つ作る。 */
async function giveSecondChapterAScene(draft = "第二章の本文"): Promise<void> {
  await planAndApply({
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
  });
  await backend.writeDocument("manuscript/02/s01.txt", textDocument(draft), null);
}

describe("章を足す", () => {
  it("末尾に足すと、ほかの章は動かさず、章立ての新規書き込みと、本文のフォルダが無いことの確認だけを返す", async () => {
    const plan = await backend.planStructureEdit(addChapter(null, "終わりの章", "夜が明ける。"));

    expect(plan.change_set.summary).toBe("第3章「終わりの章」を追加します。");
    expect(plan.completed_summary).toBe("第3章「終わりの章」を追加しました。");
    expect(plan.created).toBe("plot/chapters/03.md");
    expect(plan.renumbered).toEqual([]);
    expect(moveChanges(plan.change_set)).toEqual([]);
    expect(expectChanges(plan.change_set)).toEqual([
      { kind: "expect", path: "manuscript/03", base_hash: null },
    ]);
    const [write] = writeChanges(plan.change_set);
    expect(write).toMatchObject({ path: "plot/chapters/03.md", previous: null, base_hash: null });
    expect(write?.content).toContain("title: 終わりの章");
    expect(write?.content).toContain("夜が明ける。");
  });

  it("適用すると、末尾に新しい章ができ、既存の章はそのまま", async () => {
    await planAndApply(addChapter(null, "終わりの章"));

    expect(await plannedChapters()).toEqual([
      ["01", "雨の匂い"],
      ["02", "灯台のある岬"],
      ["03", "終わりの章"],
    ]);
    expect(await readText(backend, "manuscript/01/s01.txt")).toContain("館の扉が開くたび");
  });

  it("途中に足すと、その章以降を 1 つ後ろへずらし、本文のある章は本文のフォルダごと移す", async () => {
    const plan = await backend.planStructureEdit(addChapter("01", "序章"));

    expect(plan.renumbered).toEqual([
      { from: "01", to: "02", title: "雨の匂い" },
      { from: "02", to: "03", title: "灯台のある岬" },
    ]);
    expect(moveChanges(plan.change_set)).toEqual([
      { kind: "move", from: "plot/chapters/01.md", to: "plot/chapters/02.md" },
      { kind: "move", from: "manuscript/01", to: "manuscript/02" },
      { kind: "move", from: "plot/chapters/02.md", to: "plot/chapters/03.md" },
    ]);
    // 本文のフォルダが無い第 2 章は、移さずに、まだ無いことを確かめる。移す先（manuscript/03）も、
    // ほかの変更で扱っていないので、まだ無いことを確かめる
    expect(expectChanges(plan.change_set)).toEqual([
      { kind: "expect", path: "manuscript/02", base_hash: null },
      { kind: "expect", path: "manuscript/03", base_hash: null },
    ]);
    // 新しい章は、改名のあとの「無いこと」を条件に書く
    expect(writeChanges(plan.change_set)).toMatchObject([
      { path: "plot/chapters/01.md", previous: null, base_hash: null },
    ]);
  });

  it("本文の無い章をずらした先が、ほかの変更で扱われていれば、確認を重ねない。扱われていなければ足す", async () => {
    await planAndApply(addChapter(null, "第三章"));

    const plan = await backend.planStructureEdit(addChapter("01", "序章"));

    // 本文の無い第 2・3 章の元の場所（02・03）は確認済み。第 3 章の移す先（04）だけが、どの変更にも無い
    const expectedPaths = expectChanges(plan.change_set).map((change) => change.path);
    expect(expectedPaths).toEqual(["manuscript/02", "manuscript/03", "manuscript/04"]);
  });

  it("本文の無い章を移す先に、外で本文のある章ができたら、競合で何も変えない", async () => {
    const plan = await backend.planStructureEdit(addChapter("02", "挿入"));
    await planAndApply(addChapter(null, "外で足した章"));
    await planAndApply({
      kind: "add_scene",
      chapter: "03",
      before: null,
      scene: {
        title: "外の場面",
        summary: "",
        pov: null,
        characters: [],
        place: null,
        time: null,
        target_chars: null,
      },
    });
    await backend.writeDocument("manuscript/03/s01.txt", textDocument("外で書いた本文"), null);

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await plannedChapters()).toHaveLength(3);
    expect(await readText(backend, "manuscript/03/s01.txt")).toBe("外で書いた本文");
  });

  it("途中に足して適用すると、章の番号と本文が振り直される", async () => {
    await planAndApply(addChapter("01", "序章"));

    expect(await plannedChapters()).toEqual([
      ["01", "序章"],
      ["02", "雨の匂い"],
      ["03", "灯台のある岬"],
    ]);
    expect(await readText(backend, "manuscript/02/s01.txt")).toContain("館の扉が開くたび");
    await expect(backend.readDocument("manuscript/01/s01.txt")).rejects.toMatchObject({
      kind: "not_found",
    });
    const { document } = await backend.readDocument("plot/chapters/02.md");
    expect(document).toMatchObject({ kind: "chapter", meta: { title: "雨の匂い" } });
  });

  it("章が無い作品には 01 として足せる", async () => {
    await backend.createProject("C:\\Users\\demo\\Documents\\空の作品", {
      title: "空の作品",
      author: null,
      genre: "mystery",
      genre_note: null,
      rating: "general",
      target_length: 10000,
      idea: "アイデア",
    });

    await planAndApply(addChapter(null, "最初の章"));

    expect(await plannedChapters()).toEqual([["01", "最初の章"]]);
  });

  it("章題が空・複数行なら invalid_input、足す位置の章が無ければ not_found", async () => {
    await expect(backend.planStructureEdit(addChapter(null, "  "))).rejects.toMatchObject({
      kind: "invalid_input",
    });
    await expect(
      backend.planStructureEdit(addChapter(null, "一行目\n二行目")),
    ).rejects.toMatchObject({ kind: "invalid_input" });
    await expect(backend.planStructureEdit(addChapter("09"))).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("計画のあとに、移動先の番号の章が外でできたら、競合で何も変えない", async () => {
    const plan = await backend.planStructureEdit(addChapter("02", "挿入"));
    await backend.writeDocument(
      "plot/chapters/03.md",
      textDocument("---\ntitle: 外で足した章\n---\n"),
      null,
    );

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await plannedChapters()).toEqual([
      ["01", "雨の匂い"],
      ["02", "灯台のある岬"],
      ["03", "外で足した章"],
    ]);
  });
});

describe("章を消す", () => {
  it("章立てと本文のフォルダ（中のファイルと字数）をゴミ箱へ移し、後ろの章を 1 つ前へずらす", async () => {
    const plan = await backend.planStructureEdit(removeChapter("01"));

    expect(plan.change_set.summary).toBe("第1章「雨の匂い」をゴミ箱へ移します。");
    expect(plan.completed_summary).toBe("第1章「雨の匂い」をゴミ箱へ移しました。");
    expect(plan.created).toBeNull();
    const trashed = trashChanges(plan.change_set);
    expect(trashed.map((file) => file.path)).toEqual(["plot/chapters/01.md", "manuscript/01"]);
    expect(trashed[1]?.files.map((file) => file.path)).toEqual([
      "manuscript/01/s01.txt",
      "manuscript/01/s02.txt",
    ]);
    expect(trashed[1]?.files.every((file) => file.chars > 0)).toBe(true);
    expect(plan.renumbered).toEqual([{ from: "02", to: "01", title: "灯台のある岬" }]);
    expect(moveChanges(plan.change_set)).toEqual([
      { kind: "move", from: "plot/chapters/02.md", to: "plot/chapters/01.md" },
    ]);
    expect(expectChanges(plan.change_set)).toEqual([
      { kind: "expect", path: "manuscript/02", base_hash: null },
    ]);
  });

  it("適用すると、章立てと本文が消え、後ろの章が 1 つ前の番号になる", async () => {
    await giveSecondChapterAScene("第二章の本文");

    await planAndApply(removeChapter("01"));

    expect(await plannedChapters()).toEqual([["01", "灯台のある岬"]]);
    expect(await readText(backend, "manuscript/01/s01.txt")).toBe("第二章の本文");
    await expect(backend.readDocument("manuscript/02/s01.txt")).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("最後の章を消すときは、ほかの章を動かさない。本文が無い章は、本文が無いことを確かめる", async () => {
    const plan = await backend.planStructureEdit(removeChapter("02"));

    expect(moveChanges(plan.change_set)).toEqual([]);
    expect(plan.renumbered).toEqual([]);
    expect(trashChanges(plan.change_set).map((file) => file.path)).toEqual(["plot/chapters/02.md"]);
    expect(expectChanges(plan.change_set)).toEqual([
      { kind: "expect", path: "manuscript/02", base_hash: null },
    ]);
  });

  it("第 0 章を消すと、第 1 章が 00 になる（番号は 0 まで許す）", async () => {
    await backend.writeDocument("plot/chapters/00.md", textDocument("---\ntitle: 序\n---\n"), null);

    const { plan } = await planAndApply(removeChapter("00"));

    expect(plan.renumbered[0]).toEqual({ from: "01", to: "00", title: "雨の匂い" });
    expect(await plannedChapters()).toEqual([
      ["00", "雨の匂い"],
      ["01", "灯台のある岬"],
    ]);
  });

  it("章が無ければ not_found", async () => {
    await expect(backend.planStructureEdit(removeChapter("09"))).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("確かめたあとに本文のフォルダへファイルが増えたら、競合で何も消さない", async () => {
    const plan = await backend.planStructureEdit(removeChapter("01"));
    // 本文の無かった s03 に、外で本文ができた
    await backend.writeDocument("manuscript/01/s03.txt", textDocument("外で書いた本文"), null);

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await plannedChapters()).toHaveLength(2);
    expect(await readText(backend, "manuscript/01/s03.txt")).toBe("外で書いた本文");
  });

  it("確かめたあとに本文のフォルダの中のファイルが書き換わったら、競合で何も消さない", async () => {
    const plan = await backend.planStructureEdit(removeChapter("01"));
    const { hash } = await backend.readDocument("manuscript/01/s01.txt");
    await backend.writeDocument("manuscript/01/s01.txt", textDocument("書き直した本文"), hash);

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await plannedChapters()).toHaveLength(2);
  });

  it("本文が無いと確かめた章に、外で本文ができたら、競合になる（取り残された本文を作らない）", async () => {
    const plan = await backend.planStructureEdit(removeChapter("02"));
    await giveSecondChapterAScene();

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await plannedChapters()).toHaveLength(2);
  });
});

describe("シーンを消すときの、本文が無いことの確認", () => {
  it("本文の無いシーンは、本文が無いことの確認を付け、外で本文ができたら競合にする", async () => {
    const plan = await backend.planStructureEdit({
      kind: "remove_scene",
      chapter: "01",
      scene: "s03",
    });
    expect(expectChanges(plan.change_set)).toEqual([
      { kind: "expect", path: "manuscript/01/s03.txt", base_hash: null },
    ]);

    await backend.writeDocument("manuscript/01/s03.txt", textDocument("外で書いた本文"), null);

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });
    const manuscript = (await backend.overview()).sections.find(
      (section) => section.kind === "manuscript",
    );
    expect(manuscript?.entries[0]?.children).toHaveLength(3);
  });
});

describe("変更案の適用（移動と確認）", () => {
  function changeSetOf(files: ChangeSet["files"]): ChangeSet {
    return { summary: "試します。", files, project_root: SAMPLE_PROJECT_FOLDER };
  }

  const move = (from: string, to: string) => ({ kind: "move" as const, from, to });

  it("章の番号の入れ替え（循環する移動）も、一度に付け替える", async () => {
    await giveSecondChapterAScene();

    await backend.applyChangeSet(
      changeSetOf([
        move("plot/chapters/01.md", "plot/chapters/02.md"),
        move("plot/chapters/02.md", "plot/chapters/01.md"),
        move("manuscript/01", "manuscript/02"),
        move("manuscript/02", "manuscript/01"),
      ]),
    );

    expect(await plannedChapters()).toEqual([
      ["01", "灯台のある岬"],
      ["02", "雨の匂い"],
    ]);
    expect(await readText(backend, "manuscript/01/s01.txt")).toBe("第二章の本文");
    expect(await readText(backend, "manuscript/02/s01.txt")).toContain("館の扉が開くたび");
  });

  it("移動のあとの書き込みは、移動元の今の内容で確かめる", async () => {
    const { hash } = await backend.readDocument("plot/chapters/02.md");
    const rewritten = (baseHash: string | null) =>
      changeSetOf([
        move("plot/chapters/02.md", "plot/chapters/03.md"),
        {
          kind: "write",
          path: "plot/chapters/03.md",
          content: "---\ntitle: 書き直した章\n---\n",
          previous: null,
          base_hash: baseHash,
        },
      ]);

    await expect(backend.applyChangeSet(rewritten("00000000"))).rejects.toMatchObject({
      kind: "conflict",
    });
    await backend.applyChangeSet(rewritten(hash));

    expect(await plannedChapters()).toEqual([
      ["01", "雨の匂い"],
      ["03", "書き直した章"],
    ]);
  });

  it("移動先が空いていなければ、競合にして何も変えない", async () => {
    await expect(
      backend.applyChangeSet(changeSetOf([move("plot/chapters/01.md", "plot/chapters/02.md")])),
    ).rejects.toMatchObject({ kind: "conflict" });

    expect(await plannedChapters()).toHaveLength(2);
  });

  it("移動元が無ければ、競合にする", async () => {
    await expect(
      backend.applyChangeSet(changeSetOf([move("plot/chapters/05.md", "plot/chapters/06.md")])),
    ).rejects.toMatchObject({ kind: "conflict" });
  });

  it("無いはずのファイルがあれば、状態の確認で競合にする", async () => {
    await expect(
      backend.applyChangeSet(
        changeSetOf([{ kind: "expect", path: "concept.md", base_hash: null }]),
      ),
    ).rejects.toMatchObject({ kind: "conflict" });
    await expect(
      backend.applyChangeSet(
        changeSetOf([{ kind: "expect", path: "plot/chapters/09.md", base_hash: null }]),
      ),
    ).resolves.toBeDefined();
  });

  it.each([
    ["作品情報の移動", [move("kataribe.yaml", "other.yaml")]],
    ["内部データの移動先", [move("concept.md", ".Kataribe/concept.md")]],
    ["自分の中への移動", [move("manuscript/01", "manuscript/01/inner")]],
    [
      "同じ移動元の重なり",
      [move("manuscript/01", "manuscript/05"), move("manuscript/01", "manuscript/06")],
    ],
    [
      "ゴミ箱へ移すパスの下への書き込み",
      [
        {
          kind: "trash" as const,
          path: "manuscript/01",
          files: [{ path: "manuscript/01/s01.txt", base_hash: "0", chars: 1 }],
        },
        {
          kind: "write" as const,
          path: "manuscript/01/s09.txt",
          content: "x",
          previous: null,
          base_hash: null,
        },
      ],
    ],
  ])("形の誤りは invalid_input: %s", async (_name, files) => {
    await expect(backend.applyChangeSet(changeSetOf(files))).rejects.toMatchObject({
      kind: "invalid_input",
    });
  });
});
