import type { ScenePlan } from "../../../api/types";
import { SCENE_FIELD_LABELS } from "./fieldLabels";
import { SceneFields } from "./SceneFields";

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
        <SceneFields scene={scene} onChange={onChange} />
        {beats.length > 0 && (
          <div className="scene-card__beats">
            <p className="scene-card__beats-heading">
              {SCENE_FIELD_LABELS.beats}（生成された展開。ここでは直せません）
            </p>
            <ol>
              {beats.map((beat, index) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: 同じ文のビートが並びうる。ここでは並べ替えないので、位置が安定した識別子になる
                <li key={`${index}-${beat}`}>{beat}</li>
              ))}
            </ol>
          </div>
        )}
      </div>
    </details>
  );
}
