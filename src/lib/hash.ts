// 偽バックエンド専用の内容ハッシュ。
// 本物のハッシュ（Rust 側の ContentHash）とアルゴリズムを合わせる必要はない。
// 「同じ内容なら同じ値」「違う内容ならほぼ確実に違う値」であれば競合検出の
// シミュレーションとして十分なため、依存を増やさず FNV-1a で済ませる。

const FNV_OFFSET_BASIS = 0x811c9dc5;
const FNV_PRIME = 0x01000193;

/** 文字列から短い 16 進ハッシュを作る。 */
export function hashText(text: string): string {
  let hash = FNV_OFFSET_BASIS;
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, FNV_PRIME);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}
