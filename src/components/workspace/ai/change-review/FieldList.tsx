import { ChangeMark } from "./ChangeMark";
import type { FieldView } from "./documentComparison";

const EXTRA_FIELDS_HEADING = "その他の項目";

interface FieldListProps {
  fields: FieldView[];
}

function FieldValueText({ value }: { value: FieldView["value"] }) {
  if (Array.isArray(value)) {
    return (
      <ol className="field-list__beats">
        {value.map((beat, index) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: 同じ文のビートが並びうる。並べ替えないので、位置が安定した識別子になる
          <li key={`${index}-${beat}`}>{beat}</li>
        ))}
      </ol>
    );
  }
  return value === "" ? <span className="field-list__empty">（なし）</span> : value;
}

/** 項目名と値の、読むだけの一覧。変わった項目には印を付ける。 */
export function FieldList({ fields }: FieldListProps) {
  return (
    <dl className="field-list">
      {fields.map((field) => (
        <div
          key={field.label}
          className={
            field.changed ? "field-list__item field-list__item--changed" : "field-list__item"
          }
        >
          <dt className="field-list__label">{field.label}</dt>
          <dd className="field-list__value">
            <FieldValueText value={field.value} />
            {field.changed && <ChangeMark kind="changed" />}
          </dd>
        </div>
      ))}
    </dl>
  );
}

/**
 * アプリが知らない項目（利用者が足した項目）の一覧。無ければ何も出さない。
 * 書き直しで落ちたり変わったりしても気付けるよう、決まった項目と同じように印を付ける。
 */
export function ExtraFieldList({ fields }: FieldListProps) {
  if (fields.length === 0) {
    return null;
  }
  return (
    <div className="field-list__extra">
      <p className="field-list__extra-heading">{EXTRA_FIELDS_HEADING}</p>
      <FieldList fields={fields} />
    </div>
  );
}
