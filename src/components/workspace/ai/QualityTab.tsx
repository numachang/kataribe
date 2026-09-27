import { useEffect, useState } from "react";
import { useBackend } from "../../../api/context";
import type { QualityReport } from "../../../api/types";
import { isManuscriptFile } from "../../../lib/manuscript";
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
  const editorPath = useEditorStore((state) => state.path);
  const content = useEditorStore((state) => state.content);
  const [report, setReport] = useState<QualityReport | null>(null);

  const entry = findOverviewEntry(overview, currentPath);
  const targetChars = entry?.target_chars ?? null;
  const isManuscript = isManuscriptFile(currentPath);
  // エディタの読み込みが currentPath に追いついているか。ずれている間は前の文書の内容で
  // 解析してしまわないよう、読み込み中の表示にする。
  const isCurrentDocumentLoaded = editorPath === currentPath;

  useEffect(() => {
    if (currentPath === null || !isManuscript || !isCurrentDocumentLoaded) {
      setReport(null);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      void backend.analyzeQuality(content, targetChars).then((result) => {
        // 応答が届く前に文書が切り替わっていたら、古い結果は無視する。
        if (!cancelled) {
          setReport(result);
        }
      });
    }, ANALYZE_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [backend, content, currentPath, isCurrentDocumentLoaded, isManuscript, targetChars]);

  if (currentPath === null) {
    return <p className="quality-tab__empty">左の目次から文書を選んでください。</p>;
  }
  if (!isManuscript) {
    return <p className="quality-tab__empty">本文（.txt）を開いているときだけ確認できます。</p>;
  }
  if (entry && !entry.exists) {
    return <p className="quality-tab__empty">この本文はまだ生成されていません。</p>;
  }
  if (!isCurrentDocumentLoaded || !report) {
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
