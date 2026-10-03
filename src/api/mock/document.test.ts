import { describe, expect, it } from "vitest";
import { parseMockDocument, readMockDocument, writeMockDocument } from "./document";
import { buildPipeline } from "./pipeline";
import { PLACEHOLDER_CHARACTER_BODY, readMockFile } from "./render";
import { createSampleProjectState, SAMPLE_PROJECT_FOLDER } from "./sampleProject";
import type { ProjectState } from "./state";

function sampleWithBeats(): ProjectState {
  const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);
  const chapters = (state.chapters ?? []).map((chapter) => ({
    ...chapter,
    scenes:
      chapter.scenes?.map((scene) =>
        scene.id === "s01" ? { ...scene, beats: ["依頼人が現れる。", "雨音が強まる。"] } : scene,
      ) ?? null,
  }));
  return { ...state, chapters };
}

function readChapter(state: ProjectState) {
  const document = readMockDocument(state, "plot/chapters/01.md");
  if (document?.kind !== "chapter") {
    throw new Error("章立てとして読めるはず");
  }
  return document;
}

describe("readMockDocument", () => {
  it("人物資料を項目と本文に分けて返し、空文字の項目は null にする", () => {
    const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);
    const characters = (state.characters ?? []).map((character) =>
      character.id === "sato-kenji" ? { ...character, reading: "", order: null } : character,
    );

    const document = readMockDocument({ ...state, characters }, "characters/sato-kenji.md");

    expect(document).toMatchObject({
      kind: "character",
      meta: { name: "佐藤 健二", reading: null, order: null },
    });
    expect(document?.kind === "character" && document.body).toContain("## 外見");
  });

  it("章立てを章題・シーン・ストーリーラインに分けて返し、ビートは入っているときだけ付ける", () => {
    const document = readChapter(sampleWithBeats());

    expect(document.meta.title).toBe("雨の匂い");
    expect(document.body).toContain("柱時計の狂い");
    const [first, second] = document.meta.scenes ?? [];
    expect(first?.beats).toEqual(["依頼人が現れる。", "雨音が強まる。"]);
    expect(second?.beats).toBeUndefined();
    expect(first?.characters).toEqual(["霧島 凛", "佐藤 健二"]);
  });

  it("シーンが無い章では scenes を省く", () => {
    const document = readMockDocument(
      createSampleProjectState(SAMPLE_PROJECT_FOLDER),
      "plot/chapters/02.md",
    );

    expect(document?.kind === "chapter" && document.meta).toEqual({ title: "灯台のある岬" });
  });

  it("それ以外の文書と、無い文書", () => {
    const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);

    expect(readMockDocument(state, "concept.md")).toMatchObject({ kind: "text" });
    expect(readMockDocument(state, "characters/nobody.md")).toBeNull();
  });
});

