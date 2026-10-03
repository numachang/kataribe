import type { CharacterMeta, EditableDocument, ScenePlan } from "../../../../api/types";
import { bodyHeading } from "../../document-form/DocumentForm";
import {
  CHAPTER_FIELD_LABELS,
  CHARACTER_FIELD_LABELS,
  SCENE_FIELD_LABELS,
} from "../../document-form/fieldLabels";
import { movedSceneIds } from "./sceneOrder";

// 変更案の人物資料・章立てを、読むだけの項目の一覧にするための表示用のデータ。
// 変更前と変更後の両方があるときは、変わった項目・追加または削除または並び替えられたシーン・変わった本文に印を付ける。
// 項目名はエディタのフォーム（document-form/）と同じ。アプリが知らない項目（extra）は「その他の項目」として別に並べる。

/** 項目に分けた文書（人物資料・章立て）。 */
export type StructuredDocument = Extract<EditableDocument, { kind: "character" | "chapter" }>;

/** 今見せているのが変更前か変更後か。追加・削除の向きが変わる。 */
export type ViewedSide = "before" | "after";

/** シーンにつく印。 */
export type SceneMark = "changed" | "added" | "removed" | "moved";

/** 項目の値。一覧（ビート）は番号付きで見せ、それ以外は文字列で見せる。空文字は「（なし）」と見せる。 */
type FieldValue = string | string[];

/** 比べる前の 1 項目。`signature` が同じなら同じ値（文字列の "3" と数値の 3 のように、見た目が同じでも型が違えば違う）。 */
interface Field {
  label: string;
  value: FieldValue;
  signature: string;
}

/** 決まった項目と、アプリが知らない項目。 */
interface FieldGroups {
  fields: Field[];
  extraFields: Field[];
}

/** 表示する 1 項目。`changed` は、比べた相手の文書と値が違うとき（相手に無いときを含む）。 */
export interface FieldView {
  label: string;
  value: FieldValue;
  changed: boolean;
}

interface FieldViewGroups {
  fields: FieldView[];
  extraFields: FieldView[];
}

export interface SceneView extends FieldViewGroups {
  id: string;
  title: string;
  marks: SceneMark[];
}

export interface DocumentView extends FieldViewGroups {
  scenes: SceneView[];
  bodyHeading: string;
  body: string;
  /** 比べた相手の本文と違うか。 */
  bodyChanged: boolean;
  /** 相手の文書と比べて印を付けたか。相手が無い・分けられない・種類が違うときは false。 */
  compared: boolean;
}

// 画面が項目として扱うキー。これ以外が extra。`id` と `scenes` はフォームの項目ではないが、既知のキー。
const CHARACTER_KEYS: readonly string[] = Object.keys(CHARACTER_FIELD_LABELS);
const CHAPTER_KEYS: readonly string[] = [...Object.keys(CHAPTER_FIELD_LABELS), "scenes"];
const SCENE_KEYS: readonly string[] = ["id", ...Object.keys(SCENE_FIELD_LABELS)];

/** 相手に無い項目を、比べるために置く値。`JSON.stringify` はこの値にならない。 */
const ABSENT_SIGNATURE = "";

export function isStructuredDocument(document: EditableDocument): document is StructuredDocument {
  return document.kind !== "text";
}

function field(label: string, value: FieldValue): Field {
  return { label, value, signature: JSON.stringify(value) };
}

function optionalText(value: string | number | null | undefined): string {
  return value === null || value === undefined ? "" : String(value);
}

function extraValueText(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}

function extraFieldsOf(meta: object, knownKeys: readonly string[]): Field[] {
  return Object.entries(meta)
    .filter(([key]) => !knownKeys.includes(key))
    .map(([key, value]) => ({
      label: key,
      value: extraValueText(value),
      signature: JSON.stringify(value),
    }));
}

function characterFields(meta: CharacterMeta): FieldGroups {
  return {
    fields: [
      field(CHARACTER_FIELD_LABELS.name, meta.name),
      field(CHARACTER_FIELD_LABELS.reading, optionalText(meta.reading)),
      field(CHARACTER_FIELD_LABELS.role, meta.role),
      field(CHARACTER_FIELD_LABELS.summary, meta.summary),
      field(CHARACTER_FIELD_LABELS.order, optionalText(meta.order)),
    ],
    extraFields: extraFieldsOf(meta, CHARACTER_KEYS),
  };
}

function documentFields(document: StructuredDocument): FieldGroups {
  return document.kind === "character"
    ? characterFields(document.meta)
    : {
        fields: [field(CHAPTER_FIELD_LABELS.title, document.meta.title)],
        extraFields: extraFieldsOf(document.meta, CHAPTER_KEYS),
      };
}

function documentScenes(document: StructuredDocument): ScenePlan[] {
  return document.kind === "chapter" ? (document.meta.scenes ?? []) : [];
}

