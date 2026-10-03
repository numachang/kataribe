import type { FileChange, RenumberedChapter, StructurePlan } from "../types";
import {
  chapterTextFiles,
  expectAbsentChange,
  moveChange,
  trashChange,
  trashChapterTextChange,
} from "./fileChange";
import { chapterPath, chapterTextDir, shiftedChapterId } from "./paths";
import { renderChapterFile } from "./render";
import type { MockChapter, ProjectState } from "./state";
import { sortedByNumber } from "./state";
import {
  chapterNumber,
  described,
  ensureNewPosition,
  invalidInput,
  movedItem,
  requireChapter,
} from "./structureSupport";

// 章の追加・削除・並べ替えの変更案を作る。kataribe-engine の `structure/chapters.rs` と同じ形の変更案
// （動く範囲の章の章立てと本文のフォルダを Move、新しい章を Write、消す章を Trash）にする。

const TOO_MANY_CHAPTERS = "章は 999 までです。これより後ろには足せません。";

/** 利用者に見せる章の呼び方。 */
function chapterLabel(chapterId: string, title: string | null): string {
  return title === null
    ? `第${chapterNumber(chapterId)}章`
    : `第${chapterNumber(chapterId)}章「${title}」`;
}

function shiftedId(chapterId: string, delta: number): string {
  const shifted = shiftedChapterId(chapterId, delta);
  if (shifted === null) {
    throw invalidInput(TOO_MANY_CHAPTERS);
  }
  return shifted;
}

/** この変更案が、元の場所を空ける・元の場所を確かめる・ゴミ箱へ移すパス。ここにある行き先は、改めて確かめなくてよい。 */
function accountedPaths(files: FileChange[]): Set<string> {
  const paths = new Set<string>();
  for (const file of files) {
    if (file.kind === "move") {
      paths.add(file.from);
    } else if (file.kind !== "write") {
      paths.add(file.path);
    }
  }
  return paths;
}

/** 章立てと本文のフォルダを移す、章の番号の付け替え 1 件（今の番号 → 移す先の番号）。 */
interface ChapterAssignment {
  from: string;
  to: string;
}

/** `chapters` の番号を `delta` ずらす付け替え。 */
function shiftAssignments(chapters: MockChapter[], delta: number): ChapterAssignment[] {
  return chapters.map((chapter) => ({ from: chapter.id, to: shiftedId(chapter.id, delta) }));
}

/**
 * 付け替えのとおりに、章立てと本文のフォルダを改名する変更。
 * 本文のフォルダがまだ無い章には、適用のときにまだ無いことの確認を付ける。移す先も、同じ変更案の
 * 移動・ゴミ箱・確認で扱っていなければ、外で本文のフォルダができていないことを確かめる
 * （番号が抜けていて行き先が空いているときに、そこへ取り残された本文を作らないため）。
 * `alreadyPlanned` は、同じ変更案のほかの部分（章を消すときの、消す章の分）。
 */
function relocate(
  state: ProjectState,
  assignments: ChapterAssignment[],
  alreadyPlanned: FileChange[] = [],
): { files: FileChange[]; renumbered: RenumberedChapter[] } {
  const files: FileChange[] = [];
  const renumbered: RenumberedChapter[] = [];
  const destinationsWithoutText: string[] = [];
  for (const { from, to } of assignments) {
    files.push(moveChange(chapterPath(from), chapterPath(to)));
    if (chapterTextFiles(state, from).length > 0) {
      files.push(moveChange(chapterTextDir(from), chapterTextDir(to)));
    } else {
      files.push(expectAbsentChange(chapterTextDir(from)));
      destinationsWithoutText.push(chapterTextDir(to));
    }
    renumbered.push({ from, to, title: requireChapter(state, from).title.trim() || null });
  }
  const accounted = accountedPaths([...alreadyPlanned, ...files]);
  for (const destination of destinationsWithoutText) {
    if (!accounted.has(destination)) {
      files.push(expectAbsentChange(destination));
      accounted.add(destination);
    }
  }
  return { files, renumbered };
}

/** 末尾に足す章の id。最大の番号の次で、章が無ければ 01。 */
function nextChapterId(chapters: MockChapter[]): string {
  const last = chapters.at(-1);
  return last === undefined ? "01" : shiftedId(last.id, 1);
}