describe("writeMockDocument", () => {
  it("章立てを保存しても、同じ id のシーンの本文とビートは残る", () => {
    const state = sampleWithBeats();
    const document = readChapter(state);
    const scenes = (document.meta.scenes ?? []).map((scene) =>
      scene.id === "s01" ? { ...scene, title: "招かれた客" } : scene,
    );

    const next = writeMockDocument(state, "plot/chapters/01.md", {
      ...document,
      meta: { ...document.meta, scenes },
    });

    const firstScene = next.chapters?.[0]?.scenes?.[0];
    expect(firstScene?.title).toBe("招かれた客");
    expect(firstScene?.beats).toEqual(["依頼人が現れる。", "雨音が強まる。"]);
    expect(firstScene?.draft).toContain("館の扉が開くたび");
  });

  it("人物の読みと順番を空にして保存すると、読み直しても null で返る", () => {
    const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);
    const document = readMockDocument(state, "characters/kirishima-rin.md");
    if (document?.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }

    const next = writeMockDocument(state, "characters/kirishima-rin.md", {
      ...document,
      meta: { ...document.meta, reading: null, order: null },
    });

    expect(readMockDocument(next, "characters/kirishima-rin.md")).toMatchObject({
      meta: { reading: null, order: null },
    });
  });

  it("人物の本文を空にして保存すると、読み直しても空のまま（プレースホルダーに戻らない）", () => {
    const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);
    const document = readMockDocument(state, "characters/kirishima-rin.md");
    if (document?.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }

    const next = writeMockDocument(state, "characters/kirishima-rin.md", { ...document, body: "" });

    expect(readMockDocument(next, "characters/kirishima-rin.md")).toMatchObject({ body: "" });
    expect(readMockFile(next, "characters/kirishima-rin.md")?.endsWith("---\n")).toBe(true);
  });

  it("詳細が未生成の人物は、本文がプレースホルダーのまま名前だけ直して保存しても、未生成のまま", () => {
    const sample = createSampleProjectState(SAMPLE_PROJECT_FOLDER);
    const state = {
      ...sample,
      characters: (sample.characters ?? []).map((character) =>
        character.id === "sato-kenji" ? { ...character, detail: null } : character,
      ),
    };
    const document = readMockDocument(state, "characters/sato-kenji.md");
    if (document?.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }
    expect(document.body).toBe(PLACEHOLDER_CHARACTER_BODY);

    const next = writeMockDocument(state, "characters/sato-kenji.md", {
      ...document,
      meta: { ...document.meta, name: "佐藤 健" },
    });

    expect(next.characters?.find((character) => character.id === "sato-kenji")?.detail).toBeNull();
    const satoStep = buildPipeline(next).find(
      (step) => step.task.kind === "character" && step.task.id === "sato-kenji",
    );
    expect(satoStep?.state).toBe("ready");
  });

  it("人物の詳細を空にして保存すると、生成済みとは数えない", () => {
    const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);
    const document = readMockDocument(state, "characters/sato-kenji.md");
    if (document?.kind !== "character") {
      throw new Error("人物資料として読めるはず");
    }

    const next = writeMockDocument(state, "characters/sato-kenji.md", { ...document, body: "" });

    const satoStep = buildPipeline(next).find(
      (step) => step.task.kind === "character" && step.task.id === "sato-kenji",
    );
    expect(satoStep?.state).toBe("ready");
  });

  it.each([
    "characters/Kirishima.md",
    "characters/kirishima_rin.md",
    "characters/-rin.md",
    "characters/rin-.md",
    "characters/rin--ka.md",
    "characters/con.md",
    "characters/com1.md",
    `characters/${"a".repeat(49)}.md`,
    "characters/霧島.md",
  ])(
    "人物資料を、人物 id として無効なパス %s へ保存しようとすると invalid_input で失敗する",
    (path) => {
      const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);

      expect(() =>
        writeMockDocument(state, path, {
          kind: "character",
          meta: { name: "名", role: "", summary: "" },
          body: "",
        }),
      ).toThrowError(expect.objectContaining({ kind: "invalid_input" }));
    },
  );

  it.each([
    "plot/chapters/1.md",
    "plot/chapters/1000.md",
    "plot/chapters/ab.md",
    "plot/chapters/0a.md",
    "plot/chapters/01/02.md",
  ])(
    "章立てを、章 id として無効なパス %s へ保存しようとすると invalid_input で失敗する",
    (path) => {
      const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);

      expect(() =>
        writeMockDocument(state, path, { kind: "chapter", meta: { title: "章" }, body: "" }),
      ).toThrowError(expect.objectContaining({ kind: "invalid_input" }));
    },
  );

  it("有効な id のパスには保存できる（最長 48 字の人物 id、3 桁の章 id）", () => {
    const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);
    const longId = `${"a".repeat(23)}-${"b".repeat(24)}`;

    const withCharacter = writeMockDocument(state, `characters/${longId}.md`, {
      kind: "character",
      meta: { name: "名", role: "", summary: "" },
      body: "本文",
    });
    const withChapter = writeMockDocument(state, "plot/chapters/100.md", {
      kind: "chapter",
      meta: { title: "章" },
      body: "",
    });

    expect(readMockDocument(withCharacter, `characters/${longId}.md`)).toMatchObject({
      kind: "character",
    });
    expect(readMockDocument(withChapter, "plot/chapters/100.md")).toMatchObject({
      kind: "chapter",
    });
  });

  it("人物資料・章立てを、種類の合わないパスへ保存しようとすると invalid_input で失敗する", () => {
    const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);

    expect(() =>
      writeMockDocument(state, "concept.md", {
        kind: "character",
        meta: { name: "名", role: "", summary: "" },
        body: "",
      }),
    ).toThrowError(expect.objectContaining({ kind: "invalid_input" }));
  });
});

