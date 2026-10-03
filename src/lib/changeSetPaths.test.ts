import { describe, expect, it } from "vitest";
import type { ChangeSet, FileChange } from "../api/types";
import {
  changeKey,
  relocatedPath,
  renamesOpenDocument,
  rewritesPath,
  touchesPath,
} from "./changeSetPaths";

const WRITE: FileChange = {
  kind: "write",
  path: "plot/chapters/01.md",
  content: "章立て",
  previous: null,
  base_hash: null,
};
const TRASH: FileChange = {
  kind: "trash",
  path: "manuscript/01/s01.txt",
  files: [{ path: "manuscript/01/s01.txt", base_hash: "abcd", chars: 120 }],
};
const MOVE_FOLDER: FileChange = {
  kind: "move",
  from: "manuscript/03",
  to: "manuscript/02",
};
const MOVE_FILE: FileChange = {
  kind: "move",
  from: "plot/chapters/03.md",
  to: "plot/chapters/02.md",
};
const EXPECT: FileChange = { kind: "expect", path: "manuscript/05", base_hash: null };
const TRASH_FOLDER: FileChange = {
  kind: "trash",
  path: "manuscript/01",
  files: [{ path: "manuscript/01/s01.txt", base_hash: "abcd", chars: 120 }],
};
const CHAPTER_REMOVAL: ChangeSet = {
  summary: "",
  files: [TRASH_FOLDER, MOVE_FOLDER, MOVE_FILE, EXPECT],
  project_root: "C:work",
};
const CHANGE_SET: ChangeSet = { summary: "", files: [WRITE, TRASH], project_root: "C:work" };

describe("touchesPath", () => {
  it("書き込む文書も、ゴミ箱へ移す文書も、触れるものとして数える", () => {
    expect(touchesPath(CHANGE_SET, "plot/chapters/01.md")).toBe(true);
    expect(touchesPath(CHANGE_SET, "manuscript/01/s01.txt")).toBe(true);
  });

  it("変更案に無い文書には触れない", () => {
    expect(touchesPath(CHANGE_SET, "concept.md")).toBe(false);
  });
});

describe("touchesPath（移動・フォルダ・状態の確認）", () => {
  it("移動元のファイルと、移動元のフォルダの下の文書に触れる", () => {
    expect(touchesPath(CHAPTER_REMOVAL, "plot/chapters/03.md")).toBe(true);
    expect(touchesPath(CHAPTER_REMOVAL, "manuscript/03/s01.txt")).toBe(true);
  });

  it("ゴミ箱へ移すフォルダの下の文書に触れる", () => {
    expect(touchesPath(CHAPTER_REMOVAL, "manuscript/01/s02.txt")).toBe(true);
  });

  it("移動先や、状態を確かめるだけのパス、名前が前方一致するだけのパスには触れない", () => {
    expect(touchesPath(CHAPTER_REMOVAL, "manuscript/02/s01.txt")).toBe(false);
    expect(touchesPath(CHAPTER_REMOVAL, "manuscript/05/s01.txt")).toBe(false);
    expect(touchesPath(CHAPTER_REMOVAL, "manuscript/030/s01.txt")).toBe(false);
  });
});

describe("rewritesPath", () => {
  it("書き込む文書だけが、内容を書き換えるものになる（移動やゴミ箱へ移すだけの文書は違う）", () => {
    expect(rewritesPath(CHANGE_SET, "plot/chapters/01.md")).toBe(true);
    expect(rewritesPath(CHANGE_SET, "manuscript/01/s01.txt")).toBe(false);
    expect(rewritesPath(CHAPTER_REMOVAL, "plot/chapters/02.md")).toBe(false);
  });
});

describe("relocatedPath（移動）", () => {
  it("改名されるファイルは、新しいパス", () => {
    expect(relocatedPath(CHAPTER_REMOVAL, "plot/chapters/03.md")).toBe("plot/chapters/02.md");
  });

  it("改名されるフォルダの下の文書は、前の部分だけを置き換えたパス", () => {
    expect(relocatedPath(CHAPTER_REMOVAL, "manuscript/03/s01.txt")).toBe("manuscript/02/s01.txt");
  });

  it("ゴミ箱へ移すフォルダの下の文書は null", () => {
    expect(relocatedPath(CHAPTER_REMOVAL, "manuscript/01/s02.txt")).toBeNull();
  });

  it("番号をずらす連鎖（03 → 04、04 → 05）でも、1 回分だけ動く", () => {
    const chain: ChangeSet = {
      summary: "",
      project_root: "C:work",
      files: [
        { kind: "move", from: "manuscript/03", to: "manuscript/04" },
        { kind: "move", from: "manuscript/04", to: "manuscript/05" },
      ],
    };

    expect(relocatedPath(chain, "manuscript/03/s01.txt")).toBe("manuscript/04/s01.txt");
    expect(relocatedPath(chain, "manuscript/04/s01.txt")).toBe("manuscript/05/s01.txt");
  });

  it("移動先のパスや、状態を確かめるだけのパスは動かない（undefined）", () => {
    expect(relocatedPath(CHAPTER_REMOVAL, "manuscript/02/s01.txt")).toBeUndefined();
    expect(relocatedPath(CHAPTER_REMOVAL, "manuscript/05/s01.txt")).toBeUndefined();
  });
});

describe("relocatedPath", () => {
  it("ゴミ箱へ移る文書は null", () => {
    expect(relocatedPath(CHANGE_SET, "manuscript/01/s01.txt")).toBeNull();
  });

  it("書き換えるだけの文書と、触れない文書は undefined", () => {
    expect(relocatedPath(CHANGE_SET, "plot/chapters/01.md")).toBeUndefined();
    expect(relocatedPath(CHANGE_SET, "concept.md")).toBeUndefined();
  });
});

describe("renamesOpenDocument", () => {
  it("開いている文書が改名されるなら true", () => {
    expect(renamesOpenDocument(CHAPTER_REMOVAL, "plot/chapters/03.md")).toBe(true);
    expect(renamesOpenDocument(CHAPTER_REMOVAL, "manuscript/03/s01.txt")).toBe(true);
  });

  it("書き換えるだけ・ゴミ箱へ移る・触れない文書は、改名されない", () => {
    expect(renamesOpenDocument(CHANGE_SET, "plot/chapters/01.md")).toBe(false);
    expect(renamesOpenDocument(CHANGE_SET, "manuscript/01/s01.txt")).toBe(false);
    expect(renamesOpenDocument(CHANGE_SET, "concept.md")).toBe(false);
  });

  it("文書を開いていなければ、改名されるものは無い", () => {
    expect(renamesOpenDocument(CHAPTER_REMOVAL, null)).toBe(false);
  });
});

describe("changeKey（移動・状態の確認）", () => {
  it("移動は元と先で、状態の確認はパスで見分ける", () => {
    expect(changeKey(MOVE_FILE)).toBe("move:plot/chapters/03.md->plot/chapters/02.md");
    expect(changeKey(EXPECT)).toBe("expect:manuscript/05");
    expect(changeKey({ ...MOVE_FILE, to: "plot/chapters/04.md" })).not.toBe(changeKey(MOVE_FILE));
  });
});

describe("changeKey", () => {
  it("同じパスでも、変更の種類が違えば別の key になる", () => {
    expect(changeKey(WRITE)).not.toBe(changeKey({ ...TRASH, path: WRITE.path }));
    expect(changeKey(WRITE)).toBe("write:plot/chapters/01.md");
  });
});
