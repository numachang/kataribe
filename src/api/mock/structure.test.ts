import { beforeEach, describe, expect, it } from "vitest";
import { trashChanges, writeChanges } from "../../test/changeSets";
import { readText, textDocument } from "../../test/documents";
import type { Backend } from "../backend";
import type { CharacterMeta, NewScenePlan, StructureEdit } from "../types";
import { createMockBackend } from "./backend";
import { SAMPLE_PROJECT_FOLDER } from "./sampleProject";

let backend: Backend;

beforeEach(async () => {
  backend = createMockBackend({ delayMs: 0 });
  await backend.openProject(SAMPLE_PROJECT_FOLDER);
});

function characterMeta(overrides: Partial<CharacterMeta> = {}): CharacterMeta {
  return { name: "新山 ゆき", reading: null, role: "", summary: "", order: null, ...overrides };
}

function scenePlan(overrides: Partial<NewScenePlan> = {}): NewScenePlan {
  return {
    title: "雨上がり",
    summary: "",
    pov: null,
    characters: [],
    place: null,
    time: null,
    target_chars: null,
    ...overrides,
  };
}

function addCharacter(
  id: string | null,
  meta: Partial<CharacterMeta> = {},
  body = "",
): StructureEdit {
  return { kind: "add_character", id, meta: characterMeta(meta), body };
}

function removeCharacter(id: string): StructureEdit {
  return { kind: "remove_character", path: `characters/${id}.md` };
}

/** ID の規則に合わないファイル名の人物資料を、作品に直接置く（外で作られたファイルを模す）。 */
async function putCharacterFile(path: string, name: string): Promise<void> {
  await backend.applyChangeSet({
    summary: "人物資料を置きます。",
    project_root: SAMPLE_PROJECT_FOLDER,
    files: [
      {
        kind: "write",
        path,
        content: `---\nname: ${name}\n---\n`,
        previous: null,
        base_hash: null,
      },
    ],
  });
}

async function planAndApply(edit: StructureEdit) {
  const plan = await backend.planStructureEdit(edit);
  const overview = await backend.applyChangeSet(plan.change_set);
  return { plan, overview };
}

async function manuscriptSceneLabels(chapterId: string): Promise<string[]> {
  const overview = await backend.overview();
  const manuscript = overview.sections.find((section) => section.kind === "manuscript");
  const chapter = manuscript?.entries.find((entry) => entry.chapter === chapterId);
  return chapter?.children.map((scene) => scene.label) ?? [];
}