describe("parseMockDocument", () => {
  const state = createSampleProjectState(SAMPLE_PROJECT_FOLDER);

  function fileContent(path: string): string {
    const content = readMockFile(state, path);
    if (content === null) {
      throw new Error(`サンプルの作品に「${path}」があるはず`);
    }
    return content;
  }

  it.each(["characters/kirishima-rin.md", "plot/chapters/01.md", "plot/chapters/02.md"])(
    "「%s」を、読み込んだときと同じ項目と本文に分ける",
    (path) => {
      const parsed = parseMockDocument(path, fileContent(path));

      expect(parsed).toEqual({ document: readMockDocument(state, path), parse_error: null });
    },
  );

  it("front matter に無い人物の順番は null にする", () => {
    const parsed = parseMockDocument("characters/rin.md", "---\nname: 凛\nrole: 探偵\n---\n本文\n");

    expect(parsed.document).toMatchObject({
      kind: "character",
      meta: { name: "凛", role: "探偵", order: null },
      body: "本文\n",
    });
  });

  it("front matter の無い人物資料・章立ては、文字列のまま理由を添えて返す", () => {
    const character = parseMockDocument("characters/rin.md", "本文だけ\n");
    const chapter = parseMockDocument("plot/chapters/01.md", "本文だけ\n");

    expect(character.document).toEqual({ kind: "text", content: "本文だけ\n" });
    expect(character.parse_error).toContain("characters/rin.md");
    expect(chapter.parse_error).toContain("plot/chapters/01.md");
  });

  it("必須の項目（人物の name、章の title）が無い内容は、本物と同じく文字列のまま理由を添えて返す", () => {
    const character = parseMockDocument("characters/rin.md", "---\nrole: 探偵\n---\n本文\n");
    const chapter = parseMockDocument("plot/chapters/01.md", "---\nscenes:\n---\n本文\n");

    expect(character.document).toEqual({ kind: "text", content: "---\nrole: 探偵\n---\n本文\n" });
    expect(character.parse_error).toContain("「name」");
    expect(chapter.document).toEqual({ kind: "text", content: "---\nscenes:\n---\n本文\n" });
    expect(chapter.parse_error).toContain("「title」");
  });

  it.each(["", PLACEHOLDER_CHARACTER_BODY])("人物の本文「%s」を、書かれたまま返す", (body) => {
    const parsed = parseMockDocument("characters/rin.md", `---\nname: 凛\n---\n${body}`);

    expect(parsed.document).toMatchObject({ kind: "character", body });
  });

  it("シーンの id が重複した章立ては、文字列のまま理由を添えて返す", () => {
    const content = fileContent("plot/chapters/01.md").replace("id: s02", "id: s01");

    const parsed = parseMockDocument("plot/chapters/01.md", content);

    expect(parsed.document).toEqual({ kind: "text", content });
    expect(parsed.parse_error).toContain("「s01」");
    expect(parsed.parse_error).toContain("重複");
  });

  it("人物資料・章立て以外のパスは、理由なしで文字列のまま返す", () => {
    expect(parseMockDocument("concept.md", "---\nname: 凛\n---\n")).toEqual({
      document: { kind: "text", content: "---\nname: 凛\n---\n" },
      parse_error: null,
    });
  });
});
