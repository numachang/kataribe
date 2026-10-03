import { describe, expect, it } from "vitest";
import type { ChangeSet, FileChange, Task } from "../api/types";
import { documentCreatedBy } from "./documentCreatedBy";

const ADD_CHARACTER: Task = { kind: "add_character", instruction: "幼なじみ" };
const ADD_WORLD_DOCUMENT: Task = { kind: "add_world_document", name: null, instruction: "天気" };

function write(path: string, previous: string | null): FileChange {
  return { kind: "write", path, content: "内容", previous, base_hash: null };
}

function changeSetOf(...files: FileChange[]): ChangeSet {
  return { summary: "", files, project_root: "C:work" };
}

describe("documentCreatedBy", () => {
  it("人物の追加は、新しく書いた人物資料のパスを返す", () => {
    const changeSet = changeSetOf(write("characters/sato-kenji.md", null));

    expect(documentCreatedBy(ADD_CHARACTER, changeSet)).toBe("characters/sato-kenji.md");
  });

  it("世界観の資料の追加は、新しく書いた資料のパスを返す", () => {
    const changeSet = changeSetOf(write("world/weather.md", null));

    expect(documentCreatedBy(ADD_WORLD_DOCUMENT, changeSet)).toBe("world/weather.md");
  });

  it("既にあるファイルの書き直しは、作った文書に数えない", () => {
    const changeSet = changeSetOf(write("characters/sato-kenji.md", "古い内容"));

    expect(documentCreatedBy(ADD_CHARACTER, changeSet)).toBeNull();
  });

  it("書き直しと新規が混ざっていれば、新規の方を返す", () => {
    const changeSet = changeSetOf(
      write("characters/rin.md", "古い内容"),
      write("characters/sato-kenji.md", null),
    );

    expect(documentCreatedBy(ADD_CHARACTER, changeSet)).toBe("characters/sato-kenji.md");
  });

  it("新しく書くファイルが無い変更案には、開く文書が無い", () => {
    expect(documentCreatedBy(ADD_CHARACTER, changeSetOf())).toBeNull();
  });

  it("工程と書き直しの生成は、新しいファイルができても開かない", () => {
    const changeSet = changeSetOf(write("concept.md", null));

    expect(documentCreatedBy({ kind: "concept" }, changeSet)).toBeNull();
    expect(
      documentCreatedBy({ kind: "revise", path: "concept.md", instruction: "短く" }, changeSet),
    ).toBeNull();
  });
});
