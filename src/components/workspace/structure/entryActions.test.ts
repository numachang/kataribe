import { describe, expect, it } from "vitest";
import type { OverviewEntry } from "../../../api/types";
import { type EntryActionContext, entryActionsFor } from "./entryActions";

function entry(overrides: Partial<OverviewEntry>): OverviewEntry {
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

function character(id: string, error: string | null = null): OverviewEntry {
  return entry({ path: `characters/${id}.md`, label: id, kind: "character", error });
}

function planChapter(chapter: string, error: string | null = null): OverviewEntry {
  return entry({ path: `plot/chapters/${chapter}.md`, kind: "chapter", chapter, error });
}

/** 本文の節の章見出し（パスを持たない）。 */
function heading(chapter: string): OverviewEntry {
  return entry({ kind: "chapter", chapter, label: `第${chapter}章` });
}

function scene(chapter: string, id: string): OverviewEntry {
  return entry({ path: `manuscript/${chapter}/${id}.txt`, kind: "scene", chapter, scene: id });
}

/** 同じ階層の項目の並びから、メニューの判断材料を作る（プロットの節の章は、その並びの中の章）。 */
function contextOf(siblings: OverviewEntry[]): EntryActionContext {
  const planChapters = siblings.filter(
    (candidate) => candidate.kind === "chapter" && candidate.path !== null,
  );
  return { siblings, planChapters };
}

/** メニューの項目のうち、並べ替えだけを「名前 → 行き先の位置」で返す。 */
function movesOf(
  target: OverviewEntry,
  context: EntryActionContext | OverviewEntry[],
): Record<string, number> {
  const resolved = Array.isArray(context) ? contextOf(context) : context;
  const moves: Record<string, number> = {};
  for (const action of entryActionsFor(target, resolved)) {
    if (action.kind === "move") {
      moves[action.label] = action.edit.position;
    }
  }
  return moves;
}

describe("人物の並べ替え", () => {
  it("隣の人物と入れ替わる位置を送る。先頭には「上へ」、末尾には「下へ」を出さない", () => {
    const people = [character("a"), character("b"), character("c")];

    expect(movesOf(people[0] as OverviewEntry, people)).toEqual({ 下へ移す: 1 });
    expect(movesOf(people[1] as OverviewEntry, people)).toEqual({ 上へ移す: 0, 下へ移す: 2 });
    expect(movesOf(people[2] as OverviewEntry, people)).toEqual({ 上へ移す: 1 });
  });

  it("動かす人物は、ID ではなくパスで指す", () => {
    const people = [character("a"), character("b")];

    const [moveDown] = entryActionsFor(people[0] as OverviewEntry, contextOf(people));

    expect(moveDown).toMatchObject({
      kind: "move",
      edit: { kind: "move_character", path: "characters/a.md", position: 1 },
    });
  });

  it("読めない人物資料には並べ替えを出さず、削除だけが出る", () => {
    const people = [character("a"), character("broken", "YAML を読めません"), character("c")];

    const actions = entryActionsFor(people[1] as OverviewEntry, contextOf(people));

    expect(actions.map((action) => action.label)).toEqual(["削除"]);
  });

  it("読めない人物資料は飛ばして、その先の読める人物と入れ替わる位置を送る", () => {
    const people = [character("a"), character("broken", "YAML を読めません"), character("c")];

    expect(movesOf(people[0] as OverviewEntry, people)).toEqual({ 下へ移す: 2 });
    expect(movesOf(people[2] as OverviewEntry, people)).toEqual({ 上へ移す: 0 });
  });

  it("読める人物の後ろが読めない資料だけなら、「下へ」を出さない", () => {
    const people = [character("a"), character("b"), character("broken", "YAML を読めません")];

    expect(movesOf(people[1] as OverviewEntry, people)).toEqual({ 上へ移す: 0 });
  });

  it("人物が 1 人だけなら、並べ替えを出さない", () => {
    const people = [character("a")];

    expect(movesOf(people[0] as OverviewEntry, people)).toEqual({});
  });
});

describe("章の並べ替え", () => {
  it("あらすじの行を数えず、章の中での位置を送る", () => {
    const synopsis = entry({ path: "plot/synopsis.md", kind: "synopsis" });
    const chapters = [planChapter("01"), planChapter("02"), planChapter("03")];
    const plot = [synopsis, ...chapters];

    expect(movesOf(chapters[0] as OverviewEntry, plot)).toEqual({ 下へ移す: 1 });
    expect(movesOf(chapters[1] as OverviewEntry, plot)).toEqual({ 上へ移す: 0, 下へ移す: 2 });
    expect(movesOf(chapters[2] as OverviewEntry, plot)).toEqual({ 上へ移す: 1 });
  });

  it("番号が抜けていても、番号ではなく並びの位置を送る", () => {
    const chapters = [planChapter("01"), planChapter("02"), planChapter("05")];

    const [moveUp] = entryActionsFor(chapters[2] as OverviewEntry, contextOf(chapters)).filter(
      (action) => action.label === "上へ移す",
    );

    expect(moveUp).toMatchObject({
      edit: { kind: "move_chapter", chapter: "05", position: 1 },
    });
  });

  it("本文の章見出しにも、章の並べ替えを出す（シーンの操作と見分けられる名前で）", () => {
    const planChapters = [planChapter("01"), planChapter("02")];
    const headings = [heading("01"), heading("02")];

    const labels = entryActionsFor(headings[1] as OverviewEntry, {
      siblings: headings,
      planChapters,
    }).map((action) => action.label);

    expect(labels).toEqual(["シーンを追加", "章を上へ移す", "章を削除"]);
  });

  describe("YAML が読めない章立てが混じるとき", () => {
    // 本文の節は、読めない章を出さない。本物は、読めない章も含めた全部の章（番号順）の中で位置を数える
    const brokenFirst = [
      planChapter("01", "YAML を読めません"),
      planChapter("02"),
      planChapter("03"),
    ];
    const brokenThird = [
      planChapter("01"),
      planChapter("02"),
      planChapter("03", "YAML を読めません"),
    ];

    it("本文の章見出しでも、読めない章を数えた位置を送る（先に読めない章があるとき）", () => {
      const headings = [heading("02"), heading("03")];
      const context = { siblings: headings, planChapters: brokenFirst };

      expect(movesOf(headings[0] as OverviewEntry, context)).toEqual({
        章を上へ移す: 0,
        章を下へ移す: 2,
      });
      expect(movesOf(headings[1] as OverviewEntry, context)).toEqual({ 章を上へ移す: 1 });
    });

    it("本文の章見出しの「下へ」は、読めない章と入れ替わる位置を送る（後ろに読めない章があるとき）", () => {
      const headings = [heading("01"), heading("02")];
      const context = { siblings: headings, planChapters: brokenThird };

      expect(movesOf(headings[1] as OverviewEntry, context)).toEqual({
        章を上へ移す: 0,
        章を下へ移す: 2,
      });
    });

    it("章立ての行と本文の章見出しで、同じ章に同じ位置を送る", () => {
      const planRow = brokenFirst[1] as OverviewEntry;
      const headings = [heading("02"), heading("03")];

      const fromPlan = movesOf(planRow, { siblings: brokenFirst, planChapters: brokenFirst });
      const fromHeading = movesOf(headings[0] as OverviewEntry, {
        siblings: headings,
        planChapters: brokenFirst,
      });

      expect(fromPlan).toEqual({ 上へ移す: 0, 下へ移す: 2 });
      expect(fromHeading).toEqual({ 章を上へ移す: 0, 章を下へ移す: 2 });
    });

    it("読めない章立て自身の行も、並べ替えられる（番号だけで動かせるので）", () => {
      expect(movesOf(brokenFirst[0] as OverviewEntry, contextOf(brokenFirst))).toEqual({
        下へ移す: 1,
      });
    });
  });

  it("章が 1 つだけなら、並べ替えを出さない", () => {
    const chapters = [planChapter("01")];

    expect(movesOf(chapters[0] as OverviewEntry, chapters)).toEqual({});
  });

  it("章立てが無いときの仮の行（章を持たない）には何も出さない", () => {
    const placeholder = entry({ kind: "chapter", label: "章立て", exists: false });

    expect(entryActionsFor(placeholder, contextOf([placeholder]))).toEqual([]);
  });
});

describe("シーンの並べ替え", () => {
  it("章の中での位置を送る。先頭には「上へ」、末尾には「下へ」を出さない", () => {
    const scenes = [scene("01", "s01"), scene("01", "s02"), scene("01", "s03")];

    expect(movesOf(scenes[0] as OverviewEntry, scenes)).toEqual({ 下へ移す: 1 });
    expect(movesOf(scenes[1] as OverviewEntry, scenes)).toEqual({ 上へ移す: 0, 下へ移す: 2 });
    expect(movesOf(scenes[2] as OverviewEntry, scenes)).toEqual({ 上へ移す: 1 });
  });

  it("動かすシーンは、章とシーンの id で指す", () => {
    const scenes = [scene("02", "s07"), scene("02", "s08")];

    const [moveDown] = entryActionsFor(scenes[0] as OverviewEntry, contextOf(scenes)).filter(
      (action) => action.kind === "move",
    );

    expect(moveDown).toMatchObject({
      edit: { kind: "move_scene", chapter: "02", scene: "s07", position: 1 },
    });
  });
});
