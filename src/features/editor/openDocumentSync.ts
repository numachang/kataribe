import type { Backend } from "../../api/backend";
import { toErrorMessage } from "../../lib/errorMessage";
import { useEditorStore } from "../../store/editorStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { documentSaveController } from "./documentSaveController";

/** `path` を中央のエディタで開いていれば、未保存の編集を保存しておく。 */
export async function flushIfOpen(backend: Backend, path: string): Promise<void> {
  if (useWorkspaceStore.getState().currentPath === path) {
    await documentSaveController.flush(backend);
  }
}

interface WriteBesideEditorOptions<T> {
  /** 書き換えるファイルに、パス `path` の文書が含まれるか（ゴミ箱へ移すもの・改名するものも含む）。 */
  touches: (path: string) => boolean;
  /**
   * 書き換えのあとで内容が変わるか。内容が変わる文書だけを読み直す。省くと `touches` と同じ
   * （改名やゴミ箱へ移すだけの文書は、内容が変わらないので読み直さない）。
   */
  rewrites?: (path: string) => boolean;
  /**
   * 書き換えで、パス `path` の文書がどこへ行くか。ゴミ箱へ移るなら null（読み直さずに文書を閉じて知らせる。
   * 移ったあとのファイルは読めないため）、改名されるなら新しいパス（読み直さずに、パスだけを付け替えて続ける）、
   * 動かないなら undefined。
   */
  relocatedPath?: (path: string) => string | null | undefined;
  /** 実際の書き換え（変更案の適用・作品の設定の保存など）。 */
  write: () => Promise<T>;
  /** 開いている文書に保存できない編集が残っていて、書き換えを止めるときのエラー文。 */
  unsavedWorkMessage: string;
}

/**
 * 作品フォルダのファイルを、中央のエディタとは別の経路で書き換える（変更案の適用、設定ダイアログからの
 * kataribe.yaml の保存など）。
 *
 * 1. 先に開いている文書の保存を済ませる。そうすればディスク側の競合検出が働き、未保存の編集を黙って上書きしない。
 * 2. 書き換える文書を開いていて、保存できない編集が残っていれば、書き換えずに止める
 *    （書き換えた後の読み直しで、その編集が消えるため）。
 * 3. 書き換えた後、その文書を開いていれば読み直す。こうしないと、エディタに古い内容が残り、次の自動保存が
 *    競合になって、書き換えた内容を利用者が上書きで消しかねない。書き換えの間にエディタへ入力されていたら
 *    読み直さない（その編集の基準は古いハッシュのままなので、次の保存で競合として知らせる）。
 *
 * 開いている文書がゴミ箱へ移ったときは、読み直さずに文書を閉じて、そのことを知らせる。
 * 開いている文書が改名されたときは（章の番号の振り直し）、内容は変わらないので読み直さず、エディタと目次の選択を
 * 新しいパスへ付け替える。これで、そのまま続けて編集でき、次の自動保存は新しいパスへ書く（古いパスへ書くと、
 * 移動で空いた場所に別の章の文書があるときに、それを壊してしまう）。
 *
 * 書き換えている間（読み直し・付け替えが終わるまで）は、エディタの保存の開始を止める（`holdSaves`）。
 * 保存先のパスは保存を始めるときに決まるので、その間の入力や Ctrl+S が改名の前のパスへの保存になるのを防ぐ。
 * 止めた保存は、終わったあとの文書とパスで保存し直す。
 *
 * 読み直しに失敗しても、書き換えそのものは済んでいるので、失敗を知らせたうえで `write` の結果を返す。
 */
export async function writeBesideEditor<T>(
  backend: Backend,
  {
    touches,
    rewrites = touches,
    relocatedPath,
    write,
    unsavedWorkMessage,
  }: WriteBesideEditorOptions<T>,
): Promise<T> {
  await documentSaveController.flush(backend);
  const openPath = useWorkspaceStore.getState().currentPath;
  if (openPath !== null && touches(openPath) && documentSaveController.hasUnsavedWork()) {
    throw new Error(unsavedWorkMessage);
  }
  // 書き換えの最中に始まる保存は、改名の前のパスを対象にしてしまう（書き込みはこの書き換えの終わりまで待たされ、
  // 古いパスに別の章の文書があれば競合になって、「上書き」で壊す）。書き換えとパスの付け替えが終わるまで止め、
  // 終わったあとの文書とパスで保存し直す。
  const releaseSaves = documentSaveController.holdSaves();
  try {
    const revisionBeforeWrite = useEditorStore.getState().revision;
    const result = await write();
    const openPathAfterWrite = useWorkspaceStore.getState().currentPath;
    if (openPathAfterWrite !== null) {
      const destination = relocatedPath?.(openPathAfterWrite);
      if (destination === null) {
        closeTrashedDocument(openPathAfterWrite);
        return result;
      }
      if (destination !== undefined) {
        followRenamedDocument(openPathAfterWrite, destination);
      }
    }
    await reloadOpenDocumentIfUntouched(backend, rewrites, revisionBeforeWrite);
    return result;
  } finally {
    releaseSaves();
  }
}

/** 改名された文書を、読み直さずに新しいパスで開いたままにする。 */
function followRenamedDocument(oldPath: string, newPath: string): void {
  if (useEditorStore.getState().path === oldPath) {
    useEditorStore.getState().relocate(newPath);
  }
  useWorkspaceStore.getState().openDocument(newPath);
}

/** ゴミ箱へ移った文書を、エディタから外す。残すと、次の保存が存在しないファイルへの競合になる。 */
function closeTrashedDocument(path: string): void {
  useEditorStore.getState().reset();
  useWorkspaceStore.getState().clearCurrentDocument();
  useUiStore.getState().showToast(`開いていた「${path}」はゴミ箱へ移したので、閉じました。`);
}

async function reloadOpenDocumentIfUntouched(
  backend: Backend,
  rewrites: (path: string) => boolean,
  revisionBeforeWrite: number,
): Promise<void> {
  const openPath = useWorkspaceStore.getState().currentPath;
  if (openPath === null || !rewrites(openPath)) {
    return;
  }
  try {
    const file = await backend.readDocument(openPath);
    const editor = useEditorStore.getState();
    const isUntouchedSinceWrite =
      editor.path === openPath && editor.revision === revisionBeforeWrite;
    if (useWorkspaceStore.getState().currentPath === openPath && isUntouchedSinceWrite) {
      editor.loadDocument(openPath, file);
    }
  } catch (error) {
    useUiStore
      .getState()
      .showToast(toErrorMessage(error, "書き換えた文書を読み直せませんでした。"), "error");
  }
}
