import type { CharacterMeta } from "../../../api/types";
import { IntegerField, OptionalTextField, TextAreaField, TextField } from "./formFields";

interface CharacterFormProps {
  meta: CharacterMeta;
  onChange: (meta: CharacterMeta) => void;
}

/** 人物資料（characters/<id>.md）の項目。触っていない項目は、読んだままの値で送り返す。 */
export function CharacterForm({ meta, onChange }: CharacterFormProps) {
  return (
    <div className="document-form">
      <div className="document-form__row">
        <TextField
          label="名前"
          value={meta.name}
          onChange={(name) => onChange({ ...meta, name })}
        />
        <OptionalTextField
          label="読み"
          value={meta.reading}
          onChange={(reading) => onChange({ ...meta, reading })}
        />
      </div>
      <div className="document-form__row">
        <TextField
          label="役割"
          value={meta.role}
          onChange={(role) => onChange({ ...meta, role })}
        />
        <IntegerField
          label="順番"
          value={meta.order}
          onChange={(order) => onChange({ ...meta, order })}
        />
      </div>
      <TextAreaField
        label="概要"
        value={meta.summary}
        onChange={(summary) => onChange({ ...meta, summary })}
      />
    </div>
  );
}
