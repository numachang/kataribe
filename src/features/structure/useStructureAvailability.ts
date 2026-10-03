import { useGenerationSessionContext } from "../generation/GenerationSessionProvider";

/** 目次の追加・削除・並べ替えの操作を指す、既定の言い方。 */
const STRUCTURE_EDIT_OPERATION = "追加・削除・並べ替え";

/**
 * 構成の操作（追加・削除・並べ替え、まだ無い本文を空で作り始めること）を、いま始められない理由。始められるなら null。
 * 生成の途中や、生成した変更案の確認中に構成を変えると、変更案の書き先（新規に書く本文など）がずれて、
 * 生成した内容を適用できなくなるため、生成のセッションが落ち着いている間だけ受け付ける。
 *
 * `operation` は、理由の文に入れる操作の名前（「追加・削除・並べ替え」など。「…できません」に続く形で渡す）。
 */
export function useStructureEditBlockedReason(
  operation: string = STRUCTURE_EDIT_OPERATION,
): string | null {
  const session = useGenerationSessionContext();
  if (session.autoAdvancing) {
    return `自動で進めている間は、${operation}できません。`;
  }
  switch (session.phase) {
    case "idle":
      return null;
    case "running":
      return `生成している間は、${operation}できません。`;
    case "reviewing":
      return `生成した変更案を確認している間は、${operation}できません。適用か破棄をしてから操作してください。`;
    case "error":
      return `生成の失敗を閉じるまで、${operation}できません。`;
  }
}
