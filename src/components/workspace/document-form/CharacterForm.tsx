import type { CharacterMeta } from "../../../api/types";
import { CHARACTER_FIELD_LABELS } from "./fieldLabels";
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
          label={CHARACTER_FIELD_LABELS.name}
          value={meta.name}
          onChange={(name) => onChange({ ...meta, name })}
        />
        <OptionalTextField
          label={CHARACTER_FIELD_LABELS.reading}
          value={meta.reading}
          onChange={(reading) => onChange({ ...meta, reading })}
        />
      </div>
      <div className="document-form__row">
        <TextField
          label={CHARACTER_FIELD_LABELS.role}
          value={meta.role}
          onChange={(role) => onChange({ ...meta, role })}
        />
        <IntegerField
          label={CHARACTER_FIELD_LABELS.order}
          value={meta.order}
          onChange={(order) => onChange({ ...meta, order })}
        />
      </div>
      <TextAreaField
        label={CHARACTER_FIELD_LABELS.summary}
        value={meta.summary}
        onChange={(summary) => onChange({ ...meta, summary })}
      />
    </div>
  );
}
