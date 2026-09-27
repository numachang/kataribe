import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { readStoredNumber, writeStoredNumber } from "../lib/storedNumber";
import "./ResizableSplit.css";

const MIN_PANEL_WIDTH = 200;
const MAX_PANEL_WIDTH = 560;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

interface ResizableSplitProps {
  /** 幅の記憶に使う localStorage のキー接頭辞。 */
  storageKey: string;
  left: ReactNode;
  center: ReactNode;
  right: ReactNode;
  defaultLeftWidth?: number;
  defaultRightWidth?: number;
}

type DragTarget = "left" | "right" | null;

/** 左右のペイン幅をドラッグで変えられる 3 ペインのレイアウト。幅は端末ごとに記憶する。 */
export function ResizableSplit({
  storageKey,
  left,
  center,
  right,
  defaultLeftWidth = 260,
  defaultRightWidth = 360,
}: ResizableSplitProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const dragTarget = useRef<DragTarget>(null);

  const [leftWidth, setLeftWidth] = useState(() =>
    readStoredNumber(`${storageKey}:left`, defaultLeftWidth),
  );
  const [rightWidth, setRightWidth] = useState(() =>
    readStoredNumber(`${storageKey}:right`, defaultRightWidth),
  );

  useEffect(() => {
    function handlePointerMove(event: PointerEvent): void {
      const container = containerRef.current;
      if (!dragTarget.current || !container) {
        return;
      }
      const rect = container.getBoundingClientRect();
      if (dragTarget.current === "left") {
        setLeftWidth(clamp(event.clientX - rect.left, MIN_PANEL_WIDTH, MAX_PANEL_WIDTH));
      } else {
        setRightWidth(clamp(rect.right - event.clientX, MIN_PANEL_WIDTH, MAX_PANEL_WIDTH));
      }
    }

    function handlePointerUp(): void {
      dragTarget.current = null;
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, []);

  useEffect(() => {
    writeStoredNumber(`${storageKey}:left`, leftWidth);
  }, [storageKey, leftWidth]);

  useEffect(() => {
    writeStoredNumber(`${storageKey}:right`, rightWidth);
  }, [storageKey, rightWidth]);

  return (
    <div className="resizable-split" ref={containerRef}>
      <div className="resizable-split__pane" style={{ width: leftWidth }}>
        {left}
      </div>
      <button
        type="button"
        className="resizable-split__divider"
        aria-label="左のペインの幅を変える"
        onPointerDown={() => {
          dragTarget.current = "left";
        }}
      />
      <div className="resizable-split__pane resizable-split__pane--center">{center}</div>
      <button
        type="button"
        className="resizable-split__divider"
        aria-label="右のペインの幅を変える"
        onPointerDown={() => {
          dragTarget.current = "right";
        }}
      />
      <div className="resizable-split__pane" style={{ width: rightWidth }}>
        {right}
      </div>
    </div>
  );
}
