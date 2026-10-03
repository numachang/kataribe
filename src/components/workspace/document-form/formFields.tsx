import type { ChangeEvent } from "react";
import { useDraftInput } from "./useDraftInput";

// front matter の項目ごとの入力欄。見た目は設定ダイアログなどと同じ `.app-field`。

const NAME_SEPARATOR = /[、,，]/;
const NAME_SEPARATOR_FOR_DISPLAY = "、";

/** 全角の数字も受け付ける（日本語入力のまま打てるように）。 */
function toHalfWidthDigits(text: string): string {
  return text.normalize("NFKC").trim();
}

interface TextFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
}

/** 必須の 1 行の項目。 */
export function TextField({ label, value, onChange }: TextFieldProps) {
  return (
    <label className="app-field">
      <span>{label}</span>
      <input value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}

/** 必須の複数行の項目。 */
export function TextAreaField({ label, value, onChange }: TextFieldProps) {
  function handleChange(event: ChangeEvent<HTMLTextAreaElement>): void {
    onChange(event.target.value);
  }
  return (
    <label className="app-field">
      <span>{label}</span>
      <textarea rows={3} value={value} onChange={handleChange} spellCheck={false} />
    </label>
  );
}

interface OptionalTextFieldProps {
  label: string;
  /** 項目が無い（undefined）ときも、空欄として出す。 */
  value: string | null | undefined;
  onChange: (value: string | null) => void;
}

/** 空欄にすると null になる、任意の 1 行の項目。 */
export function OptionalTextField({ label, value, onChange }: OptionalTextFieldProps) {
  return (
    <label className="app-field">
      <span>{label}</span>
      <input
        value={value ?? ""}
        onChange={(event) => onChange(event.target.value.trim() === "" ? null : event.target.value)}
      />
    </label>
  );
}

interface IntegerFieldProps {
  label: string;
  value: number | null | undefined;
  /** 空欄にすると null。 */
  onChange: (value: number | null) => void;
}

/** 0 以上の整数の項目。空欄にすると null になる。数字でない入力は、フォーカスを外すと元の値に戻る。 */
export function IntegerField({ label, value, onChange }: IntegerFieldProps) {
  const input = useDraftInput<number | null>({
    value: value ?? null,
    format: (current) => (current === null ? "" : String(current)),
    parse: (text) => {
      const digits = toHalfWidthDigits(text);
      if (digits === "") {
        return { value: null };
      }
      return /^\d+$/.test(digits) ? { value: Number(digits) } : null;
    },
    onCommit: onChange,
  });
  return (
    <label className="app-field">
      <span>{label}</span>
      <input inputMode="numeric" {...input} />
    </label>
  );
}

interface NameListFieldProps {
  label: string;
  value: string[] | undefined;
  onChange: (value: string[]) => void;
}

/** 名前を「、」で区切って並べる項目。 */
export function NameListField({ label, value, onChange }: NameListFieldProps) {
  const input = useDraftInput<string[]>({
    value: value ?? [],
    format: (names) => names.join(NAME_SEPARATOR_FOR_DISPLAY),
    parse: (text) => ({
      value: text
        .split(NAME_SEPARATOR)
        .map((name) => name.trim())
        .filter((name) => name !== ""),
    }),
    onCommit: onChange,
  });
  return (
    <label className="app-field">
      <span>{label}</span>
      <input placeholder="「、」で区切って並べる" {...input} />
    </label>
  );
}
