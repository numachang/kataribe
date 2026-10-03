import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../../../../api/backend";
import { BackendError } from "../../../../api/backend";
import { BackendProvider } from "../../../../api/context";
import type { ParsedDocument } from "../../../../api/types";
import { createStubBackend } from "../../../../test/fakeBackend";
import { useParsedDocument } from "./useParsedDocument";

afterEach(() => {
  vi.restoreAllMocks();
});

function textResult(content: string): ParsedDocument {
  return { document: { kind: "text", content }, parse_error: null };
}

/** 呼ぶたびに、外から解決・失敗させられる結果を返す。 */
function deferredParseBackend() {
  const pending: Array<{
    content: string;
    resolve: (parsed: ParsedDocument) => void;
    reject: (error: unknown) => void;
  }> = [];
  const backend: Backend = createStubBackend({
    parseDocument: (_path, content) =>
      new Promise<ParsedDocument>((resolve, reject) => {
        pending.push({ content, resolve, reject });
      }),
  });
  function call(content: string) {
    const found = pending.find((candidate) => candidate.content === content);
    if (found === undefined) {
      throw new Error(`「${content}」を分ける呼び出しが無い`);
    }
    return found;
  }
  return { backend, call };
}

function renderParsed(backend: Backend, initial: { path: string; content: string | null }) {
  return renderHook(({ path, content }) => useParsedDocument(path, content), {
    initialProps: initial,
    wrapper: ({ children }: { children: ReactNode }) => (
      <BackendProvider backend={backend}>{children}</BackendProvider>
    ),
  });
}

describe("useParsedDocument", () => {
  it("内容が無いときは、分けずに absent を返す", () => {
    const { backend } = deferredParseBackend();

    const { result } = renderParsed(backend, { path: "characters/rin.md", content: null });

    expect(result.current).toEqual({ status: "absent" });
  });

  it("分け終わるまでは parsing、分け終わると ready を返す", async () => {
    const { backend, call } = deferredParseBackend();
    const { result } = renderParsed(backend, { path: "concept.md", content: "企画" });
    expect(result.current).toEqual({ status: "parsing" });

    call("企画").resolve(textResult("企画"));

    await waitFor(() => {
      expect(result.current).toEqual({ status: "ready", parsed: textResult("企画") });
    });
  });

  it("失敗は握りつぶさず、理由つきの failed として返す", async () => {
    const { backend, call } = deferredParseBackend();
    const { result } = renderParsed(backend, { path: "concept.md", content: "企画" });

    call("企画").reject(new BackendError("internal", "コマンドが登録されていません"));

    await waitFor(() => {
      expect(result.current).toEqual({
        status: "failed",
        reason: "コマンドが登録されていません",
      });
    });
  });

  it("入力が変わったら、前の入力の結果は新しい入力の結果として返さない", async () => {
    const { backend, call } = deferredParseBackend();
    const { result, rerender } = renderParsed(backend, { path: "concept.md", content: "古い" });
    call("古い").resolve(textResult("古い"));
    await waitFor(() => {
      expect(result.current.status).toBe("ready");
    });

    rerender({ path: "concept.md", content: "新しい" });

    expect(result.current).toEqual({ status: "parsing" });
  });

  it("古い入力の結果が、新しい入力の結果より後に届いても、新しい入力の結果が残る", async () => {
    const { backend, call } = deferredParseBackend();
    const { result, rerender } = renderParsed(backend, { path: "concept.md", content: "古い" });
    rerender({ path: "concept.md", content: "新しい" });

    call("新しい").resolve(textResult("新しい"));
    await waitFor(() => {
      expect(result.current).toEqual({ status: "ready", parsed: textResult("新しい") });
    });
    call("古い").resolve(textResult("古い"));
    await Promise.resolve();

    expect(result.current).toEqual({ status: "ready", parsed: textResult("新しい") });
  });

  it("古い入力の失敗が、新しい入力の表示に出ない", async () => {
    const { backend, call } = deferredParseBackend();
    const { result, rerender } = renderParsed(backend, { path: "concept.md", content: "古い" });
    rerender({ path: "concept.md", content: "新しい" });

    call("古い").reject(new Error("古い入力の失敗"));
    await Promise.resolve();

    expect(result.current).toEqual({ status: "parsing" });
  });

  it("アンマウントしたあとに結果が届いても、警告もエラーも出ない", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { backend, call } = deferredParseBackend();
    const { unmount } = renderParsed(backend, { path: "concept.md", content: "企画" });
    unmount();

    call("企画").resolve(textResult("企画"));
    await Promise.resolve();

    expect(consoleError).not.toHaveBeenCalled();
  });
});