/** 新しい章の章立て。章題とストーリーラインだけで、シーン構成は無い。 */
function renderNewChapter(id: string, title: string, storyline: string): string {
  const trimmed = storyline.trim();
  return renderChapterFile({
    id,
    title,
    storyline: trimmed === "" ? "" : `${trimmed}\n`,
    scenes: null,
  });
}

export function planAddChapter(
  state: ProjectState,
  before: string | null,
  rawTitle: string,
  storyline: string,
): StructurePlan {
  const title = rawTitle.trim();
  if (title === "") {
    throw invalidInput("章題を入力してください。");
  }
  if (/[\r\n]/.test(title)) {
    throw invalidInput("章題は 1 行で入力してください。");
  }
  const chapters = sortedByNumber(state.chapters ?? []);
  if (before !== null) {
    requireChapter(state, before);
  }
  const newId = before ?? nextChapterId(chapters);
  const shifted =
    before === null
      ? []
      : chapters.filter((chapter) => chapterNumber(chapter.id) >= chapterNumber(before));
  const { files, renumbered } = relocate(state, shiftAssignments(shifted, 1));
  if (before === null) {
    // 末尾に足す章の本文のフォルダは、まだ無いはず。外で作られたら適用を止める
    files.push(expectAbsentChange(chapterTextDir(newId)));
  }
  const path = chapterPath(newId);
  // 書き先は、改名のあとの「無いこと」が条件（空いた場所に新しく書く）
  files.push({
    kind: "write",
    path,
    content: renderNewChapter(newId, title, storyline),
    previous: null,
    base_hash: null,
  });
  return {
    ...described(state, `${chapterLabel(newId, title)}を追加し`, files),
    created: path,
    references: [],
    renumbered,
    notices: [],
  };
}

export function planRemoveChapter(state: ProjectState, chapterId: string): StructurePlan {
  const chapter = requireChapter(state, chapterId);
  const shifted = sortedByNumber(state.chapters ?? []).filter(
    (candidate) => chapterNumber(candidate.id) > chapterNumber(chapterId),
  );
  const files: FileChange[] = [
    trashChange(state, chapterPath(chapterId)),
    trashChapterTextChange(state, chapterId) ?? expectAbsentChange(chapterTextDir(chapterId)),
  ];
  const { files: moves, renumbered } = relocate(state, shiftAssignments(shifted, -1), files);
  return {
    ...described(state, `${chapterLabel(chapterId, chapter.title.trim() || null)}をゴミ箱へ移し`, [
      ...files,
      ...moves,
    ]),
    created: null,
    references: [],
    renumbered,
    notices: [],
  };
}

/**
 * 章を並べ替える変更案。動く範囲の章が持っている番号を、並べ替えたあとの並びへそのまま割り当てる
 * （番号の集合は変わらないので、抜けた番号は抜けたまま。入れ替えのような循環は、適用が一度にまとめて付け替える）。
 */
export function planMoveChapter(
  state: ProjectState,
  chapterId: string,
  position: number,
): StructurePlan {
  const chapter = requireChapter(state, chapterId);
  const existing = sortedByNumber(state.chapters ?? []).map((candidate) => candidate.id);
  const subject = chapterLabel(chapterId, chapter.title.trim() || null);
  ensureNewPosition(subject, "章", existing.indexOf(chapterId), position, existing.length);

  // 並べ替えたあとの並びの i 番目の章に、今の並びの i 番目の番号を割り当てる。動かない章は同じ番号に割り当たるので除く
  const assignments = movedItem(existing, existing.indexOf(chapterId), position)
    .map((from, index) => ({ from, to: existing[index] ?? from }))
    .filter(({ from, to }) => from !== to);
  const destination = assignments.find(({ from }) => from === chapterId)?.to ?? chapterId;
  const { files, renumbered } = relocate(state, assignments);
  return {
    ...described(state, `${subject}を第${chapterNumber(destination)}章へ移し`, files),
    created: null,
    references: [],
    renumbered: renumbered.toSorted(
      (left, right) => chapterNumber(left.from) - chapterNumber(right.from),
    ),
    notices: [],
  };
}
