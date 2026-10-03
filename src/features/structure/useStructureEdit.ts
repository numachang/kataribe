import { useCallback } from "react";
import type { Backend } from "../../api/backend";
import { useBackend } from "../../api/context";
import type { StructureEdit, StructurePlan } from "../../api/types";
import {
  relocatedPath,
  renamesOpenDocument,
  rewritesPath,
  touchesPath,
} from "../../lib/changeSetPaths";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { documentSaveController } from "../editor/documentSaveController";
import { writeBesideEditor } from "../editor/openDocumentSync";

const UNSAVED_WORK_BLOCKS_EDIT =
  "開いている文書に保存できていない編集があるため、変更しませんでした。保存してから、もう一度操作してください。";

export interface StructureEditApi {
  /** 開いている文書の保存を済ませてから、操作の変更案を作る（作品は書き換えない）。 */
  prepare: (edit: StructureEdit) => Promise<StructurePlan>;
  /** 変更案を作品に適用し、目次と工程を直して、作った文書を開く（改名された文書を開いていたときは、そのまま続ける）。 */
  commit: (plan: StructurePlan) => Promise<void>;
  /** 確認を挟まない操作（追加・並べ替え）。変更案を作って、そのまま適用し、変更案の注意書きがあれば知らせる。 */
  apply: (edit: StructureEdit) => Promise<void>;
}

/**
 * 構成の操作（人物・世界観の資料・章・シーンの追加・削除・並べ替え）の流れ。
 * 変更案の組み立ては Rust（`planStructureEdit`）に任せ、画面は適用と、そのあとの画面の整え直しを受け持つ。
 * 開いている文書との食い違いを防ぐ手順（先に保存し、適用後に読み直すか閉じるか、改名されたパスへ付け替える）は `writeBesideEditor` に任せる。
 */
export function useStructureEdit(): StructureEditApi {
  const backend = useBackend();

  const prepare = useCallback(
    async (edit: StructureEdit): Promise<StructurePlan> => {
      await documentSaveController.flush(backend);
      return backend.planStructureEdit(edit);
    },
    [backend],
  );

  const commit = useCallback(
    (plan: StructurePlan): Promise<void> => whileStructureEditing(() => commitPlan(backend, plan)),
    [backend],
  );

  const apply = useCallback(
    (edit: StructureEdit): Promise<void> =>
      whileStructureEditing(async () => {
        const plan = await prepare(edit);
        await commit(plan);
        announceNotices(plan);
      }),
    [commit, prepare],
  );

  return { prepare, commit, apply };
}

/**
 * 構成の操作を始めてから終わるまでを、ストアに記録する。その間は、生成の開始も次の構成の操作も受け付けない
 * （適用の前後で目次の位置や章の番号が変わるので、古い番号のパスで生成が進み、別の章のフォルダに本文を書いてしまうのを防ぐため）。
 * `apply` が `commit` を呼ぶように入れ子になっても、件数で数えるので、内側が終わっただけで解けてしまうことはない。
 */
async function whileStructureEditing<Result>(work: () => Promise<Result>): Promise<Result> {
  const { beginStructureEdit, endStructureEdit } = useWorkspaceStore.getState();
  beginStructureEdit();
  try {
    return await work();
  } finally {
    endStructureEdit();
  }
}

async function commitPlan(backend: Backend, plan: StructurePlan): Promise<void> {
  const changeSet = plan.change_set;
  // 番号の振り直しで改名される文書を開いていたら、その文書のまま続けられるようにする
  // （作った文書を開くと、利用者が編集していた文書から切り替わってしまう）。適用の前に決める。
  const keepsRenamedDocument = renamesOpenDocument(
    changeSet,
    useWorkspaceStore.getState().currentPath,
  );
  const overview = await writeBesideEditor(backend, {
    touches: (path) => touchesPath(changeSet, path),
    relocatedPath: (path) => relocatedPath(changeSet, path),
    rewrites: (path) => rewritesPath(changeSet, path),
    unsavedWorkMessage: UNSAVED_WORK_BLOCKS_EDIT,
    write: () => backend.applyChangeSet(changeSet),
  });
  useWorkspaceStore.getState().setOverview(overview);
  await refreshPipelineAfterApply(backend);
  if (plan.created !== null && !keepsRenamedDocument) {
    useWorkspaceStore.getState().openDocument(plan.created);
  }
  useUiStore.getState().showToast(plan.completed_summary);
}

/**
 * 変更案の注意書き（読めない資料は後ろに並べたまま、など）を、適用の要約に続けて知らせる。
 * 確認のダイアログを通る削除は、ダイアログで先に見せているので、ここは通らない操作（追加・並べ替え）のため。
 */
function announceNotices(plan: StructurePlan): void {
  for (const notice of plan.notices) {
    useUiStore.getState().showToast(notice);
  }
}

/** 適用そのものは済んでいるので、工程を読み直せなくても失敗にはせず、知らせるだけにする。 */
async function refreshPipelineAfterApply(backend: Backend): Promise<void> {
  try {
    await useWorkspaceStore.getState().refreshPipeline(backend);
  } catch (error) {
    useUiStore.getState().showToast(toErrorMessage(error, "工程を読み直せませんでした。"), "error");
  }
}
