import { describe, expect, it } from "vitest";
import { type MockCharacter, sortedForDisplay } from "./state";

function character(id: string, order: number | null): MockCharacter {
  return { id, name: id, reading: "", role: "", summary: "", order, detail: null };
}

const idsOf = (characters: MockCharacter[]) => characters.map((candidate) => candidate.id);

describe("sortedForDisplay", () => {
  it("順番の小さい順に並べ、順番の無い人物は最後に置く", () => {
    const sorted = sortedForDisplay([character("c", null), character("b", 2), character("a", 1)]);

    expect(idsOf(sorted)).toEqual(["a", "b", "c"]);
  });

  it("同じ値どうしは、id の順ではなく、本物の目次と同じファイル名の順に並べる", () => {
    // 「rin-a.md」は「rin.md」より前（「-」は「.」より小さい）。id だけで比べると逆になる
    const sorted = sortedForDisplay([character("rin", null), character("rin-a", null)]);

    expect(idsOf(sorted)).toEqual(["rin-a", "rin"]);
  });

  it("元の配列を並べ替えない", () => {
    const original = [character("b", 2), character("a", 1)];

    sortedForDisplay(original);

    expect(idsOf(original)).toEqual(["b", "a"]);
  });
});