function sceneFields(scene: ScenePlan): FieldGroups {
  const fields = [
    field(SCENE_FIELD_LABELS.title, scene.title),
    field(SCENE_FIELD_LABELS.summary, scene.summary),
    field(SCENE_FIELD_LABELS.pov, optionalText(scene.pov)),
    field(SCENE_FIELD_LABELS.characters, (scene.characters ?? []).join("、")),
    field(SCENE_FIELD_LABELS.place, optionalText(scene.place)),
    field(SCENE_FIELD_LABELS.time, optionalText(scene.time)),
    field(SCENE_FIELD_LABELS.target_chars, optionalText(scene.target_chars)),
  ];
  const beats = scene.beats ?? [];
  // ビートはある（生成した）ときだけ項目にする。フォームと同じ。比べるときは、無い側を「（なし）」とみなす。
  return {
    fields: beats.length > 0 ? [...fields, field(SCENE_FIELD_LABELS.beats, beats)] : fields,
    extraFields: extraFieldsOf(scene, SCENE_KEYS),
  };
}

/**
 * 項目に印を付ける。相手が無ければ（比べない）付けない。
 * 比べる項目は両側の和集合で、相手にだけある項目は、こちらでは空の項目として出し、変わったとみなす。
 */
function markFields(own: Field[], counterpart: Field[] | null): FieldView[] {
  if (counterpart === null) {
    return own.map(({ label, value }) => ({ label, value, changed: false }));
  }
  const ownLabels = new Set(own.map((candidate) => candidate.label));
  const absent = counterpart
    .filter((candidate) => !ownLabels.has(candidate.label))
    .map(({ label }): Field => ({ label, value: "", signature: ABSENT_SIGNATURE }));
  return [...own, ...absent].map(({ label, value, signature }) => {
    const other = counterpart.find((candidate) => candidate.label === label);
    return { label, value, changed: other?.signature !== signature };
  });
}

function markFieldGroups(own: FieldGroups, counterpart: FieldGroups | null): FieldViewGroups {
  return {
    fields: markFields(own.fields, counterpart?.fields ?? null),
    extraFields: markFields(own.extraFields, counterpart?.extraFields ?? null),
  };
}

function hasChangedField(groups: FieldViewGroups): boolean {
  return [...groups.fields, ...groups.extraFields].some((candidate) => candidate.changed);
}

function toSceneView(scene: ScenePlan, marks: SceneMark[], groups: FieldViewGroups): SceneView {
  return { id: scene.id, title: scene.title, marks, ...groups };
}

/** 変更後を見せるときは、変更後に無いシーンを「削除」として末尾に足す。 */
function buildSceneViews(
  scenes: ScenePlan[],
  counterpartScenes: ScenePlan[] | null,
  side: ViewedSide,
): SceneView[] {
  if (counterpartScenes === null) {
    return scenes.map((scene) => toSceneView(scene, [], markFieldGroups(sceneFields(scene), null)));
  }
  const movedIds = movedSceneIds(scenes, counterpartScenes);
  const views = scenes.map((scene) => {
    const counterpart = counterpartScenes.find((candidate) => candidate.id === scene.id);
    if (counterpart === undefined) {
      const mark = side === "after" ? "added" : "removed";
      return toSceneView(scene, [mark], markFieldGroups(sceneFields(scene), null));
    }
    const groups = markFieldGroups(sceneFields(scene), sceneFields(counterpart));
    const marks: SceneMark[] = [];
    if (hasChangedField(groups)) {
      marks.push("changed");
    }
    if (movedIds.has(scene.id)) {
      marks.push("moved");
    }
    return toSceneView(scene, marks, groups);
  });
  if (side === "before") {
    return views;
  }
  const removed = counterpartScenes
    .filter((counterpart) => !scenes.some((scene) => scene.id === counterpart.id))
    .map((scene) => toSceneView(scene, ["removed"], markFieldGroups(sceneFields(scene), null)));
  return [...views, ...removed];
}

/**
 * 表示用のデータを作る。`counterpart` は、変更前を見せるなら変更後、変更後を見せるなら変更前の文書。
 * 無いとき、または同じ種類の文書でないとき（比べられない）は、印を付けない。
 */
export function buildDocumentView(
  document: StructuredDocument,
  counterpart: EditableDocument | null,
  side: ViewedSide,
): DocumentView {
  const comparable =
    counterpart !== null && counterpart.kind === document.kind ? counterpart : null;
  return {
    ...markFieldGroups(documentFields(document), comparable && documentFields(comparable)),
    scenes: buildSceneViews(
      documentScenes(document),
      comparable && documentScenes(comparable),
      side,
    ),
    bodyHeading: bodyHeading(document) ?? "本文",
    body: document.body,
    bodyChanged: comparable !== null && comparable.body !== document.body,
    compared: comparable !== null,
  };
}
