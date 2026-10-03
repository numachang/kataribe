import { create } from "zustand";
import type { Backend } from "../api/backend";
import type { PipelineStep, ProjectOverview } from "../api/types";

interface WorkspaceState {
  overview: ProjectOverview | null;
  pipeline: PipelineStep[];
  /** 中央のエディタで開いているファイルの、作品フォルダからの相対パス。 */
  currentPath: string | null;
  /**
   * 構成の操作（追加・削除・並べ替え）を、作ってから適用し終えるまでの件数。0 より大きい間は、
   * 目次の位置や章の番号が変わる途中なので、生成を始めたり、新しい構成の操作を始めたりしてはならない。
   */
  activeStructureEdits: number;

  beginStructureEdit: () => void;
  endStructureEdit: () => void;
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
  activeStructureEdits: 0,

  beginStructureEdit() {
    set((state) => ({ activeStructureEdits: state.activeStructureEdits + 1 }));
  },

  endStructureEdit() {
    // 作品を閉じて 0 に戻ったあとに、終わりかけの操作が呼んでも負にしない
    set((state) => ({ activeStructureEdits: Math.max(0, state.activeStructureEdits - 1) }));
  },

  openWorkspace(overview) {
    set({ overview, currentPath: null });
  },

  closeWorkspace() {
    set({ overview: null, pipeline: [], currentPath: null, activeStructureEdits: 0 });
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
