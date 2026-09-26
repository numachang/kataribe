import { useEffect, useState } from "react";
import { useBackend } from "../../../api/context";
import type { QualityReport } from "../../../api/types";
import { findOverviewEntry } from "../../../lib/overviewTree";
import { useEditorStore } from "../../../store/editorStore";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import "./QualityTab.css";

const ANALYZE_DEBOUNCE_MS = 500;

const SEVERITY_LABELS = { error: "重大", warning: "注意", info: "参考" } as const;

function toPercent(ratio: number): string {
  return `${Math.round(ratio * 100)}%`;
}

/** 「品質」タブ。開いている本文を機械的に検査し、指標と問題点を見せる。 */
export function QualityTab() {
  const backend = useBackend();
  const overview = useWorkspaceStore((state) => state.overview);
  const currentPath = useWorkspaceStore((state) => state.currentPath);
  const content = useEditorStore((state) => state.content);
  const [report, setReport] = useState<QualityReport | null>(null);

  const entry = findOverviewEntry(overview, currentPath);
  const targetChars = entry?.target_chars ?? null;

  useEffect(() => {
    if (currentPath === null) {
      setReport(null);
      return;
    }
    const timer = setTimeout(() => {
      void backend.analyzeQuality(content, targetChars).then(setReport);
    }, ANALYZE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [backend, content, currentPath, targetChars]);

  if (currentPath === null) {
    return <p className="quality-tab__empty">左の目次から文書を選んでください。</p>;
  }
  if (!report) {
    return <p className="quality-tab__empty">解析しています…</p>;
  }

  return (
    <div className="quality-tab">
      <dl className="quality-tab__metrics">
        <div>
          <dt>会話率</dt>
          <dd>{toPercent(report.metrics.dialogue_ratio)}</dd>
        </div>
        <div>
          <dt>漢字率</dt>
          <dd>{toPercent(report.metrics.kanji_ratio)}</dd>
        </div>
        <div>
          <dt>平均文長</dt>
          <dd>{report.metrics.average_sentence_length} 字</dd>
        </div>
        <div>
          <dt>文末の連続</dt>
          <dd>{report.metrics.longest_same_ending_run} 文</dd>
        </div>
        <div>
          <dt>言い回しの反復</dt>
          <dd>{toPercent(report.metrics.repeated_phrase_ratio)}</dd>
        </div>
      </dl>

      <div className="quality-tab__issues">
        {report.issues.length === 0 ? (
          <p className="quality-tab__no-issues">目立った問題は見つかりませんでした。</p>
        ) : (
          <ul>
            {report.issues.map((issue) => (
              <li
                key={`${issue.kind}:${issue.message}:${issue.excerpt ?? ""}`}
                className={`quality-tab__issue quality-tab__issue--${issue.severity}`}
              >
                <span className="quality-tab__issue-severity">
                  {SEVERITY_LABELS[issue.severity]}
                </span>
                <div>
                  <p className="quality-tab__issue-message">{issue.message}</p>
                  {issue.excerpt && <p className="quality-tab__issue-excerpt">{issue.excerpt}</p>}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
