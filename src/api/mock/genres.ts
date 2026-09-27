import type { GenrePreset } from "../types";

// Rust 側の crates/kataribe-engine/presets/genres.yaml に相当する、
// 画面の開発・テストに使うためのジャンル一覧。
export const GENRE_PRESETS: GenrePreset[] = [
  {
    id: "junbungaku",
    label: "純文学",
    description: "文体そのものを味わう、内面描写に重きを置いた小説。",
  },
  { id: "mystery", label: "ミステリ", description: "謎の提示と論理的な解決を軸にした小説。" },
  { id: "scifi", label: "SF", description: "科学的・思弁的な仮定から物語を組み立てる小説。" },
  {
    id: "fantasy",
    label: "ファンタジー",
    description: "現実にない法則や存在がある世界を舞台にした小説。",
  },
  {
    id: "light_novel",
    label: "ライトノベル",
    description: "会話とテンポを重視した、軽やかな娯楽小説。",
  },
  { id: "romance", label: "恋愛", description: "恋愛関係の展開を主軸にした小説。" },
  { id: "bl", label: "BL", description: "男性同士の恋愛を主軸にした小説。" },
  {
    id: "erotic",
    label: "官能",
    description: "性的な描写を主軸にした小説。年齢区分の設定に注意する。",
  },
  { id: "horror", label: "ホラー", description: "恐怖や不安を喚起することを目的にした小説。" },
];
