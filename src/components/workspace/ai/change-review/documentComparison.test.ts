import { describe, expect, it } from "vitest";
import type {
  ChapterMeta,
  CharacterMeta,
  EditableDocument,
  ScenePlan,
} from "../../../../api/types";
import {
  buildDocumentView,
  type DocumentView,
  type FieldView,
  type StructuredDocument,
} from "./documentComparison";

/** アプリが知らない項目（Rust が JSON の meta 直下に載せてくる）を足した meta。型は知らないので、値を足して作る。 */
function withExtra<Meta extends object>(meta: Meta, extra: Record<string, unknown>): Meta {
  return { ...meta, ...extra };
}

function character(meta: Partial<CharacterMeta> = {}, body = "本文"): StructuredDocument {
  return {
    kind: "character",
    meta: { name: "霧島 凛", role: "主人公", summary: "盲目の探偵", ...meta },
    body,
  };
}

function scene(id: string, overrides: Partial<ScenePlan> = {}): ScenePlan {
  return { id, title: `${id}の題`, summary: `${id}の概要`, ...overrides };
}

function chapter(
  scenes: ScenePlan[],
  meta: Partial<ChapterMeta> = {},
  body = "ストーリーライン",
): StructuredDocument {
  return { kind: "chapter", meta: { title: "雨の匂い", scenes, ...meta }, body };
}

function field(fields: FieldView[], label: string): FieldView {
  const found = fields.find((candidate) => candidate.label === label);
  if (found === undefined) {
    throw new Error(`項目「${label}」が無い: ${fields.map((candidate) => candidate.label)}`);
  }
  return found;
}

function sceneView(view: DocumentView, id: string) {
  const found = view.scenes.find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`シーン「${id}」が無い: ${view.scenes.map((candidate) => candidate.id)}`);
  }
  return found;
}

function view(
  document: StructuredDocument,
  counterpart: EditableDocument | null,
  side: "before" | "after" = "after",
): DocumentView {
  return buildDocumentView(document, counterpart, side);
}

describe("buildDocumentView / 項目の比較", () => {
  it("相手が無ければ、どの項目にも印を付けず、比べていないと伝える", () => {
    const result = view(character(), null);

    expect(result.fields.every((candidate) => !candidate.changed)).toBe(true);
    expect(result.bodyChanged).toBe(false);
    expect(result.compared).toBe(false);
  });

  it("文書の種類が違う相手とは比べない", () => {
    const result = view(character(), chapter([]));

    expect(result.compared).toBe(false);
    expect(result.fields.every((candidate) => !candidate.changed)).toBe(true);
  });

  it("変わった項目にだけ印を付ける", () => {
    const result = view(character({ role: "探偵" }), character({ role: "主人公" }));

    expect(result.compared).toBe(true);
    expect(field(result.fields, "役割").changed).toBe(true);
    expect(field(result.fields, "名前").changed).toBe(false);
  });

  it("読みが null の項目と空文字の項目は、同じ値とみなす", () => {
    const result = view(character({ reading: "" }), character({ reading: null }));

    expect(field(result.fields, "読み").changed).toBe(false);
  });

  it("読みが無い項目も、null や空文字と同じ値とみなす", () => {
    const result = view(character({}), character({ reading: "" }));

    expect(field(result.fields, "読み").changed).toBe(false);
  });

  it("本文が変わったときは bodyChanged になる", () => {
    const result = view(character({}, "新しい"), character({}, "古い"));

    expect(result.bodyChanged).toBe(true);
  });
});

describe("buildDocumentView / ビート", () => {
  const beats = ["依頼が届く", "現場へ向かう"];

  it("変更前にあったビートが変更後に無いときは、変更後に「ビート」を空の項目として出して印を付ける", () => {
    const result = view(chapter([scene("s01")]), chapter([scene("s01", { beats })]));

    const beatField = field(sceneView(result, "s01").fields, "ビート");
    expect(beatField.value).toBe("");
    expect(beatField.changed).toBe(true);
    expect(sceneView(result, "s01").marks).toEqual(["changed"]);
  });

  it("変更前のシーンを見ると、あったビートに印が付く", () => {
    const result = view(chapter([scene("s01", { beats })]), chapter([scene("s01")]), "before");

    const beatField = field(sceneView(result, "s01").fields, "ビート");
    expect(beatField.value).toEqual(beats);
    expect(beatField.changed).toBe(true);
  });

  it("変更後にビートが増えたときも印が付く", () => {
    const result = view(chapter([scene("s01", { beats })]), chapter([scene("s01")]));

    expect(field(sceneView(result, "s01").fields, "ビート").changed).toBe(true);
  });

  it("ビートの中身が変わったときだけ印が付き、同じなら付かない", () => {
    const changed = view(
      chapter([scene("s01", { beats: ["新しい"] })]),
      chapter([scene("s01", { beats: ["古い"] })]),
    );
    const same = view(
      chapter([scene("s01", { beats })]),
      chapter([scene("s01", { beats: [...beats] })]),
    );

    expect(field(sceneView(changed, "s01").fields, "ビート").changed).toBe(true);
    expect(field(sceneView(same, "s01").fields, "ビート").changed).toBe(false);
    expect(sceneView(same, "s01").marks).toEqual([]);
  });
});

