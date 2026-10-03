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
import { chapterNumber, described, invalidInput, requireChapter } from "./structureSupport";

// 章の追加と削除の変更案を作る。kataribe-engine の `structure/chapters.rs` と同じ形の変更案
// （後ろの章の章立てと本文のフォルダを Move、新しい章を Write、消す章を Trash）にする。

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

/**
 * `chapters` の章立てと本文のフォルダを、番号を `delta` ずらした場所へ改名する変更。
 * 本文のフォルダがまだ無い章には、適用のときにまだ無いことの確認を付ける。移す先も、同じ変更案の
 * 移動・ゴミ箱・確認で扱っていなければ、外で本文のフォルダができていないことを確かめる
 * （番号が抜けていて行き先が空いているときに、そこへ取り残された本文を作らないため）。
 * `alreadyPlanned` は、同じ変更案のほかの部分（章を消すときの、消す章の分）。
 */
function renumber(
  state: ProjectState,
  chapters: MockChapter[],
  delta: number,
  alreadyPlanned: FileChange[] = [],
): { files: FileChange[]; renumbered: RenumberedChapter[] } {
  const files: FileChange[] = [];
  const renumbered: RenumberedChapter[] = [];
  const destinationsWithoutText: string[] = [];
  for (const chapter of chapters) {
    const to = shiftedId(chapter.id, delta);
    files.push(moveChange(chapterPath(chapter.id), chapterPath(to)));
    if (chapterTextFiles(state, chapter.id).length > 0) {
      files.push(moveChange(chapterTextDir(chapter.id), chapterTextDir(to)));
    } else {
      files.push(expectAbsentChange(chapterTextDir(chapter.id)));
      destinationsWithoutText.push(chapterTextDir(to));
    }
    renumbered.push({ from: chapter.id, to, title: chapter.title.trim() || null });
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
  const { files, renumbered } = renumber(state, shifted, 1);
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
  const { files: moves, renumbered } = renumber(state, shifted, -1, files);
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
