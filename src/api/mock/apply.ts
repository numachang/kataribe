import { hashText } from "../../lib/hash";
import { BackendError } from "../backend";
import type {
  ChangeSet,
  ExpectFileChange,
  FileChange,
  MoveFileChange,
  TrashedFile,
  TrashFileChange,
  WriteFileChange,
} from "../types";
import { chapterTextFiles } from "./fileChange";
import { chapterIdFromPath, chapterIdFromTextDir } from "./paths";
import { readMockFile } from "./render";
import type { ProjectState } from "./state";
import { findChapter, sortedByNumber } from "./state";
import { trashMockChapterText, trashMockFile, writeMockFile } from "./write";

// 変更案の適用。本物（kataribe-project の `apply_changes`）と同じく、並び順に頼らず
// 「確かめる（Expect）→ ゴミ箱へ移す → 移動 → 書く」の順に、それぞれ競合を確かめる。
// 状態を書き換えずに新しい状態を作って返すので、競合したら呼び出し側の状態は何も変わらない。
//
// 本物との違い: 偽の作品はファイルの木ではなく章ごとの構造なので、本文のフォルダ（`manuscript/<NN>`）は章と一緒に動く
// （章立ての移動が章の id の付け替えを一度にまとめて行い、本文のフォルダの移動は存在の確認だけになる）。
// パスの重なりの検証も簡略で、同じパスの重なりだけを見る（フォルダとその中のパスの重なりは調べない）。

const PROTECTED_FILE = "kataribe.yaml";
const PROTECTED_FOLDER = ".kataribe";

/** 本物のフォルダの代わりに、「中身がある」ことを表す値。どのファイルのハッシュとも一致しない。 */
const FOLDER_WITH_FILES = "folder";

/** 変更案を適用した後の状態を返す。競合したら conflict、形が誤っていれば invalid_input。 */
export function applyMockChangeSet(project: ProjectState, changeSet: ChangeSet): ProjectState {
  checkChangeSetShape(changeSet);
  const expects = changesOfKind<ExpectFileChange>(changeSet, "expect");
  const trashes = changesOfKind<TrashFileChange>(changeSet, "trash");
  const moves = changesOfKind<MoveFileChange>(changeSet, "move");
  const writes = changesOfKind<WriteFileChange>(changeSet, "write");

  for (const file of expects) {
    checkExpectation(project, file);
  }
  for (const file of trashes) {
    checkTrashable(project, file);
  }

  let next = trashes.reduce(trashEntry, project);
  next = moveEntries(next, moves);
  for (const file of writes) {
    assertUnchanged(next, file.path, file.base_hash);
    next = writeMockFile(next, file.path, file.content);
  }
  return next;
}

/** `path` の今のハッシュが `expectedHash`（null なら「無いこと」）と一致しなければ conflict。 */
export function assertUnchanged(
  project: ProjectState,
  path: string,
  expectedHash: string | null,
): void {
  const current = readMockFile(project, path);
  const currentHash = current !== null ? hashText(current) : null;
  if (expectedHash !== currentHash) {
    throw new BackendError(
      "conflict",
      "この文書は外部で変更されています。再読み込みするか、上書きしてください。",
    );
  }
}

function changesOfKind<Change extends FileChange>(
  changeSet: ChangeSet,
  kind: Change["kind"],
): Change[] {
  return changeSet.files.filter((file): file is Change => file.kind === kind);
}

// ---- 競合の確認 ----

/** ファイルのハッシュ。章の本文のフォルダは、中にファイルがあれば `FOLDER_WITH_FILES`。何も無ければ null。 */
function currentEntryHash(project: ProjectState, path: string): string | null {
  const chapterId = chapterIdFromTextDir(path);
  if (chapterId !== null) {
    return chapterTextFiles(project, chapterId).length > 0 ? FOLDER_WITH_FILES : null;
  }
  const content = readMockFile(project, path);
  return content === null ? null : hashText(content);
}

function checkExpectation(project: ProjectState, file: ExpectFileChange): void {
  if (currentEntryHash(project, file.path) !== file.base_hash) {
    throw new BackendError(
      "conflict",
      `「${file.path}」が確かめたあとに変更されたため、適用しませんでした。`,
    );
  }
}

function checkTrashable(project: ProjectState, file: TrashFileChange): void {
  const chapterId = chapterIdFromTextDir(file.path);
  const isUnchanged =
    chapterId === null
      ? readMockFile(project, file.path) !== null &&
        currentEntryHash(project, file.path) === (file.files[0]?.base_hash ?? null)
      : sameFiles(chapterTextFiles(project, chapterId), file.files);
  if (!isUnchanged) {
    throw new BackendError(
      "conflict",
      `「${file.path}」が確かめたあとに変更されたため、ゴミ箱へ移しませんでした。`,
    );
  }
}

/** フォルダの中のファイルの一覧が、パスもハッシュも完全に一致するか（増えていても変わっていても不一致）。 */
function sameFiles(current: TrashedFile[], planned: TrashedFile[]): boolean {
  const plannedHashes = new Map(planned.map((file) => [file.path, file.base_hash]));
  return (
    current.length === planned.length &&
    current.every((file) => plannedHashes.get(file.path) === file.base_hash)
  );
}

// ---- 適用 ----

function trashEntry(project: ProjectState, file: TrashFileChange): ProjectState {
  const chapterId = chapterIdFromTextDir(file.path);
  return chapterId === null
    ? trashMockFile(project, file.path)
    : trashMockChapterText(project, chapterId);
}

/**
 * 移動をまとめて適用する。章の id の付け替えは、移動の順や入れ替えに依らないよう一度に行う。
 * 本文のフォルダの移動は、章立ての移動と一緒に済むので、移動元があることの確認だけをする。
 */
