import type { KeyboardEvent } from "react";
import { useEffect, useRef, useState } from "react";
import "./ActionMenu.css";

export interface ActionMenuItem {
  label: string;
  onSelect: () => void;
  /** 無効のとき、その理由。項目の title に出す。 */
  disabledReason?: string;
}

interface ActionMenuProps {
  /** 開くボタンの名前（読み上げ用）。例: 「霧島 凛」の操作 */
  label: string;
  items: ActionMenuItem[];
}

const ENABLED_ITEM_SELECTOR = '[role="menuitem"]:not(:disabled)';

/**
 * 行ごとの操作メニュー（「⋯」を押すと開く）。
 * Escape・メニューの外のクリック・フォーカスがメニューの外へ移ったときに閉じる。
 */
export function ActionMenu({ label, items }: ActionMenuProps) {
  const [isOpen, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    listRef.current?.querySelector<HTMLElement>(ENABLED_ITEM_SELECTOR)?.focus();

    function closeIfOutside(event: Event): void {
      if (!containerRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    function closeOnEscape(event: globalThis.KeyboardEvent): void {
      // IME の変換中の Escape まで拾って閉じてしまわないようにする。
      if (event.key === "Escape" && !event.isComposing) {
        setOpen(false);
        triggerRef.current?.focus();
      }
    }
    document.addEventListener("pointerdown", closeIfOutside);
    document.addEventListener("focusin", closeIfOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeIfOutside);
      document.removeEventListener("focusin", closeIfOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [isOpen]);

  function moveFocus(event: KeyboardEvent<HTMLDivElement>): void {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (step === 0) {
      return;
    }
    event.preventDefault();
    const enabledItems = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>(ENABLED_ITEM_SELECTOR),
    );
    const current = enabledItems.indexOf(document.activeElement as HTMLElement);
    const next = (current + step + enabledItems.length) % enabledItems.length;
    enabledItems[next]?.focus();
  }

  function select(item: ActionMenuItem): void {
    setOpen(false);
    item.onSelect();
  }

  return (
    <div className="action-menu" ref={containerRef}>
      <button
        type="button"
        ref={triggerRef}
        className="action-menu__trigger"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        onClick={() => setOpen(!isOpen)}
      >
        ⋯
      </button>
      {isOpen && (
        <div
          className="action-menu__list"
          role="menu"
          aria-label={label}
          ref={listRef}
          tabIndex={-1}
          onKeyDown={moveFocus}
        >
          {items.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              className="action-menu__item"
              disabled={item.disabledReason !== undefined}
              title={item.disabledReason}
              onClick={() => select(item)}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
