import type { ScenePlan } from "../../../api/types";
import {
  IntegerField,
  NameListField,
  OptionalTextField,
  TextAreaField,
  TextField,
} from "./formFields";

interface SceneCardProps {
  scene: ScenePlan;
  onChange: (scene: ScenePlan) => void;
}

/** 章立ての 1 シーン。見出し（id とタイトル）で開閉する。ビートは生成された展開なので、読むだけにする。 */
export function SceneCard({ scene, onChange }: SceneCardProps) {
  const beats = scene.beats ?? [];
  return (
    <details className="scene-card">
      <summary className="scene-card__heading">
        {scene.id}　{scene.title}
      </summary>
      <div className="document-form scene-card__fields">
        <TextField
          label="タイトル"
          value={scene.title}
          onChange={(title) => onChange({ ...scene, title })}
        />
        <TextAreaField
          label="概要"
          value={scene.summary}
          onChange={(summary) => onChange({ ...scene, summary })}
        />
        <div className="document-form__row">
          <OptionalTextField
            label="視点"
            value={scene.pov}
            onChange={(pov) => onChange({ ...scene, pov })}
          />
          <NameListField
            label="登場人物"
            value={scene.characters}
            onChange={(characters) => onChange({ ...scene, characters })}
          />
        </div>
        <div className="document-form__row">
          <OptionalTextField
            label="場所"
            value={scene.place}
            onChange={(place) => onChange({ ...scene, place })}
          />
          <OptionalTextField
            label="時間"
            value={scene.time}
            onChange={(time) => onChange({ ...scene, time })}
          />
          <IntegerField
            label="目標文字数"
            value={scene.target_chars}
            onChange={(target_chars) => onChange({ ...scene, target_chars })}
          />
        </div>
        {beats.length > 0 && (
          <div className="scene-card__beats">
            <p className="scene-card__beats-heading">
              ビート（生成された展開。ここでは直せません）
            </p>
            <ol>
              {beats.map((beat) => (
                <li key={beat}>{beat}</li>
              ))}
            </ol>
          </div>
        )}
      </div>
    </details>
  );
}
