/** BackendError を含む任意の失敗を、トーストにそのまま出せる日本語メッセージへ変換する。 */
export function toErrorMessage(error: unknown, fallback = "エラーが発生しました。"): string {
  return error instanceof Error ? error.message : fallback;
}
