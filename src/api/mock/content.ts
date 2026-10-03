import { toGraphemes } from "../../lib/graphemes";
import type { Manifest } from "../types";
import type { MockChapter, MockCharacter, MockScene } from "./state";

// 偽実装が生成する「それらしい」日本語のテンプレート集。
// 本物の LLM の代わりに、企画の内容を軽く織り込みながら固定の文章を組み立てる。

function firstLine(text: string): string {
  return (text.split("\n")[0] ?? text).trim();
}

export function generateConceptText(manifest: Manifest): string {
  const seed = firstLine(manifest.idea);
  return [
    `『${manifest.title}』は、${seed}という着想から出発する物語である。`,
    "",
    `年齢区分は${manifest.rating === "general" ? "全年齢" : manifest.rating.toUpperCase()}とし、` +
      `目標文字数はおよそ${manifest.target_length}字を見込む。`,
    "",
    "読み終えたときに、登場人物たちの選択が読者自身の記憶と静かに響き合うような読後感を目指す。",
  ].join("\n");
}

export function generateStyleText(manifest: Manifest): string {
  return [
    `${manifest.genre_note ?? manifest.genre} の空気に合わせ、視点人物の内面に寄り添う文体を基本とする。`,
    "地の文は説明を急がず、五感の描写を積み重ねて状況を伝える。会話は簡潔にし、間（沈黙）を活かす。",
    "",
    "文体見本：",
    "窓の外で、雨が屋根を叩く音だけが規則正しく続いていた。",
    "誰も、何も言わなかった。けれど、その沈黙こそが答えだった。",
  ].join("\n");
}

export function generateWorldText(manifest: Manifest): string {
  return [
    `舞台は、${firstLine(manifest.idea)}を成り立たせるための世界とする。`,
    "季節・地理・社会のありようを、物語の緊張を高める方向に少しずつ誇張する。",
    "登場人物たちの価値観と衝突する規範・慣習を、あらかじめ一つ以上仕込んでおく。",
  ].join("\n");
}

interface CastDraft {
  id: string;
  name: string;
  reading: string;
  role: string;
  summary: string;
}

export function generateCastDrafts(): CastDraft[] {
  return [
    {
      id: "protagonist",
      name: "新田 澪",
      reading: "にった みお",
      role: "主人公",
      summary: "物語の視点人物。抱えている迷いが、やがて選択の軸になる。",
    },
    {
      id: "counterpart",
      name: "北条 玲",
      reading: "ほうじょう れい",
      role: "主人公と関わる人物",
      summary: "主人公の価値観を揺さぶる存在。表向きの態度と本音に隔たりがある。",
    },
  ];
}

export function generateCharacterDetail(
  character: Pick<MockCharacter, "name" | "summary" | "role">,
): string {
  return [
    "## 外見",
    `${character.role}らしい佇まい。第一印象と、深く知ったときの印象がわずかに食い違う。`,
    "",
    "## 口調",
    "一人称・語尾に癖を持たせ、他の人物と声だけで聞き分けられるようにする。",
    "",
    "## 背景",
    character.summary,
  ].join("\n");
}

export function generateSynopsisText(manifest: Manifest, characters: MockCharacter[]): string {
  const protagonist = characters[0]?.name ?? "主人公";
  const others = characters
    .slice(1)
    .map((character) => character.name)
    .join("、");
  return [
    firstLine(manifest.idea),
    "",
    `${protagonist}は、${others || "周囲の人々"}と関わりながら、当初は気づかなかった真実に近づいていく。`,
    "選択の先に何を得て、何を手放すのか。物語はその一点に向けて収束していく。",
  ].join("\n");
}

export function generateOutlineChapters(): Array<Pick<MockChapter, "id" | "title" | "storyline">> {
  return [
    {
      id: "01",
      title: "第一章 予兆",
      storyline: "日常に小さな違和感が差し込まれ、主人公が動き出すきっかけが訪れる。",
    },
    {
      id: "02",
      title: "第二章 転機",
      storyline: "主人公の選択が周囲を巻き込み、後戻りのできない状況へと進んでいく。",
    },
  ];
}

export function generateScenePlan(
  chapter: Pick<MockChapter, "title" | "storyline">,
): Array<Omit<MockScene, "draft" | "beats">> {
  const base = [
    {
      title: "予兆",
      summary: `${chapter.storyline}の入り口となる出来事。`,
      place: "日常の場所",
      time: "物語の初め",
    },
    {
      title: "衝突",
      summary: "登場人物どうしの思惑がぶつかる。",
      place: "対立の舞台となる場所",
      time: "事態が動く時",
    },
    { title: "選択", summary: "主人公が次の一歩を決める。", place: "静かな場所", time: "夜" },
  ];
  return base.map((scene, index) => ({
    id: `s${String(index + 1).padStart(2, "0")}`,
    title: scene.title,
    summary: scene.summary,
    pov: "主人公",
    characters: ["主人公"],
    place: scene.place,
    time: scene.time,
    targetChars: 1500,
  }));
}

