import { useEffect, useState } from "react";
import { useBackend } from "../../api/context";
import type { Segment } from "../../api/types";
import "./RubyPreview.css";

interface RubyPreviewProps {
  text: string;
  vertical: boolean;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
}

/** 本文を、ルビ・傍点を実際の見た目で描くプレビュー。縦書きにも対応する。 */
export function RubyPreview({
  text,
  vertical,
  fontFamily,
  fontSize,
  lineHeight,
}: RubyPreviewProps) {
  const backend = useBackend();
  const [segments, setSegments] = useState<Segment[]>([]);

  useEffect(() => {
    let cancelled = false;
    void backend.parseRuby(text).then((result) => {
      if (!cancelled) {
        setSegments(result);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [backend, text]);

  return (
    <div
      className={`ruby-preview${vertical ? " ruby-preview--vertical" : ""}`}
      style={{ fontFamily, fontSize, lineHeight }}
    >
      {segments.map((segment, index) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: セグメントに一意な id がないため
        <RubySegment key={index} segment={segment} />
      ))}
    </div>
  );
}

function RubySegment({ segment }: { segment: Segment }) {
  if (segment.kind === "ruby") {
    return (
      <ruby>
        {segment.base}
        <rt>{segment.reading}</rt>
      </ruby>
    );
  }
  if (segment.kind === "emphasis") {
    return <span className="ruby-preview__emphasis">{segment.text}</span>;
  }
  return <>{segment.text}</>;
}
