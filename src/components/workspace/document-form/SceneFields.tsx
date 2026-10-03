import type { ScenePlan } from "../../../api/types";
import { SCENE_FIELD_LABELS } from "./fieldLabels";
import {
  IntegerField,
  NameListField,
  OptionalTextField,
  TextAreaField,
  TextField,
} from "./formFields";

/** シーンの項目のうち、利用者が自分で書くもの（id とビートは含まない）。 */
export type SceneFieldValues = Pick<
  ScenePlan,
  "title" | "summary" | "pov" | "characters" | "place" | "time" | "target_chars"
>;

interface SceneFieldsProps<Scene extends SceneFieldValues> {
  scene: Scene;
  onChange: (scene: Scene) => void;
}

/**
 * シーンの項目の入力欄。章立てのシーンのカードと、シーンを足すダイアログが共有する。
 * 渡された `scene` の、この欄にない項目（id・ビートなど）はそのまま引き継いで返す。
 */
export function SceneFields<Scene extends SceneFieldValues>({
  scene,
  onChange,
}: SceneFieldsProps<Scene>) {
  return (
    <div className="document-form">
      <TextField
        label={SCENE_FIELD_LABELS.title}
        value={scene.title}
        onChange={(title) => onChange({ ...scene, title })}
      />
      <TextAreaField
        label={SCENE_FIELD_LABELS.summary}
        value={scene.summary}
        onChange={(summary) => onChange({ ...scene, summary })}
      />
      <div className="document-form__row">
        <OptionalTextField
          label={SCENE_FIELD_LABELS.pov}
          value={scene.pov}
          onChange={(pov) => onChange({ ...scene, pov })}
        />
        <NameListField
          label={SCENE_FIELD_LABELS.characters}
          value={scene.characters}
          onChange={(characters) => onChange({ ...scene, characters })}
        />
      </div>
      <div className="document-form__row">
        <OptionalTextField
          label={SCENE_FIELD_LABELS.place}
          value={scene.place}
          onChange={(place) => onChange({ ...scene, place })}
        />
        <OptionalTextField
          label={SCENE_FIELD_LABELS.time}
          value={scene.time}
          onChange={(time) => onChange({ ...scene, time })}
        />
        <IntegerField
          label={SCENE_FIELD_LABELS.target_chars}
          value={scene.target_chars}
          onChange={(target_chars) => onChange({ ...scene, target_chars })}
        />
      </div>
    </div>
  );
}
