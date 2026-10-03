import type { ChapterMeta, ScenePlan } from "../../../api/types";
import { TextField } from "./formFields";
import { SceneCard } from "./SceneCard";

interface ChapterFormProps {
  meta: ChapterMeta;
  onChange: (meta: ChapterMeta) => void;
}

/** 章立て（plot/chapters/<NN>.md）の項目。シーンの追加・削除・並べ替えはここではしない。 */
export function ChapterForm({ meta, onChange }: ChapterFormProps) {
  const scenes = meta.scenes ?? [];

  function changeScene(changed: ScenePlan): void {
    onChange({
      ...meta,
      scenes: scenes.map((scene) => (scene.id === changed.id ? changed : scene)),
    });
  }

  return (
    <div className="document-form">
      <TextField
        label="章題"
        value={meta.title}
        onChange={(title) => onChange({ ...meta, title })}
      />
      {scenes.length > 0 && (
        <div className="document-form__scenes">
          <p className="document-form__heading">シーン</p>
          {scenes.map((scene) => (
            <SceneCard key={scene.id} scene={scene} onChange={changeScene} />
          ))}
        </div>
      )}
    </div>
  );
}