describe("buildDocumentView / シーンの追加・削除・並び替え", () => {
  const before = chapter([scene("s01"), scene("s02"), scene("s03")]);

  it("変更後を見ると、増えたシーンは追加、減ったシーンは末尾に削除で出る", () => {
    const after = chapter([scene("s01"), scene("s04")]);

    const result = view(after, before);

    expect(result.scenes.map((candidate) => [candidate.id, candidate.marks])).toEqual([
      ["s01", []],
      ["s04", ["added"]],
      ["s02", ["removed"]],
      ["s03", ["removed"]],
    ]);
  });

  it("変更前を見ると、変更後に無いシーンが削除になり、変更後にだけあるシーンは出ない", () => {
    const after = chapter([scene("s01"), scene("s04")]);

    const result = view(before, after, "before");

    expect(result.scenes.map((candidate) => [candidate.id, candidate.marks])).toEqual([
      ["s01", []],
      ["s02", ["removed"]],
      ["s03", ["removed"]],
    ]);
  });

  it("並びが入れ替わったシーンには「順序変更」の印を付ける", () => {
    const after = chapter([scene("s02"), scene("s01"), scene("s03")]);

    const result = view(after, before);

    const moved = result.scenes.filter((candidate) => candidate.marks.includes("moved"));
    expect(moved).toHaveLength(1);
    expect(result.scenes.map((candidate) => candidate.id)).toEqual(["s02", "s01", "s03"]);
  });

  it("1 つを先頭から末尾へ移したときは、移したシーンだけに印を付ける", () => {
    const after = chapter([scene("s02"), scene("s03"), scene("s01")]);

    const result = view(after, before);

    expect(sceneView(result, "s01").marks).toEqual(["moved"]);
    expect(sceneView(result, "s02").marks).toEqual([]);
    expect(sceneView(result, "s03").marks).toEqual([]);
  });

  it("項目も変わって並びも変わったシーンには、両方の印を付ける", () => {
    const after = chapter([scene("s02"), scene("s03"), scene("s01", { title: "題を変えた" })]);

    const result = view(after, before);

    expect(sceneView(result, "s01").marks).toEqual(["changed", "moved"]);
  });

  it("追加・削除したシーンがあっても、残ったシーンの並びが同じなら順序変更にしない", () => {
    const after = chapter([scene("s00"), scene("s01"), scene("s03")]);

    const result = view(after, before);

    expect(sceneView(result, "s01").marks).toEqual([]);
    expect(sceneView(result, "s03").marks).toEqual([]);
  });

  it("変更前を見ても、並び替えに印が付く", () => {
    const after = chapter([scene("s02"), scene("s03"), scene("s01")]);

    const result = view(before, after, "before");

    expect(sceneView(result, "s01").marks).toEqual(["moved"]);
  });
});

describe("buildDocumentView / アプリが知らない項目（extra）", () => {
  it("人物資料の知らない項目を「その他の項目」に、キー名と値で出す", () => {
    const meta = withExtra(
      { name: "霧島 凛", role: "主人公", summary: "探偵" },
      { theme: "喪失", priority: 3, tags: ["探偵", "盲目"] },
    );

    const result = view({ kind: "character", meta, body: "" }, null);

    expect(result.extraFields.map((candidate) => [candidate.label, candidate.value])).toEqual([
      ["theme", "喪失"],
      ["priority", "3"],
      ["tags", JSON.stringify(["探偵", "盲目"], null, 2)],
    ]);
    expect(result.fields.map((candidate) => candidate.label)).not.toContain("theme");
  });

  it("知らない項目が無ければ、「その他の項目」は空", () => {
    expect(view(character(), null).extraFields).toEqual([]);
  });

  it("人物資料の知らない項目の、変更・追加・削除が分かる", () => {
    const before = character(withExtra({}, { theme: "喪失", note: "消える", same: "同じ" }));
    const after = character(withExtra({}, { theme: "再生", added: "増えた", same: "同じ" }));

    const result = view(after, before);

    expect(field(result.extraFields, "theme").changed).toBe(true);
    expect(field(result.extraFields, "added").changed).toBe(true);
    expect(field(result.extraFields, "same").changed).toBe(false);
    const dropped = field(result.extraFields, "note");
    expect(dropped.value).toBe("");
    expect(dropped.changed).toBe(true);
  });

  it('文字列の "3" と数値の 3 は、見た目が同じでも違う値として印を付ける', () => {
    const result = view(
      character(withExtra({}, { priority: "3" })),
      character(withExtra({}, { priority: 3 })),
    );

    expect(field(result.extraFields, "priority").changed).toBe(true);
  });

  it("章立てのトップレベルの知らない項目も出して比べる。scenes は項目ではない", () => {
    const before = chapter([scene("s01")], withExtra({}, { theme: "喪失" }));
    const after = chapter([scene("s01")], withExtra({}, { theme: "再生" }));

    const result = view(after, before);

    expect(result.extraFields.map((candidate) => candidate.label)).toEqual(["theme"]);
    expect(field(result.extraFields, "theme").changed).toBe(true);
  });

  it("シーンの知らない項目の、変更・追加・削除が分かり、シーンの印にもなる", () => {
    const before = chapter([
      scene("s01", withExtra({}, { mood: "暗い", weather: "雨" })),
      scene("s02", withExtra({}, { mood: "静か" })),
    ]);
    const after = chapter([
      scene("s01", withExtra({}, { mood: "明るい", season: "夏" })),
      scene("s02", withExtra({}, { mood: "静か" })),
    ]);

    const result = view(after, before);

    const first = sceneView(result, "s01");
    expect(field(first.extraFields, "mood").changed).toBe(true);
    expect(field(first.extraFields, "season").changed).toBe(true);
    const dropped = field(first.extraFields, "weather");
    expect(dropped.value).toBe("");
    expect(dropped.changed).toBe(true);
    expect(first.marks).toEqual(["changed"]);
    expect(sceneView(result, "s02").marks).toEqual([]);
  });

  it("シーンの id は知らない項目に出さない", () => {
    const result = view(chapter([scene("s01")]), null);

    expect(sceneView(result, "s01").extraFields).toEqual([]);
  });
});
