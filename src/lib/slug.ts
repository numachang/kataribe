// ファイル名に使う slug（人物の ID・世界観の資料の名前）の規則。
// kataribe-project の `slug::is_valid`（`CharacterId::new` が使う）と同じ。画面は送る前の確認に、偽バックエンドは本物の検証の再現に使う。

const SLUG_MAX_LENGTH = 48;
const LOWERCASE_ALPHANUMERIC_OR_HYPHEN = /^[a-z0-9-]+$/;

// Windows が予約しているファイル名（kataribe-project の `RESERVED_STEMS` と同じ）。
const WINDOWS_RESERVED_NAMES = new Set([
  "con",
  "prn",
  "aux",
  "nul",
  ...Array.from({ length: 9 }, (_, index) => `com${index + 1}`),
  ...Array.from({ length: 9 }, (_, index) => `lpt${index + 1}`),
]);

/**
 * slug として使えない理由（日本語）。使えるなら null。
 * 規則は、小文字の英数字とハイフン。48 文字まで。先頭・末尾・連続にハイフンを置かず、Windows の予約名でもないこと。
 * 空文字は「決めない」の意味で呼び出し側が先に扱うので、ここでは「入力してください」を返す。
 */
export function slugProblem(slug: string): string | null {
  if (slug === "") {
    return "入力してください。";
  }
  if (/[A-Z]/.test(slug)) {
    return "大文字は使えません。小文字にしてください。";
  }
  if (!LOWERCASE_ALPHANUMERIC_OR_HYPHEN.test(slug)) {
    return "小文字の英数字とハイフンだけにしてください。";
  }
  if (slug.startsWith("-") || slug.endsWith("-")) {
    return "先頭と末尾にハイフンは使えません。";
  }
  if (slug.includes("--")) {
    return "ハイフンは続けて使えません。";
  }
  if (slug.length > SLUG_MAX_LENGTH) {
    return `${SLUG_MAX_LENGTH} 文字までにしてください。`;
  }
  if (WINDOWS_RESERVED_NAMES.has(slug)) {
    return `「${slug}」は Windows が予約している名前なので使えません。`;
  }
  return null;
}

/** slug の規則に合うか。 */
export function isValidSlug(slug: string): boolean {
  return slugProblem(slug) === null;
}
