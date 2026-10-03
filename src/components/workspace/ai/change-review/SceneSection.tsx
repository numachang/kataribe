import { ChangeMark } from "./ChangeMark";
import type { SceneView } from "./documentComparison";
import { ExtraFieldList, FieldList } from "./FieldList";

interface SceneSectionProps {
  scene: SceneView;
}

/** 章立ての 1 シーンの、読むだけのまとまり。追加・削除・変更・順序変更の印をシーンの見出しに付ける。 */
export function SceneSection({ scene }: SceneSectionProps) {
  const className = scene.marks.includes("removed")
    ? "scene-section scene-section--removed"
    : "scene-section";
  return (
    <section className={className}>
      <h5 className="scene-section__heading">
        <span>
          {scene.id}　{scene.title}
        </span>
        {scene.marks.map((mark) => (
          <ChangeMark key={mark} kind={mark} />
        ))}
      </h5>
      <FieldList fields={scene.fields} />
      <ExtraFieldList fields={scene.extraFields} />
    </section>
  );
}