describe("人物を足す", () => {
  it("書き込みの変更案と、適用後に開く文書を返し、作品フォルダはまだ変えない", async () => {
    const plan = await backend.planStructureEdit(addCharacter("niiyama-yuki"));

    expect(plan.created).toBe("characters/niiyama-yuki.md");
    expect(plan.change_set.summary).toBe("人物「新山 ゆき」を追加します。");
    expect(plan.completed_summary).toBe("人物「新山 ゆき」を追加しました。");
    expect(plan.change_set.project_root).toBe(SAMPLE_PROJECT_FOLDER);
    const [write] = writeChanges(plan.change_set);
    expect(write).toMatchObject({ path: "characters/niiyama-yuki.md", previous: null });
    expect(write?.base_hash).toBeNull();
    await expect(backend.readDocument("characters/niiyama-yuki.md")).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("適用すると、目次と工程に出て、人物資料として開ける", async () => {
    const { overview } = await planAndApply(
      addCharacter("niiyama-yuki", { role: "助手", summary: "新人" }, "## 外見\n小柄。"),
    );

    const characters = overview.sections.find((section) => section.kind === "characters");
    expect(characters?.entries.map((entry) => entry.label)).toContain("新山 ゆき");
    const { document } = await backend.readDocument("characters/niiyama-yuki.md");
    expect(document).toMatchObject({
      kind: "character",
      meta: { name: "新山 ゆき", role: "助手", summary: "新人" },
      body: "## 外見\n小柄。",
    });
    const pipeline = await backend.pipeline();
    expect(pipeline.find((step) => step.label === "登場人物: 新山 ゆき")?.state).toBe("done");
  });

  it("本文が空の人物は、生成の工程に取りかかれる工程として出る", async () => {
    await planAndApply(addCharacter("niiyama-yuki", {}, ""));

    const pipeline = await backend.pipeline();
    expect(pipeline.find((step) => step.label === "登場人物: 新山 ゆき")?.state).toBe("ready");
  });

  it("順番を決めなければ、今の最大の次（末尾）になる", async () => {
    await planAndApply(addCharacter("niiyama-yuki"));

    const { document } = await backend.readDocument("characters/niiyama-yuki.md");
    expect(document).toMatchObject({ kind: "character", meta: { order: 3 } });
  });

  it("ID を省くと、名前から英数字の ID を決め、使用済みなら番号を付ける", async () => {
    await planAndApply(addCharacter(null, { name: "Kirishima Rin" }));

    const { document } = await backend.readDocument("characters/kirishima-rin-2.md");
    expect(document).toMatchObject({ kind: "character", meta: { name: "Kirishima Rin" } });
  });

  it("名前が空なら invalid_input", async () => {
    await expect(
      backend.planStructureEdit(addCharacter("x", { name: "  " })),
    ).rejects.toMatchObject({ kind: "invalid_input", message: "人物の名前を入力してください。" });
  });

  it("使用済みの ID は invalid_input で、理由に ID を含める", async () => {
    await expect(backend.planStructureEdit(addCharacter("kirishima-rin"))).rejects.toMatchObject({
      kind: "invalid_input",
      message: "ID「kirishima-rin」はもう使われています。",
    });
  });

  it.each([
    ["Rin", "大文字は使えません"],
    ["霧島", "小文字の英数字とハイフンだけ"],
    ["Kirishima Rin", "大文字は使えません"],
    ["-rin", "先頭と末尾にハイフン"],
    ["rin--x", "ハイフンは続けて使えません"],
    ["a".repeat(49), "48 文字まで"],
    ["con", "予約"],
  ])("使えない ID「%s」は invalid_input で、理由を返す", async (id, reason) => {
    await expect(backend.planStructureEdit(addCharacter(id))).rejects.toMatchObject({
      kind: "invalid_input",
      message: expect.stringContaining(reason),
    });
  });

  it("空白だけの ID は「決めない」と同じで、名前から自動で決める", async () => {
    const plan = await backend.planStructureEdit(addCharacter("  ", { name: "Niiyama Yuki" }));

    expect(plan.created).toBe("characters/niiyama-yuki.md");
  });
});

describe("人物の ID の提案", () => {
  it("英数字だけを slug にし、使用済みの ID を避ける", async () => {
    expect(await backend.suggestCharacterId("", "Rin Sato")).toBe("rin-sato");
    expect(await backend.suggestCharacterId("", "Kirishima Rin")).toBe("kirishima-rin-2");
    expect(await backend.suggestCharacterId("Sato Kenji", "")).toBe("sato-kenji-2");
  });

  it("読みを優先し、作れなければ名前を使う", async () => {
    expect(await backend.suggestCharacterId("yuki", "Yuki Niiyama")).toBe("yuki");
    expect(await backend.suggestCharacterId("にいやま", "Niiyama")).toBe("niiyama");
  });

  it("英数字を含まない（かなや漢字だけの）入力は character にする（かなは変換しない）", async () => {
    expect(await backend.suggestCharacterId("きりしま りん", "霧島 凛")).toBe("character");
  });
});

describe("人物を消す", () => {
  it("ゴミ箱へ移す変更案と、その人物を挙げているシーンを返す", async () => {
    const plan = await backend.planStructureEdit(removeCharacter("sato-kenji"));

    expect(plan.change_set.summary).toBe("人物「佐藤 健二」をゴミ箱へ移します。");
    const [trash] = trashChanges(plan.change_set);
    expect(trash?.path).toBe("characters/sato-kenji.md");
    expect(trash?.files[0]).toMatchObject({ path: "characters/sato-kenji.md" });
    expect(trash?.files[0]?.base_hash).not.toBeNull();
    expect(trash?.files[0]?.chars).toBeGreaterThan(0);
    expect(plan.references.map((reference) => reference.scene)).toEqual(["s01", "s02"]);
    expect(plan.references[0]).toMatchObject({
      chapter: "01",
      chapter_title: "雨の匂い",
      scene_title: "招かれざる客",
      as_pov: false,
      as_character: true,
    });
  });

  it("名前だけ・空白の違いの書き方も、同じ人物として数える", async () => {
    // 視点の「霧島 凛」と、登場人物の「霧島 凛」がそろって挙がる。
    const plan = await backend.planStructureEdit(removeCharacter("kirishima-rin"));

    expect(plan.references).toHaveLength(3);
    expect(plan.references[0]).toMatchObject({ as_pov: true, as_character: true });
  });

  it("適用すると、目次から消える", async () => {
    const { overview } = await planAndApply(removeCharacter("sato-kenji"));

    const characters = overview.sections.find((section) => section.kind === "characters");
    expect(characters?.entries.map((entry) => entry.label)).toEqual(["霧島 凛"]);
    await expect(backend.readDocument("characters/sato-kenji.md")).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("確かめたあとに人物資料が書き換えられたら、競合で何も変えない", async () => {
    const plan = await backend.planStructureEdit(removeCharacter("sato-kenji"));
    const { document, hash } = await backend.readDocument("characters/sato-kenji.md");
    if (document.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }
    await backend.writeDocument(
      "characters/sato-kenji.md",
      { ...document, body: "外で書き換えた" },
      hash,
    );

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    const after = await backend.readDocument("characters/sato-kenji.md");
    expect(after.document).toMatchObject({ body: "外で書き換えた" });
  });

  it("人物資料が無ければ not_found", async () => {
    await expect(backend.planStructureEdit(removeCharacter("nobody"))).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("ファイル名が ID の規則に合わない人物資料も、パスで指せば消せる", async () => {
    await putCharacterFile("characters/Rin.md", "リン");
    await putCharacterFile("characters/凛.md", "凛");

    const plan = await backend.planStructureEdit(removeCharacter("Rin"));
    expect(trashChanges(plan.change_set)[0]?.path).toBe("characters/Rin.md");
    await backend.applyChangeSet(plan.change_set);
    const { overview } = await planAndApply(removeCharacter("凛"));

    const characters = overview.sections.find((section) => section.kind === "characters");
    expect(characters?.entries.map((entry) => entry.label)).toEqual(["霧島 凛", "佐藤 健二"]);
  });

  it("characters/ 直下の Markdown でないパスは invalid_input", async () => {
    for (const path of ["characters/sub/rin.md", "characters/rin.txt", "concept.md"]) {
      await expect(
        backend.planStructureEdit({ kind: "remove_character", path }),
      ).rejects.toMatchObject({ kind: "invalid_input" });
    }
  });
});

describe("世界観の資料を足す", () => {
  const addWorld = (name: string | null, title: string, body = ""): StructureEdit => ({
    kind: "add_world_document",
    name,
    title,
    body,
  });

  it("題を見出しにして、本文を空行を挟んで続ける", async () => {
    const plan = await backend.planStructureEdit(addWorld("glossary", "用語集", "霧：朝に出る。"));

    expect(plan.created).toBe("world/glossary.md");
    expect(writeChanges(plan.change_set)[0]?.content).toBe("# 用語集\n\n霧：朝に出る。\n");
  });

  it("本文が空なら見出しだけにする", async () => {
    const plan = await backend.planStructureEdit(addWorld("glossary", "用語集", "  \n"));

    expect(writeChanges(plan.change_set)[0]?.content).toBe("# 用語集\n");
  });

  it("適用すると、目次に題で出て、文字列の文書として開ける", async () => {
    const { overview } = await planAndApply(addWorld("glossary", "用語集", "霧：朝に出る。"));

    const world = overview.sections.find((section) => section.kind === "world");
    expect(world?.entries.map((entry) => [entry.path, entry.label])).toEqual([
      ["world/overview.md", "世界観"],
      ["world/glossary.md", "用語集"],
    ]);
    expect(await readText(backend, "world/glossary.md")).toBe("# 用語集\n\n霧：朝に出る。\n");
  });

  it("名前を省くと、英数字の題からは slug、日本語の題からは doc を使い、重なれば番号を付ける", async () => {
    await planAndApply(addWorld(null, "City Map"));
    await planAndApply(addWorld(null, "用語集"));
    await planAndApply(addWorld(null, "地名"));

    for (const name of ["city-map", "doc", "doc-2"]) {
      await expect(readText(backend, `world/${name}.md`)).resolves.toContain("# ");
    }
  });

  it("題が空・複数行、名前が使えない・使用済みなら invalid_input", async () => {
    await planAndApply(addWorld("glossary", "用語集"));

    for (const edit of [
      addWorld("x", "  "),
      addWorld("x", "一行目\n二行目"),
      addWorld("Glossary", "題"),
      addWorld("overview", "題"),
      addWorld("glossary", "題"),
    ]) {
      await expect(backend.planStructureEdit(edit)).rejects.toMatchObject({
        kind: "invalid_input",
      });
    }
  });
});

describe("世界観の資料を消す", () => {
  it("足した資料はゴミ箱へ移す変更案になり、適用すると目次から消える", async () => {
    await planAndApply({ kind: "add_world_document", name: "glossary", title: "用語集", body: "" });

    const { plan, overview } = await planAndApply({
      kind: "remove_world_document",
      path: "world/glossary.md",
    });

    expect(plan.change_set.summary).toBe("世界観の資料「用語集」をゴミ箱へ移します。");
    expect(trashChanges(plan.change_set)[0]?.path).toBe("world/glossary.md");
    const world = overview.sections.find((section) => section.kind === "world");
    expect(world?.entries.map((entry) => entry.path)).toEqual(["world/overview.md"]);
  });

  it("概要は大文字小文字が違っても足した資料として扱わない（Windows は区別しない）", async () => {
    await expect(
      backend.planStructureEdit({ kind: "remove_world_document", path: "world/Overview.md" }),
    ).rejects.toMatchObject({ kind: "invalid_input" });
    await backend.applyChangeSet({
      summary: "置きます。",
      project_root: SAMPLE_PROJECT_FOLDER,
      files: [
        { kind: "write", path: "world/Overview.md", content: "x", previous: null, base_hash: null },
      ],
    });

    const world = (await backend.overview()).sections.find((section) => section.kind === "world");
    expect(world?.entries.map((entry) => entry.path)).toEqual(["world/overview.md"]);
  });

  it("世界観の概要と、world/ の外のファイルは消せない（invalid_input）", async () => {
    for (const path of ["world/overview.md", "concept.md", "world/sub/x.md"]) {
      await expect(
        backend.planStructureEdit({ kind: "remove_world_document", path }),
      ).rejects.toMatchObject({ kind: "invalid_input" });
    }
  });

  it("資料が無ければ not_found", async () => {
    await expect(
      backend.planStructureEdit({ kind: "remove_world_document", path: "world/none.md" }),
    ).rejects.toMatchObject({ kind: "not_found" });
  });
});

describe("シーンを足す", () => {
  const addScene = (
    chapter: string,
    before: string | null,
    scene = scenePlan(),
  ): StructureEdit => ({
    kind: "add_scene",
    chapter,
    before,
    scene,
  });

  it("章の末尾に、使われていない最小の番号の id で足し、章立てを開く", async () => {
    const plan = await backend.planStructureEdit(addScene("01", null));

    expect(plan.created).toBe("plot/chapters/01.md");
    expect(plan.change_set.summary).toBe("第1章にシーン「雨上がり」を追加します。");
    const [write] = writeChanges(plan.change_set);
    expect(write?.path).toBe("plot/chapters/01.md");
    expect(write?.base_hash).not.toBeNull();
    expect(write?.content).toContain("  - id: s04\n    title: 雨上がり");
  });

  it("適用すると、末尾に並び、既存のシーンの本文は残る", async () => {
    const { overview } = await planAndApply(addScene("01", null));

    expect(await manuscriptSceneLabels("01")).toEqual([
      "招かれざる客",
      "遺言状の間",
      "消えた甥",
      "雨上がり",
    ]);
    const manuscript = overview.sections.find((section) => section.kind === "manuscript");
    const added = manuscript?.entries[0]?.children[3];
    expect(added).toMatchObject({ scene: "s04", chapter: "01", exists: false });
    expect(await readText(backend, "manuscript/01/s01.txt")).toContain("館の扉が開くたび");
  });

  it("シーンの前に足せる", async () => {
    await planAndApply(addScene("01", "s02"));

    expect(await manuscriptSceneLabels("01")).toEqual([
      "招かれざる客",
      "雨上がり",
      "遺言状の間",
      "消えた甥",
    ]);
  });

  it("入力した項目が章立てに入り、空の項目は入れない", async () => {
    await planAndApply(
      addScene(
        "01",
        null,
        scenePlan({
          summary: "  雨が上がる。 ",
          pov: "霧島 凛",
          characters: ["霧島 凛", " ", "佐藤 健二"],
          target_chars: 1200,
        }),
      ),
    );

    const { document } = await backend.readDocument("plot/chapters/01.md");
    if (document.kind !== "chapter") {
      throw new Error("章立てとして読めるはず");
    }
    expect(document.meta.scenes?.[3]).toMatchObject({
      id: "s04",
      title: "雨上がり",
      summary: "雨が上がる。",
      pov: "霧島 凛",
      characters: ["霧島 凛", "佐藤 健二"],
      target_chars: 1200,
    });
  });

  it("シーン構成がまだ無い章には、s01 として足せる", async () => {
    await planAndApply(addScene("02", null));

    expect(await manuscriptSceneLabels("02")).toEqual(["雨上がり"]);
    const pipeline = await backend.pipeline();
    expect(pipeline.some((step) => step.label === "灯台のある岬 雨上がり")).toBe(true);
  });

  it("題が空なら invalid_input、章や位置が無ければ not_found", async () => {
    await expect(
      backend.planStructureEdit(addScene("01", null, scenePlan({ title: " " }))),
    ).rejects.toMatchObject({ kind: "invalid_input" });
    await expect(backend.planStructureEdit(addScene("09", null))).rejects.toMatchObject({
      kind: "not_found",
    });
    await expect(backend.planStructureEdit(addScene("01", "s99"))).rejects.toMatchObject({
      kind: "not_found",
    });
  });

  it("確かめたあとに章立てが書き換えられたら、競合で何も変えない", async () => {
    const plan = await backend.planStructureEdit(addScene("01", null));
    const { document, hash } = await backend.readDocument("plot/chapters/01.md");
    if (document.kind !== "chapter") {
      throw new Error("章立てとして読めるはず");
    }
    await backend.writeDocument(
      "plot/chapters/01.md",
      { ...document, meta: { ...document.meta, title: "外で直した章題" } },
      hash,
    );

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await manuscriptSceneLabels("01")).toHaveLength(3);
  });
});

describe("シーンを消す", () => {
  const removeScene = (scene: string): StructureEdit => ({
    kind: "remove_scene",
    chapter: "01",
    scene,
  });

  it("本文のあるシーンは、章立ての書き換えと本文のゴミ箱への移動を、本文の字数つきで返す", async () => {
    const plan = await backend.planStructureEdit(removeScene("s01"));

    expect(plan.change_set.summary).toBe("第1章のシーン「招かれざる客」を削除します。");
    expect(writeChanges(plan.change_set).map((file) => file.path)).toEqual(["plot/chapters/01.md"]);
    const [trash] = trashChanges(plan.change_set);
    expect(trash?.path).toBe("manuscript/01/s01.txt");
    expect(trash?.files[0]?.chars).toBeGreaterThan(0);
  });

  it("本文の無いシーンは、章立てを書き換えるだけ", async () => {
    const plan = await backend.planStructureEdit(removeScene("s03"));

    expect(plan.change_set.summary).toBe("第1章のシーン「消えた甥」を削除します。");
    expect(trashChanges(plan.change_set)).toHaveLength(0);
  });

  it("適用すると、章立てからも本文からも消え、ほかのシーンの本文は残る", async () => {
    await planAndApply(removeScene("s01"));

    expect(await manuscriptSceneLabels("01")).toEqual(["遺言状の間", "消えた甥"]);
    await expect(backend.readDocument("manuscript/01/s01.txt")).rejects.toMatchObject({
      kind: "not_found",
    });
    expect(await readText(backend, "manuscript/01/s02.txt")).toContain("書斎には");
  });

  it("消したシーンの番号は、あとから足したシーンに引き継がれても、本文は引き継がない", async () => {
    await planAndApply(removeScene("s01"));
    await planAndApply({ kind: "add_scene", chapter: "01", before: null, scene: scenePlan() });

    const manuscript = (await backend.overview()).sections.find(
      (section) => section.kind === "manuscript",
    );
    const added = manuscript?.entries[0]?.children.at(-1);
    expect(added).toMatchObject({ scene: "s01", exists: false });
  });

  it("確かめたあとに本文が書き換えられたら、競合で何も変えない（章立ても残る）", async () => {
    const plan = await backend.planStructureEdit(removeScene("s01"));
    const { hash } = await backend.readDocument("manuscript/01/s01.txt");
    await backend.writeDocument("manuscript/01/s01.txt", textDocument("外で書き足した本文"), hash);

    await expect(backend.applyChangeSet(plan.change_set)).rejects.toMatchObject({
      kind: "conflict",
    });

    expect(await manuscriptSceneLabels("01")).toHaveLength(3);
    expect(await readText(backend, "manuscript/01/s01.txt")).toBe("外で書き足した本文");
  });

  it("章やシーンが無ければ not_found", async () => {
    await expect(backend.planStructureEdit(removeScene("s99"))).rejects.toMatchObject({
      kind: "not_found",
    });
    await expect(
      backend.planStructureEdit({ kind: "remove_scene", chapter: "09", scene: "s01" }),
    ).rejects.toMatchObject({ kind: "not_found" });
  });
});

describe("変更案の適用の検証", () => {
  it("同じパスへの変更が重なる変更案は invalid_input で、何も変えない", async () => {
    const plan = await backend.planStructureEdit(removeCharacter("sato-kenji"));
    const duplicated = {
      ...plan.change_set,
      files: [...plan.change_set.files, ...plan.change_set.files],
    };

    await expect(backend.applyChangeSet(duplicated)).rejects.toMatchObject({
      kind: "invalid_input",
    });

    await expect(backend.readDocument("characters/sato-kenji.md")).resolves.toBeDefined();
  });

  it.each([
    "kataribe.yaml",
    "Kataribe.yaml",
    ".kataribe/cache/summary.json",
    ".Kataribe/cache/summary.json",
    ".KATARIBE",
  ])("%s はゴミ箱へ移せない（大文字小文字の違いも拒む。invalid_input）", async (path) => {
    const plan = await backend.planStructureEdit(removeCharacter("sato-kenji"));
    const trash = {
      kind: "trash" as const,
      path,
      files: [{ path, base_hash: "00000000", chars: 0 }],
    };

    await expect(
      backend.applyChangeSet({ ...plan.change_set, files: [trash] }),
    ).rejects.toMatchObject({ kind: "invalid_input" });
  });

  it("ゴミ箱へ移す変更の、移すファイルの一覧が対象と同じ 1 つでなければ invalid_input で、何も変えない", async () => {
    const plan = await backend.planStructureEdit(removeCharacter("sato-kenji"));
    const [trash] = trashChanges(plan.change_set);
    if (trash === undefined) {
      throw new Error("ゴミ箱へ移す変更があるはず");
    }
    const [only] = trash.files;
    if (only === undefined) {
      throw new Error("移すファイルがあるはず");
    }
    const mismatches = [
      [],
      [only, only],
      [{ ...only, path: "characters/kirishima-rin.md" }],
      [only, { ...only, path: "concept.md" }],
    ];

    for (const files of mismatches) {
      await expect(
        backend.applyChangeSet({ ...plan.change_set, files: [{ ...trash, files }] }),
      ).rejects.toMatchObject({ kind: "invalid_input" });
    }

    await expect(backend.readDocument("characters/sato-kenji.md")).resolves.toBeDefined();
  });

  it("別の作品の変更案は適用しない", async () => {
    const plan = await backend.planStructureEdit(addCharacter("niiyama-yuki"));

    await expect(
      backend.applyChangeSet({ ...plan.change_set, project_root: "C:\\other" }),
    ).rejects.toMatchObject({ kind: "invalid_input" });
  });
});
