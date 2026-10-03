import { useCallback, useState } from "react";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";
import type { MoveEdit } from "./structureRequest";
import { useStructureEdit } from "./useStructureEdit";

/**
 * 目次の人物・章・シーンの並べ替え。確認は挟まず、変更案を作ってそのまま適用する（元に戻せる操作なので）。
 * 失敗したときは、理由をトーストで知らせる（ダイアログが無いので、出す場所がほかに無い）。
 *
 * 動かしている間は `isMoving` を返す。目次の位置は適用のあとに変わるので、続けて押された操作は、
 * 古い目次の位置で変更案を作ってしまい、意図しない所へ動かすため。画面はその間、構成の操作を無効にする。
 */
export function useEntryMove() {
  const { apply } = useStructureEdit();
  const [isMoving, setMoving] = useState(false);

  const move = useCallback(
    async (edit: MoveEdit): Promise<void> => {
      setMoving(true);
      try {
        await apply(edit);
      } catch (error) {
        useUiStore.getState().showToast(toErrorMessage(error, "並べ替えに失敗しました。"), "error");
      } finally {
        setMoving(false);
      }
    },
    [apply],
  );

  return { move, isMoving };
}