function moveEntries(project: ProjectState, moves: MoveFileChange[]): ProjectState {
  const renames = new Map<string, string>();
  for (const move of moves) {
    const fromChapter = chapterIdFromPath(move.from);
    const toChapter = chapterIdFromPath(move.to);
    if (fromChapter !== null && toChapter !== null) {
      requireMovable(findChapter(project, fromChapter) !== null, move);
      renames.set(fromChapter, toChapter);
      continue;
    }
    const fromText = chapterIdFromTextDir(move.from);
    const toText = chapterIdFromTextDir(move.to);
    if (fromText !== null && toText !== null) {
      requireMovable(chapterTextFiles(project, fromText).length > 0, move);
      continue;
    }
    throw new BackendError(
      "invalid_input",
      `「${move.from}」は偽バックエンドでは移動できません（動かせるのは、章立てと章の本文のフォルダです）。`,
    );
  }
  return renames.size === 0 ? project : renumberChapters(project, renames);
}

function requireMovable(exists: boolean, move: MoveFileChange): void {
  if (!exists) {
    throw new BackendError(
      "conflict",
      `「${move.from}」が確かめたあとに無くなったため、移動しませんでした。`,
    );
  }
}

function renumberChapters(project: ProjectState, renames: Map<string, string>): ProjectState {
  const renumbered = (project.chapters ?? []).map((chapter) => ({
    ...chapter,
    id: renames.get(chapter.id) ?? chapter.id,
  }));
  const ids = renumbered.map((chapter) => chapter.id);
  if (new Set(ids).size !== ids.length) {
    throw new BackendError(
      "conflict",
      "移動先に同じ番号の章があるため、番号を振り直せませんでした。",
    );
  }
  return { ...project, chapters: sortedByNumber(renumbered) };
}

// ---- 形の検証 ----

/** 変更の対象のパスと、その変更の種類。移動は元と先を別々に数える。 */
function touchedPaths(file: FileChange): Array<{ role: string; path: string }> {
  switch (file.kind) {
    case "move":
      return [
        { role: "move_from", path: file.from },
        { role: "move_to", path: file.to },
      ];
    default:
      return [{ role: file.kind, path: file.path }];
  }
}

/** ゴミ箱へ移せない・動かせない場所か。Windows は大文字小文字を区別しないので、`Kataribe.yaml` や `.Kataribe/…` も同じ扱い。 */
function isProtected(path: string): boolean {
  const lowerCased = path.toLowerCase();
  return (
    lowerCased === PROTECTED_FILE ||
    lowerCased === PROTECTED_FOLDER ||
    lowerCased.startsWith(`${PROTECTED_FOLDER}/`)
  );
}

/**
 * 本物と同じく、画面から戻ってくる変更案の形を確かめる。
 * 同じ種類の変更が同じパスに重ならないこと（書き込みが移動元・移動先と重なるのは、空いた場所への書き込みなので許す）、
 * ゴミ箱へ移せない・動かせないパスを含まないこと、ゴミ箱へ移す変更の `files` が対象に合っていること。
 */
function checkChangeSetShape(changeSet: ChangeSet): void {
  const seen = new Set<string>();
  for (const file of changeSet.files) {
    for (const { role, path } of touchedPaths(file)) {
      const key = `${role}:${path.toLowerCase()}`;
      if (seen.has(key)) {
        throw new BackendError("invalid_input", `「${path}」への変更が重なっています。`);
      }
      seen.add(key);
    }
    if (file.kind === "trash") {
      checkTrashShape(file);
    }
    if (file.kind === "move") {
      checkMoveShape(file);
    }
  }
  checkTrashedPathsAreNotWritten(changeSet);
}

function checkTrashShape(file: TrashFileChange): void {
  if (isProtected(file.path)) {
    throw new BackendError("invalid_input", `「${file.path}」はゴミ箱へ移せません。`);
  }
  const isFolder = chapterIdFromTextDir(file.path) !== null;
  const isListCorrect = isFolder
    ? file.files.every((moved) => moved.path.startsWith(`${file.path}/`))
    : file.files.length === 1 && file.files[0]?.path === file.path;
  if (!isListCorrect) {
    throw new BackendError(
      "invalid_input",
      `「${file.path}」をゴミ箱へ移す変更の、移すファイルの一覧が合っていません。`,
    );
  }
}

function checkMoveShape(file: MoveFileChange): void {
  for (const path of [file.from, file.to]) {
    if (isProtected(path)) {
      throw new BackendError("invalid_input", `「${path}」は移動できません。`);
    }
  }
  const [from, to] = [file.from.toLowerCase(), file.to.toLowerCase()];
  if (from === to || to.startsWith(`${from}/`) || from.startsWith(`${to}/`)) {
    throw new BackendError(
      "invalid_input",
      `「${file.from}」を「${file.to}」へは移せません（同じ場所、または自分の中になります）。`,
    );
  }
}

/** ゴミ箱へ移すパスに、同じ変更案で書くことはできない（移動で空いた場所と違い、消えるものの上に書くことになる）。 */
function checkTrashedPathsAreNotWritten(changeSet: ChangeSet): void {
  const trashed = changesOfKind<TrashFileChange>(changeSet, "trash").map((file) =>
    file.path.toLowerCase(),
  );
  for (const file of changesOfKind<WriteFileChange>(changeSet, "write")) {
    const path = file.path.toLowerCase();
    if (trashed.some((folder) => path === folder || path.startsWith(`${folder}/`))) {
      throw new BackendError("invalid_input", `「${file.path}」への変更が重なっています。`);
    }
  }
}