function draftParagraphs(scene: Pick<MockScene, "place" | "time" | "summary">): string[] {
  return [
    `${scene.time}、${scene.place}。空気が張り詰めているのを、誰もが感じていた。`,
    `${scene.summary}`,
    "誰かが息を吸い込む音が、やけにはっきりと響いた。",
    "それでも、時間は止まってくれない。",
  ];
}

/** ビート単位の生成用に、シーンの要約を短い展開へ分割する（本物にはない簡易版）。 */
export function splitIntoBeats(scene: Pick<MockScene, "summary">): string[] {
  return [`${scene.summary}の発端`, "状況が変化する瞬間", "その場に残る余韻"];
}

export function generateDraftParagraphsForBeat(
  scene: Pick<MockScene, "place" | "time" | "summary">,
  beat: string,
  beatIndex: number,
): string[] {
  if (beatIndex === 0) {
    return [`${scene.time}、${scene.place}。${beat}。`, "誰もがまだ、それに気づいていなかった。"];
  }
  return [`${beat}。`, "空気が、少しだけ変わった。"];
}

export function generateDraftFullText(
  scene: Pick<MockScene, "place" | "time" | "summary">,
): string {
  return draftParagraphs(scene).join("\n");
}

export function generateRevisionText(originalContent: string, instruction: string): string {
  const trimmed = originalContent.trim();
  if (/短く|要約|簡潔/.test(instruction)) {
    const sentences = trimmed.split(/(?<=[。！？])/u).filter((sentence) => sentence.length > 0);
    const shortened = sentences.slice(0, Math.max(1, Math.ceil(sentences.length * 0.6)));
    return shortened.join("");
  }
  const addition = pickRevisionAddition(instruction);
  return `${trimmed}\n${addition}`;
}

function pickRevisionAddition(instruction: string): string {
  if (/怖|恐怖|不穏/.test(instruction)) {
    return "そのとき、廊下の奥で何かが軋む音がした。理由のわからない悪寒が、背筋を伝った。";
  }
  if (/優し|温か|穏やか/.test(instruction)) {
    return "それでも、差し込む光はどこまでも柔らかく、二人の間に流れる時間を静かに包んでいた。";
  }
  if (/テンポ|速く|緊迫/.test(instruction)) {
    return "息をつく間もなく、次の出来事が動き出した。";
  }
  return "その一言が、後になって大きな意味を持つことになるとは、まだ誰も知らなかった。";
}

// ---- 指示から作って足す人物・世界観の資料 ----

interface AddedCharacter {
  name: string;
  reading: string;
  role: string;
  summary: string;
  /** 人物資料の本文（2 回目の生成）。 */
  detail: string;
}

const FIRST_ADDED_CHARACTER = { name: "佐藤 健二", reading: "さとう けんじ" };

const ADDED_CHARACTER_CANDIDATES = [
  FIRST_ADDED_CHARACTER,
  { name: "高橋 美咲", reading: "たかはし みさき" },
  { name: "中村 遼", reading: "なかむら りょう" },
  { name: "小林 沙織", reading: "こばやし さおり" },
];

const SUMMARY_MAX_CHARS = 40;
const TITLE_MAX_CHARS = 16;

/** 指示の 1 行目を、`maxChars` 字までに切り詰める。 */
function abbreviatedInstruction(instruction: string, maxChars: number): string {
  const graphemes = toGraphemes(firstLine(instruction));
  return graphemes.length <= maxChars
    ? graphemes.join("")
    : `${graphemes.slice(0, maxChars).join("")}…`;
}

/** 指示から作った人物。名前は、すでにいる人物と重ならない候補から選ぶ。 */
export function generateAddedCharacter(
  instruction: string,
  existing: Array<Pick<MockCharacter, "name">>,
): AddedCharacter {
  const taken = new Set(existing.map((character) => character.name));
  const candidate =
    ADDED_CHARACTER_CANDIDATES.find(({ name }) => !taken.has(name)) ?? FIRST_ADDED_CHARACTER;
  const role = "登場人物";
  const summary = `指示「${abbreviatedInstruction(instruction, SUMMARY_MAX_CHARS)}」から作った人物。`;
  return {
    ...candidate,
    role,
    summary,
    detail: generateCharacterDetail({ name: candidate.name, role, summary }),
  };
}

/** 指示から作った世界観の資料の題と本文。 */
export function generateAddedWorldDocument(instruction: string): { title: string; body: string } {
  const title = abbreviatedInstruction(instruction, TITLE_MAX_CHARS);
  return {
    title,
    body: [
      "## 概要",
      `指示「${firstLine(instruction)}」にもとづく設定の覚え書き。物語の中で何度も触れられる決まりごとを、ここにまとめる。`,
      "",
      "## 使いどころ",
      "場面の背景として自然に織り込み、説明が長くならないようにする。",
    ].join("\n"),
  };
}
