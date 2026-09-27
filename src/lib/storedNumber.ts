// ペイン幅など、利用者ごとの見た目の好みだけを localStorage に覚えておくための小さな補助関数。
// 読めなくても・書けなくても（プライベートウィンドウなど）画面が壊れないようにする。

export function readStoredNumber(key: string, fallback: number): number {
  try {
    const raw = window.localStorage.getItem(key);
    if (raw === null) {
      return fallback;
    }
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? parsed : fallback;
  } catch {
    return fallback;
  }
}

export function writeStoredNumber(key: string, value: number): void {
  try {
    window.localStorage.setItem(key, String(value));
  } catch {
    // 保存できなくても致命的ではないため無視する。
  }
}
