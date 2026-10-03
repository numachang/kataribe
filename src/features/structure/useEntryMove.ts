import { useCallback } from "react";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";
import type { MoveEdit } from "./structureRequest";
import { useStructureEdit } from "./useStructureEdit";

/**
 * 目次の人物・章・シーンの並べ替え。確認は挟まず、変更案を作ってそのまま適用する（元に戻せる操作なので）。
 * 失敗したときは、理由をトーストで知らせる（ダイアログが無いので、出す場所がほかに無い）。
 *
 * 動かしている間は、続けて押された操作や生成の開始を止める。目次の位置や章の番号は適用のあとに変わるので、
 * 古い位置で変更案を作ったり、古い番号のパスで生成したりしてしまうため。止める仕組みは `useStructureEdit` が
 * ストアに記録する「構成の操作の途中」で、画面は `useStructureEditBlockedReason` などで参照する。
 */
export function useEntryMove(): (edit: MoveEdit) => Promise<void> {
  const { apply } = useStructureEdit();

  return useCallback(
    async (edit: MoveEdit): Promise<void> => {
      try {
        await apply(edit);
      } catch (error) {
        useUiStore.getState().showToast(toErrorMessage(error, "並べ替えに失敗しました。"), "error");
      }
    },
    [apply],
  );
}
