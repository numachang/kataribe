import { describe, expect, it } from "vitest";
import type { ChangeSet, TrashedFile } from "../api/types";
import { manuscriptAmong, trashedFilesOf } from "./trashSummary";

function trashed(path: string, chars: number): TrashedFile {
  return { path, base_hash: "abcd", chars };
}

describe("trashedFilesOf", () => {
  it("書き込みは含めず、ゴミ箱へ移すファイルを並びのまま集める", () => {
    const changeSet: ChangeSet = {
      summary: "",
      project_root: "C:work",
      files: [
        { kind: "write", path: "plot/chapters/01.md", content: "", previous: "", base_hash: null },
        { kind: "trash", path: "characters/a.md", files: [trashed("characters/a.md", 10)] },
        { kind: "trash", path: "world/b.md", files: [trashed("world/b.md", 20)] },
      ],
    };

    expect(trashedFilesOf(changeSet).map((file) => file.path)).toEqual([
      "characters/a.md",
      "world/b.md",
    ]);
  });
});

describe("trashedFilesOf（フォルダ）", () => {
  it("フォルダをゴミ箱へ移す変更は、中のファイル全部を数える", () => {
    const changeSet: ChangeSet = {
      summary: "",
      project_root: "C:work",
      files: [
        { kind: "trash", path: "plot/chapters/02.md", files: [trashed("plot/chapters/02.md", 30)] },
        {
          kind: "trash",
          path: "manuscript/02",
          files: [trashed("manuscript/02/s01.txt", 1000), trashed("manuscript/02/s02.txt", 500)],
        },
        { kind: "move", from: "plot/chapters/03.md", to: "plot/chapters/02.md" },
        { kind: "expect", path: "manuscript/03", base_hash: null },
      ],
    };

    const files = trashedFilesOf(changeSet);

    expect(files.map((file) => file.path)).toEqual([
      "plot/chapters/02.md",
      "manuscript/02/s01.txt",
      "manuscript/02/s02.txt",
    ]);
    expect(manuscriptAmong(files)).toEqual({ count: 2, chars: 1500 });
  });
});

describe("manuscriptAmong", () => {
  it("本文（manuscript/ の下）のファイルの数と字数の合計だけを数える", () => {
    const files = [
      trashed("manuscript/01/s01.txt", 1000),
      trashed("manuscript/01/s02.txt", 3210),
      trashed("characters/a.md", 500),
    ];

    expect(manuscriptAmong(files)).toEqual({ count: 2, chars: 4210 });
  });

  it("本文が無ければ 0", () => {
    expect(manuscriptAmong([trashed("world/b.md", 20)])).toEqual({ count: 0, chars: 0 });
  });
});
