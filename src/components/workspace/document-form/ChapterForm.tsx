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

  // シーンは id ではなく並びの位置で特定する（手で書いた章立てに、同じ id が並ぶことがあっても取り違えないため）。
  function changeScene(index: number, changed: ScenePlan): void {
    onChange({
      ...meta,
      scenes: scenes.map((scene, position) => (position === index ? changed : scene)),
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
          {scenes.map((scene, index) => (
            <SceneCard
              // biome-ignore lint/suspicious/noArrayIndexKey: 手で書いた章立てには同じ id のシーンが並びうる。ここではシーンを並べ替えないので、位置が安定した識別子になる
              key={`${index}-${scene.id}`}
              scene={scene}
              onChange={(changed) => changeScene(index, changed)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
