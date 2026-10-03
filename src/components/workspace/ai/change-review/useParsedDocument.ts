import { useEffect, useState } from "react";
import { useBackend } from "../../../../api/context";
import type { ParsedDocument } from "../../../../api/types";
import { toErrorMessage } from "../../../../lib/errorMessage";

/** 分けた結果。 */
export type ParsedDocumentState =
  /** 分ける内容が無い（変更前の無い新規ファイル）。 */
  | { status: "absent" }
  | { status: "parsing" }
  | { status: "ready"; parsed: ParsedDocument }
  /** 分ける呼び出し自体に失敗した（IPC の失敗、コマンドの登録漏れなど）。内容が分けられなかったこととは別。 */
  | { status: "failed"; reason: string };

interface Settled {
  path: string;
  content: string;
  state: Extract<ParsedDocumentState, { status: "ready" | "failed" }>;
}

/**
 * 文字列を、パスの種類に応じて項目と本文に分ける（Rust 側の分け方）。
 *
 * 分け終わるまでと、失敗したときも、呼び出し側が内容をそのまま見せられるよう、結果の種類で返す
 * （何も見えない時間を作らない。失敗は握りつぶさず、理由を画面に出せるようにする）。
 * `content` が null なら、呼ばずに `absent` を返す。
 */
export function useParsedDocument(path: string, content: string | null): ParsedDocumentState {
  const backend = useBackend();
  const [settled, setSettled] = useState<Settled | null>(null);

  useEffect(() => {
    if (content === null) {
      return undefined;
    }
    let isStale = false;
    backend.parseDocument(path, content).then(
      (parsed) => {
        if (!isStale) {
          setSettled({ path, content, state: { status: "ready", parsed } });
        }
      },
      (error: unknown) => {
        if (!isStale) {
          const reason = toErrorMessage(error, "原因は不明です。");
          setSettled({ path, content, state: { status: "failed", reason } });
        }
      },
    );
    return () => {
      isStale = true;
    };
  }, [backend, path, content]);

  if (content === null) {
    return { status: "absent" };
  }
  // 前の入力の結果は、入力が変わったら使わない。
  return settled?.path === path && settled.content === content
    ? settled.state
    : { status: "parsing" };
}
