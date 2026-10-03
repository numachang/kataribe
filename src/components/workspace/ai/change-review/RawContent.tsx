interface RawContentProps {
  content: string;
  /** 項目に分けて見せられなかったことの説明。無ければ（項目に分けない文書など）何も添えない。 */
  notice: string | null;
}

/** ファイルの中身を、そのまま見せる。項目に分けられなかったときは、理由も添える。 */
export function RawContent({ content, notice }: RawContentProps) {
  return (
    <>
      {notice !== null && <p className="changeset-review__notice">{notice}</p>}
      <pre className="changeset-review__content">{content}</pre>
    </>
  );
}
