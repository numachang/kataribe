import { useState } from "react";
import type { OverviewEntry, SectionKind } from "../../api/types";
import type { MoveEdit, StructureRequest } from "../../features/structure/structureRequest";
import { useEntryMove } from "../../features/structure/useEntryMove";
import { useStructureEditBlockedReason } from "../../features/structure/useStructureAvailability";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { ActionMenu } from "../ActionMenu";
import type { EntryAction } from "./structure/entryActions";
import { entryActionsFor } from "./structure/entryActions";
import { StructureDialogs } from "./structure/StructureDialogs";
import "./ProjectTree.css";

function formatChars(entry: OverviewEntry): string | null {
  // target_chars が 0 以下は「目標なし」として扱う（quality.ts の detectLengthIssue と揃える）。
  if (entry.target_chars !== null && entry.target_chars > 0) {
    return `${entry.chars.toLocaleString("ja-JP")} / ${entry.target_chars.toLocaleString("ja-JP")} 字`;
  }
  if (entry.chars > 0) {
    return `${entry.chars.toLocaleString("ja-JP")} 字`;
  }
  return null;
}

/** 構成の操作を求める手段。追加・削除・並べ替えは、いま始められない理由があれば無効にする。 */
interface StructureControls {
  /** 始められない理由。始められるなら null。 */
  blockedReason: string | null;
  request: (request: StructureRequest) => void;
  move: (edit: MoveEdit) => void;
}

/** 並べ替えている間（適用が済んで目次が変わるまで）、構成の操作を受け付けない理由。 */
const MOVING_REASON = "並べ替えている間は、操作できません。";

/** 節の見出しの「＋」で足せるもの。 */
const SECTION_ADDITIONS: Partial<
  Record<SectionKind, { label: string; request: StructureRequest }>
> = {
  world: { label: "資料を追加", request: { kind: "add_world_document" } },
  characters: { label: "人物を追加", request: { kind: "add_character" } },
  plot: { label: "章を追加", request: { kind: "add_chapter", before: null } },
};

/**
 * 行の操作メニューの名前。章は、章立ての行と本文の章見出しが同じ題で並ぶので、章立ての行だけ「章立て」を添えて見分ける。
 */
function menuLabelFor(entry: OverviewEntry): string {
  const isPlanChapter = entry.kind === "chapter" && entry.path !== null;
  return isPlanChapter ? `「${entry.label}」の章立ての操作` : `「${entry.label}」の操作`;
}

interface EntryNodeProps {
  entry: OverviewEntry;
  depth: number;
  /** 同じ階層の項目の並び（この項目を含む）。「この後に追加」と並べ替えの位置に使う。 */
  siblings: OverviewEntry[];
  controls: StructureControls;
}

function EntryNode({ entry, depth, siblings, controls }: EntryNodeProps) {
  const currentPath = useWorkspaceStore((state) => state.currentPath);
  const isSelected = entry.path !== null && entry.path === currentPath;
  const charsLabel = formatChars(entry);

  const label = (
    <>
      {entry.error && (
        <span className="project-tree__warning" title={entry.error}>
          ⚠
        </span>
      )}
      <span className="project-tree__label">{entry.label}</span>
      {charsLabel && <span className="project-tree__chars">{charsLabel}</span>}
    </>
  );

  const rowClassName = [
    "project-tree__row",
    !entry.exists && "project-tree__row--pending",
    isSelected && "project-tree__row--selected",
  ]
    .filter(Boolean)
    .join(" ");

  const path = entry.path;
  const menuItems = entryActionsFor(entry, siblings).map((action) => ({
    label: action.label,
    onSelect: () => select(action),
    disabledReason: controls.blockedReason ?? undefined,
  }));

  function select(action: EntryAction): void {
    if (action.kind === "move") {
      controls.move(action.edit);
    } else {
      controls.request(action.request);
    }
  }

  return (
    <li>
      <div className={rowClassName}>
        {path !== null ? (
          <button
            type="button"
            className="project-tree__entry"
            style={{ paddingLeft: 12 + depth * 14 }}
            onClick={() => useWorkspaceStore.getState().openDocument(path)}
          >
            {label}
          </button>
        ) : (
          <div className="project-tree__entry" style={{ paddingLeft: 12 + depth * 14 }}>
            {label}
          </div>
        )}
        {menuItems.length > 0 && <ActionMenu label={menuLabelFor(entry)} items={menuItems} />}
      </div>
      {entry.children.length > 0 && (
        <ul className="project-tree__children">
          {entry.children.map((child, index) => (
            <EntryNode
              key={child.path ?? `${entry.label}-${index}`}
              entry={child}
              depth={depth + 1}
              siblings={entry.children}
              controls={controls}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

/** 左ペイン。作品の目次をツリーで表示し、文字数の進み具合とあわせて見せる。人物・資料・章・シーンの追加・削除と、人物・章・シーンの並べ替えもここから行う。 */
export function ProjectTree() {
  const overview = useWorkspaceStore((state) => state.overview);
  const sessionBlockedReason = useStructureEditBlockedReason();
  const { move, isMoving } = useEntryMove();
  const [request, setRequest] = useState<StructureRequest | null>(null);
  if (!overview) {
    return null;
  }

  const progress =
    overview.target_length > 0 ? Math.min(1, overview.total_chars / overview.target_length) : 0;
  const blockedReason = sessionBlockedReason ?? (isMoving ? MOVING_REASON : null);
  const controls: StructureControls = { blockedReason, request: setRequest, move };

  return (
    <div className="project-tree">
      <header className="project-tree__header">
        <h2 className="project-tree__title">{overview.title}</h2>
        <div className="project-tree__progress-bar">
          <div className="project-tree__progress-fill" style={{ width: `${progress * 100}%` }} />
        </div>
        <span className="project-tree__progress-label">
          {overview.target_length > 0
            ? `${overview.total_chars.toLocaleString("ja-JP")} / ${overview.target_length.toLocaleString("ja-JP")} 字`
            : `${overview.total_chars.toLocaleString("ja-JP")} 字`}
        </span>
      </header>

      <nav className="project-tree__sections">
        {overview.sections.map((section) => {
          const addition = SECTION_ADDITIONS[section.kind];
          return (
            <section key={section.kind} className="project-tree__section">
              <div className="project-tree__section-header">
                <h3>{section.label}</h3>
                {addition && (
                  <button
                    type="button"
                    className="project-tree__add"
                    aria-label={addition.label}
                    title={blockedReason ?? addition.label}
                    disabled={blockedReason !== null}
                    onClick={() => setRequest(addition.request)}
                  >
                    ＋
                  </button>
                )}
              </div>
              <ul>
                {section.entries.map((entry, index) => (
                  <EntryNode
                    key={entry.path ?? `${section.kind}-${index}`}
                    entry={entry}
                    depth={0}
                    siblings={section.entries}
                    controls={controls}
                  />
                ))}
              </ul>
            </section>
          );
        })}
      </nav>

      <StructureDialogs request={request} onClose={() => setRequest(null)} />
    </div>
  );
}
