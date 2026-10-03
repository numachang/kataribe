import type { ChapterMeta, CharacterMeta, ScenePlan } from "../../../api/types";

// 人物資料・章立ての項目名。エディタのフォーム（このフォルダ）と、変更案の表示（ai/change-review/）が
// 同じ名前で見せるために、ここに一つだけ置く。キーは front matter の項目名。

export const CHARACTER_FIELD_LABELS: Record<keyof CharacterMeta, string> = {
  name: "名前",
  reading: "読み",
  role: "役割",
  summary: "概要",
  order: "順番",
};

export const CHAPTER_FIELD_LABELS: Record<Exclude<keyof ChapterMeta, "scenes">, string> = {
  title: "章題",
};

export const SCENE_FIELD_LABELS: Record<Exclude<keyof ScenePlan, "id">, string> = {
  title: "タイトル",
  summary: "概要",
  pov: "視点",
  characters: "登場人物",
  place: "場所",
  time: "時間",
  target_chars: "目標文字数",
  beats: "ビート",
};

/** 章立ての、シーンの一覧の見出し。 */
export const SCENES_HEADING = "シーン";
