import { beforeEach, describe, expect, it } from "vitest";
import { useUiStore } from "./uiStore";

function reset(): void {
  useUiStore.setState({ toasts: [] });
}

describe("useUiStore", () => {
  beforeEach(reset);

  it("showToast はトーストを追加し、その id を返す", () => {
    const id = useUiStore.getState().showToast("保存しました");
    expect(useUiStore.getState().toasts).toEqual([{ id, kind: "info", message: "保存しました" }]);
  });

  it("kind を省略すると info になる", () => {
    useUiStore.getState().showToast("完了");
    expect(useUiStore.getState().toasts[0]?.kind).toBe("info");
  });

  it("複数のトーストを積み重ねられる", () => {
    useUiStore.getState().showToast("ひとつめ");
    useUiStore.getState().showToast("ふたつめ", "error");
    expect(useUiStore.getState().toasts.map((toast) => toast.message)).toEqual([
      "ひとつめ",
      "ふたつめ",
    ]);
  });

  it("dismissToast は指定した id のトーストだけを取り除く", () => {
    const firstId = useUiStore.getState().showToast("ひとつめ");
    const secondId = useUiStore.getState().showToast("ふたつめ");

    useUiStore.getState().dismissToast(firstId);

    expect(useUiStore.getState().toasts).toEqual([
      { id: secondId, kind: "info", message: "ふたつめ" },
    ]);
  });
});
