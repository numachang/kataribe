import type { OverviewEntry } from "../../api/types";
import { useWorkspaceStore } from "../../store/workspaceStore";
import "./ProjectTree.css";

function formatChars(entry: OverviewEntry): string | null {
  if (entry.target_chars !== null) {
    return `${entry.chars.toLocaleString("ja-JP")} / ${entry.target_chars.toLocaleString("ja-JP")} 字`;
  }
  if (entry.chars > 0) {
    return `${entry.chars.toLocaleString("ja-JP")} 字`;
  }
  return null;
}

interface EntryNodeProps {
  entry: OverviewEntry;
  depth: number;
}

function EntryNode({ entry, depth }: EntryNodeProps) {
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
    "project-tree__entry",
    !entry.exists && "project-tree__entry--pending",
    isSelected && "project-tree__entry--selected",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <li>
      {entry.path !== null ? (
        <button
          type="button"
          className={rowClassName}
          style={{ paddingLeft: 12 + depth * 14 }}
          onClick={() => useWorkspaceStore.getState().openDocument(entry.path ?? "")}
        >
          {label}
        </button>
      ) : (
        <div className={rowClassName} style={{ paddingLeft: 12 + depth * 14 }}>
          {label}
        </div>
      )}
      {entry.children.length > 0 && (
        <ul className="project-tree__children">
          {entry.children.map((child, index) => (
            <EntryNode
              key={child.path ?? `${entry.label}-${index}`}
              entry={child}
              depth={depth + 1}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

/** 左ペイン。作品の目次をツリーで表示し、文字数の進み具合とあわせて見せる。 */
export function ProjectTree() {
  const overview = useWorkspaceStore((state) => state.overview);
  if (!overview) {
    return null;
  }

  const progress =
    overview.target_length > 0 ? Math.min(1, overview.total_chars / overview.target_length) : 0;

  return (
    <div className="project-tree">
      <header className="project-tree__header">
        <h2 className="project-tree__title">{overview.title}</h2>
        <div className="project-tree__progress-bar">
          <div className="project-tree__progress-fill" style={{ width: `${progress * 100}%` }} />
        </div>
        <span className="project-tree__progress-label">
          {overview.total_chars.toLocaleString("ja-JP")} /{" "}
          {overview.target_length.toLocaleString("ja-JP")} 字
        </span>
      </header>

      <nav className="project-tree__sections">
        {overview.sections.map((section) => (
          <section key={section.kind} className="project-tree__section">
            <h3>{section.label}</h3>
            <ul>
              {section.entries.map((entry, index) => (
                <EntryNode key={entry.path ?? `${section.kind}-${index}`} entry={entry} depth={0} />
              ))}
            </ul>
          </section>
        ))}
      </nav>
    </div>
  );
}
