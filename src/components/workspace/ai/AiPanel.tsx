import { useState } from "react";
import { useGenerationSessionContext } from "../../../features/generation/GenerationSessionProvider";
import { ChangeSetReview } from "./ChangeSetReview";
import { DocumentTab } from "./DocumentTab";
import { GenerationProgress } from "./GenerationProgress";
import { PipelineTab } from "./PipelineTab";
import { QualityTab } from "./QualityTab";
import "./AiPanel.css";

type AiTab = "pipeline" | "document" | "quality";

const TAB_LABELS: Record<AiTab, string> = {
  pipeline: "工程",
  document: "この文書",
  quality: "品質",
};

/** 右ペイン「AI」。工程・この文書・品質の 3 つのタブを持ち、生成中はどのタブからでも進捗を見せる。 */
export function AiPanel() {
  const [tab, setTab] = useState<AiTab>("pipeline");
  const session = useGenerationSessionContext();
  const isGenerating = session.phase === "running" || session.phase === "error";
  const isReviewing = session.phase === "reviewing";

  return (
    <div className="ai-panel">
      <div className="ai-panel__tabs" role="tablist">
        {(Object.keys(TAB_LABELS) as AiTab[]).map((key) => (
          <button
            key={key}
            type="button"
            role="tab"
            aria-selected={tab === key}
            className={`ai-panel__tab${tab === key ? " ai-panel__tab--active" : ""}`}
            onClick={() => setTab(key)}
          >
            {TAB_LABELS[key]}
          </button>
        ))}
      </div>

      <div className="ai-panel__body">
        {isGenerating && <GenerationProgress />}
        {isReviewing && <ChangeSetReview />}
        {!isGenerating && !isReviewing && (
          <>
            {tab === "pipeline" && <PipelineTab />}
            {tab === "document" && <DocumentTab />}
            {tab === "quality" && <QualityTab />}
          </>
        )}
      </div>
    </div>
  );
}
