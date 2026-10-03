import { describe, expect, it } from "vitest";
import { isValidSlug, slugProblem } from "./slug";

describe("slugProblem", () => {
  it("小文字の英数字とハイフンの名前は使える", () => {
    for (const slug of ["kirishima-rin", "rin2", "a", "7", "a".repeat(48)]) {
      expect(slugProblem(slug), slug).toBeNull();
      expect(isValidSlug(slug), slug).toBe(true);
    }
  });

  it("大文字・かな・漢字・空白・記号は、使えない理由を返す", () => {
    expect(slugProblem("Rin")).toBe("大文字は使えません。小文字にしてください。");
    expect(slugProblem("霧島")).toBe("小文字の英数字とハイフンだけにしてください。");
    expect(slugProblem("rin kirishima")).toBe("小文字の英数字とハイフンだけにしてください。");
    expect(slugProblem("rin_kirishima")).toBe("小文字の英数字とハイフンだけにしてください。");
  });

  it("ハイフンが先頭・末尾・連続にあると使えない", () => {
    expect(slugProblem("-rin")).toBe("先頭と末尾にハイフンは使えません。");
    expect(slugProblem("rin-")).toBe("先頭と末尾にハイフンは使えません。");
    expect(slugProblem("rin--kirishima")).toBe("ハイフンは続けて使えません。");
  });

  it("49 文字以上は長すぎる", () => {
    expect(slugProblem("a".repeat(49))).toBe("48 文字までにしてください。");
  });

  it("Windows の予約名は使えない", () => {
    expect(slugProblem("con")).toContain("予約");
    expect(isValidSlug("com1")).toBe(false);
    expect(isValidSlug("lpt9")).toBe(false);
    expect(isValidSlug("console")).toBe(true);
  });

  it("空文字は使えない（呼び出し側が「自動」として先に扱う）", () => {
    expect(isValidSlug("")).toBe(false);
  });
});
