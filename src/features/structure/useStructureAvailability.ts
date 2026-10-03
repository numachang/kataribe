import { useGenerationSessionContext } from "../generation/GenerationSessionProvider";

/**
 * 構成の追加・削除をいま始められない理由。始められるなら null。
 * 生成の途中や、生成した変更案の確認中に構成を変えると、変更案の書き先（新規に書く本文など）がずれるため、
 * 生成のセッションが落ち着いている間だけ受け付ける。
 */
export function useStructureEditBlockedReason(): string | null {
  const session = useGenerationSessionContext();
  if (session.autoAdvancing) {
    return "自動で進めている間は、追加・削除できません。";
  }
  switch (session.phase) {
    case "idle":
      return null;
    case "running":
      return "生成している間は、追加・削除できません。";
    case "reviewing":
      return "生成した変更案を確認している間は、追加・削除できません。適用か破棄をしてから操作してください。";
    case "error":
      return "生成の失敗を閉じるまで、追加・削除できません。";
  }
}
