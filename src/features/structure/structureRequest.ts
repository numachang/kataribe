import type { StructureEdit } from "../../api/types";

/** 消す操作（確認のダイアログを経る）。 */
export type RemoveEdit = Extract<
  StructureEdit,
  { kind: "remove_character" | "remove_world_document" | "remove_chapter" | "remove_scene" }
>;

/** 目次から求められた、構成の操作のダイアログ。 */
export type StructureRequest =
  | { kind: "add_character" }
  | { kind: "add_world_document" }
  /** `before` の章の前に足す。null なら末尾。 */
  | { kind: "add_chapter"; before: string | null }
  /** `before` のシーンの前に足す。null なら章の末尾。 */
  | { kind: "add_scene"; chapter: string; before: string | null }
  | { kind: "remove"; edit: RemoveEdit };
