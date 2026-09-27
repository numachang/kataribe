import { useRef, useState } from "react";
import { useBackend } from "../../api/context";
import type { LlmSettings, ModelInfo } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";

/** 接続テストの状態と操作。見つかったモデルは、モデル欄の候補に使う。 */
export interface ConnectionTest {
  models: ModelInfo[];
  isTesting: boolean;
  /** `llm` で接続を試し、結果を知らせる。 */
  run: (llm: LlmSettings) => Promise<void>;
  /** 結果を捨てる。試している途中なら、その結果も出さない（接続先を切り替えたときに呼ぶ）。 */
  reset: () => void;
}

export function useConnectionTest(): ConnectionTest {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [isTesting, setIsTesting] = useState(false);
  // 試している途中で接続先を切り替えたら、前の接続先の結果を出さないための通し番号
  const testId = useRef(0);

  function reset(): void {
    testId.current += 1;
    setModels([]);
    setIsTesting(false);
  }

  async function run(llm: LlmSettings): Promise<void> {
    const currentId = ++testId.current;
    const isCurrent = () => currentId === testId.current;
    setIsTesting(true);
    try {
      const list = await backend.listModels(llm);
      if (!isCurrent()) {
        return;
      }
      setModels(list);
      showToast(
        llm.provider === "claude_code"
          ? "Claude Code を使えます（ログインしています）。"
          : `接続できました。${list.length} 件のモデルが見つかりました。`,
      );
    } catch (error) {
      if (isCurrent()) {
        showToast(toErrorMessage(error, "接続できませんでした。"), "error");
      }
    } finally {
      if (isCurrent()) {
        setIsTesting(false);
      }
    }
  }

  return { models, isTesting, run, reset };
}
