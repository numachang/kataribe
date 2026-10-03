import type { ChangeEvent } from "react";
import { useState } from "react";

interface DraftInputOptions<Value> {
  /** 文書が持っている今の値。 */
  value: Value;
  /** 値を入力欄に出す文字列にする。 */
  format: (value: Value) => string;
  /** 入力した文字列を値にする。値にできない文字列（打ちかけの入力など）は null。 */
  parse: (text: string) => { value: Value } | null;
  /** 入力が値になるたびに呼ばれる。 */
  onCommit: (value: Value) => void;
}

/**
 * 文字列と値が一対一でない入力欄（「、」区切りの名前、空欄が null の数値など）のための状態。
 *
 * 入力中は、打った文字列をそのまま入力欄に出す。値への変換は入力のたびに行って文書へ反映するが、
 * 「霧島 凛、」の末尾の「、」のように、値にすると消える打ちかけの文字を、入力欄から消さないため。
 * 値からの整形（「、」でそろえる・数字を整える）は、フォーカスを外したときに初めて入力欄へ反映する。
 * 値にできない文字列（`parse` が null）は文書へ反映しないので、フォーカスを外すと、
 * 途中で確定した最後の有効な値に戻る。
 */
export function useDraftInput<Value>({ value, format, parse, onCommit }: DraftInputOptions<Value>) {
  const [draft, setDraft] = useState<string | null>(null);

  return {
    value: draft ?? format(value),
    onChange(event: ChangeEvent<HTMLInputElement>) {
      const text = event.target.value;
      setDraft(text);
      const parsed = parse(text);
      if (parsed) {
        onCommit(parsed.value);
      }
    },
    onBlur() {
      setDraft(null);
    },
  };
}
