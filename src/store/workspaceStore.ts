import { create } from "zustand";
import type { Backend } from "../api/backend";
import type { PipelineStep, ProjectOverview } from "../api/types";

interface WorkspaceState {
  overview: ProjectOverview | null;
  pipeline: PipelineStep[];
  /** 中央のエディタで開いているファイルの、作品フォルダからの相対パス。 */
  currentPath: string | null;

  openWorkspace: (overview: ProjectOverview) => void;
  closeWorkspace: () => void;
  openDocument: (path: string) => void;
  /** 選択中の文書を外す（読み込みに失敗したときなど、選択自体を取り消したいとき）。 */
  clearCurrentDocument: () => void;
  /** Backend を呼ばず、既に持っている目次で置き換える（applyChangeSet の戻り値の反映など）。 */
  setOverview: (overview: ProjectOverview) => void;
  /** `setOverview` の工程版。 */
  setPipeline: (pipeline: PipelineStep[]) => void;
  refreshOverview: (backend: Backend) => Promise<void>;
  refreshPipeline: (backend: Backend) => Promise<void>;
}

/** 開いている作品の目次・工程・選択中の文書。作品を閉じたらすべてリセットする。 */
export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  overview: null,
  pipeline: [],
  currentPath: null,

  openWorkspace(overview) {
    set({ overview, currentPath: null });
  },

  closeWorkspace() {
    set({ overview: null, pipeline: [], currentPath: null });
  },

  openDocument(path) {
    set({ currentPath: path });
  },

  clearCurrentDocument() {
    set({ currentPath: null });
  },

  setOverview(overview) {
    set({ overview });
  },

  setPipeline(pipeline) {
    set({ pipeline });
  },

  async refreshOverview(backend) {
    const overview = await backend.overview();
    set({ overview });
  },

  async refreshPipeline(backend) {
    const pipeline = await backend.pipeline();
    set({ pipeline });
  },
}));
