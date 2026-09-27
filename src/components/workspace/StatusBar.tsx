import { useEffect, useState } from "react";
import { useBackend } from "../../api/context";
import type { TextStats } from "../../api/types";
import type { SaveStatus } from "../../store/editorStore";
import "./StatusBar.css";

const STATS_DEBOUNCE_MS = 300;

interface StatusBarProps {
  text: string;
  targetChars: number | null;
  status: SaveStatus;
  errorMessage: string | null;
}

function saveStatusLabel(status: SaveStatus, errorMessage: string | null): string {
  switch (status) {
    case "clean":
      return "保存済み";
    case "dirty":
      return "未保存の変更があります";
    case "saving":
      return "保存中…";
    case "error":
      return errorMessage ?? "保存に失敗しました";
  }
}

/** 文字数・原稿用紙換算・保存状態を表示する下部バー。文字数は入力が落ち着いてから数え直す。 */
export function StatusBar({ text, targetChars, status, errorMessage }: StatusBarProps) {
  const backend = useBackend();
  const [stats, setStats] = useState<TextStats | null>(null);

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(() => {
      void backend.textStats(text).then((result) => {
        // 応答が届く前に text が変わっていたら（別の文書に切り替わったなど）、古い結果は無視する。
        if (!cancelled) {
          setStats(result);
        }
      });
    }, STATS_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [backend, text]);

  const progress = stats && targetChars ? Math.round((stats.chars / targetChars) * 100) : null;

  return (
    <div className="status-bar">
      <span>{stats ? `${stats.chars.toLocaleString("ja-JP")} 字` : "…"}</span>
      {stats && <span>原稿用紙換算 {stats.manuscript_pages.toFixed(1)} 枚</span>}
      {progress !== null && targetChars !== null && (
        <span>
          目標 {targetChars.toLocaleString("ja-JP")} 字中 {progress}%
        </span>
      )}
      <span className={`status-bar__save status-bar__save--${status}`}>
        {saveStatusLabel(status, errorMessage)}
      </span>
    </div>
  );
}
