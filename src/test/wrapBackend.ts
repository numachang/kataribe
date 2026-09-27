import type { Backend } from "../api/backend";

/**
 * Backend の実装をラップし、一部のメソッドだけ差し替えたテスト用の Backend を作る。
 *
 * 偽バックエンド（クラスベースの実装）はメソッドの中で `this` を通じて内部状態を読み書きする。
 * `Object.create(inner)` でプロトタイプ委譲するだけだと、上書きしていないメソッドを
 * ラップしたオブジェクト経由で呼び出したときに `this` がラップ側にずれてしまい、
 * `inner` 側の状態（開いている作品など）が更新されない。
 * ここでは全メソッドをあらかじめ `inner` に束縛してから、指定したものだけ差し替える。
 */
export function wrapBackend(inner: Backend, overrides: Partial<Backend>): Backend {
  const bound: Record<string, unknown> = {};
  let prototype: object | null = Object.getPrototypeOf(inner);
  while (prototype && prototype !== Object.prototype) {
    for (const key of Object.getOwnPropertyNames(prototype)) {
      if (key === "constructor" || key in bound) {
        continue;
      }
      const value = (inner as unknown as Record<string, unknown>)[key];
      if (typeof value === "function") {
        bound[key] = value.bind(inner);
      }
    }
    prototype = Object.getPrototypeOf(prototype);
  }
  return { ...bound, ...overrides } as Backend;
}
