import { SCENES_HEADING } from "../../document-form/fieldLabels";
import { ChangeMark } from "./ChangeMark";
import type { DocumentView } from "./documentComparison";
import { ExtraFieldList, FieldList } from "./FieldList";
import { SceneSection } from "./SceneSection";
import "./StructuredDocumentView.css";

interface StructuredDocumentViewProps {
  view: DocumentView;
}

/** 変更案の人物資料・章立てを、項目の一覧・シーン・本文に分けて見せる（読むだけ。YAML は見せない）。 */
export function StructuredDocumentView({ view }: StructuredDocumentViewProps) {
  return (
    <div className="structured-view">
      <FieldList fields={view.fields} />
      <ExtraFieldList fields={view.extraFields} />
      {view.scenes.length > 0 && (
        <div className="structured-view__scenes">
          <h4 className="structured-view__heading">{SCENES_HEADING}</h4>
          {view.scenes.map((scene) => (
            <SceneSection key={scene.id} scene={scene} />
          ))}
        </div>
      )}
      <div className="structured-view__body-block">
        <h4 className="structured-view__heading">
          {view.bodyHeading}
          {view.bodyChanged && <ChangeMark kind="changed" />}
        </h4>
        <pre className="structured-view__body">{view.body}</pre>
      </div>
    </div>
  );
}
