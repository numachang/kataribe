import { describe, expect, it } from "vitest";
import type { ChangeSet, FileChange } from "../api/types";
import { changeKey, relocatedPath, touchesPath } from "./changeSetPaths";

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

describe("relocatedPath", () => {
  it("ゴミ箱へ移る文書は null", () => {
    expect(relocatedPath(CHANGE_SET, "manuscript/01/s01.txt")).toBeNull();
  });

  it("書き換えるだけの文書と、触れない文書は undefined", () => {
    expect(relocatedPath(CHANGE_SET, "plot/chapters/01.md")).toBeUndefined();
    expect(relocatedPath(CHANGE_SET, "concept.md")).toBeUndefined();
  });
});

describe("changeKey", () => {
  it("同じパスでも、変更の種類が違えば別の key になる", () => {
    expect(changeKey(WRITE)).not.toBe(changeKey({ ...TRASH, path: WRITE.path }));
    expect(changeKey(WRITE)).toBe("write:plot/chapters/01.md");
  });
});
