import { hashText } from "../../lib/hash";
import { analyzeQualityText } from "../../lib/quality";
import { parseRubySegments } from "../../lib/ruby";
import { computeTextStats } from "../../lib/textStats";
import type { Backend } from "../backend";
import { BackendError } from "../backend";
import type {
  AppSettings,
  ChangeSet,
  FileChange,
  GenerationEvent,
  GenrePreset,
  LlmSettings,
  ModelInfo,
  NewProject,
  PipelineStep,
  ProjectOverview,
  QualityReport,
  Segment,
  Task,
  TextFile,
  TextStats,
} from "../types";
import {
  createGenerationJob,
  GenerationCancelled,
  type GenerationJob,
  runGeneration,
} from "./generation";
import { GENRE_PRESETS } from "./genres";
import { buildOverview } from "./overview";
import { buildPipeline } from "./pipeline";
import { readMockFile } from "./render";
import {
  createEmptyProjectState,
  createSampleProjectState,
  SAMPLE_PROJECT_FOLDER,
} from "./sampleProject";
import type { ProjectState } from "./state";
import { writeMockFile } from "./write";

const MAX_RECENT_PROJECTS = 10;
const DEFAULT_CHUNK_DELAY_MS = 30;

const DEFAULT_SETTINGS: AppSettings = {
  llm: { base_url: "http://localhost:1234/v1", model: "" },
  generation: {
    draft_unit: "scene",
    chars_per_call: 1500,
    context_tokens: 16384,
    temperature: 0.8,
    polish: true,
    quality_retries: 1,
    disable_thinking: true,
  },
  editor: { vertical: true, font_style: "mincho", font_size: 17, line_height: 1.9 },
  recent_projects: [SAMPLE_PROJECT_FOLDER],
};

const MODEL_LIST: ModelInfo[] = [
  { id: "lmstudio-community/Meta-Llama-3.1-8B-Instruct-GGUF", context_length: 8192 },
  { id: "lmstudio-community/Qwen2.5-14B-Instruct-GGUF", context_length: 32768 },
  { id: "lmstudio-community/Ministral-8B-Instruct-2410-GGUF", context_length: 32768 },
];

export interface MockBackendOptions {
  /** ストリーミングの 1 チャンクあたりの待ち時間（ミリ秒）。テストでは 0 にできる。 */
  delayMs?: number;
  /** フォルダ選択ダイアログの代わり。既定では毎回新しいフォルダパスを返す。 */
  pickFolder?: () => Promise<string | null>;
}

class MockBackend implements Backend {
  private settings: AppSettings = structuredClone(DEFAULT_SETTINGS);
  private storedApiKey = false;
  private project: ProjectState | null = null;
  private readonly projectsByFolder = new Map<string, ProjectState>();
  private readonly jobs = new Map<string, GenerationJob>();
  private readonly delayMs: number;
  private readonly pickFolderImpl: () => Promise<string | null>;
  private folderCounter = 0;

  constructor(options: MockBackendOptions) {
    this.delayMs = options.delayMs ?? DEFAULT_CHUNK_DELAY_MS;
    this.pickFolderImpl = options.pickFolder ?? (() => this.pickDefaultFolder());
    this.projectsByFolder.set(
      SAMPLE_PROJECT_FOLDER,
      createSampleProjectState(SAMPLE_PROJECT_FOLDER),
    );
  }

  private async pickDefaultFolder(): Promise<string> {
    this.folderCounter += 1;
    return `C:\\Users\\demo\\Documents\\新しい作品${this.folderCounter}`;
  }

  private requireProject(): ProjectState {
    if (!this.project) {
      throw new BackendError("not_found", "作品が開かれていません。");
    }
    return this.project;
  }

  private setCurrentProject(next: ProjectState): void {
    this.project = next;
    this.projectsByFolder.set(next.folder, next);
  }

  /** 作品を開き直す・閉じる。Rust と同じく、前の作品のための生成は中止する。 */
  private switchProject(next: ProjectState | null): void {
    for (const job of this.jobs.values()) {
      job.cancel();
    }
    if (next) {
      this.setCurrentProject(next);
    } else {
      this.project = null;
    }
  }

  private addRecentProject(folder: string): void {
    const withoutFolder = this.settings.recent_projects.filter((entry) => entry !== folder);
    this.settings.recent_projects = [folder, ...withoutFolder].slice(0, MAX_RECENT_PROJECTS);
  }

  async loadSettings(): Promise<AppSettings> {
    return structuredClone(this.settings);
  }

  async saveSettings(settings: AppSettings): Promise<void> {
    this.settings = structuredClone(settings);
  }

  async setApiKey(apiKey: string | null): Promise<void> {
    this.storedApiKey = apiKey !== null;
  }

  async hasApiKey(): Promise<boolean> {
    return this.storedApiKey;
  }

  async listModels(llm?: LlmSettings): Promise<ModelInfo[]> {
    if (llm && llm.base_url.trim().length === 0) {
      throw new BackendError("invalid_input", "接続先 URL を入力してください。");
    }
    return MODEL_LIST;
  }

  async listGenres(): Promise<GenrePreset[]> {
    return GENRE_PRESETS;
  }

  async pickFolder(): Promise<string | null> {
    return this.pickFolderImpl();
  }

  async createProject(folder: string, project: NewProject): Promise<ProjectOverview> {
    if (project.title.trim().length === 0) {
      throw new BackendError("invalid_input", "題名を入力してください。");
    }
    if (this.projectsByFolder.has(folder)) {
      throw new BackendError(
        "invalid_input",
        "フォルダが空ではありません。空のフォルダを選んでください。",
      );
    }
    const state = createEmptyProjectState(folder, project);
    this.switchProject(state);
    this.addRecentProject(folder);
    return buildOverview(state);
  }

  async openProject(folder: string): Promise<ProjectOverview> {
    const state = this.projectsByFolder.get(folder);
    if (!state) {
      throw new BackendError(
        "not_found",
        "指定されたフォルダに作品が見つかりません。kataribe.yaml があるフォルダを選んでください。",
      );
    }
    this.switchProject(state);
    this.addRecentProject(folder);
    return buildOverview(state);
  }

  async closeProject(): Promise<void> {
    this.switchProject(null);
  }

  async overview(): Promise<ProjectOverview> {
    return buildOverview(this.requireProject());
  }

  async pipeline(): Promise<PipelineStep[]> {
    return buildPipeline(this.requireProject());
  }

  async readFile(path: string): Promise<TextFile> {
    const content = readMockFile(this.requireProject(), path);
    if (content === null) {
      throw new BackendError("not_found", `「${path}」はまだ生成されていません。`);
    }
    return { content, hash: hashText(content) };
  }

  async writeFile(path: string, content: string, expectedHash: string | null): Promise<string> {
    const project = this.requireProject();
    this.checkWriteConflict(project, path, expectedHash);
    const next = writeMockFile(project, path, content);
    this.setCurrentProject(next);
    return hashText(content);
  }

  private checkWriteConflict(
    project: ProjectState,
    path: string,
    expectedHash: string | null,
  ): void {
    const current = readMockFile(project, path);
    const currentHash = current !== null ? hashText(current) : null;
    if (expectedHash !== currentHash) {
      throw new BackendError(
        "conflict",
        "この文書は外部で変更されています。再読み込みするか、上書きしてください。",
      );
    }
  }

  async textStats(text: string): Promise<TextStats> {
    return computeTextStats(text);
  }

  async parseRuby(text: string): Promise<Segment[]> {
    return parseRubySegments(text);
  }

  async analyzeQuality(text: string, targetChars: number | null): Promise<QualityReport> {
    return analyzeQualityText(text, targetChars);
  }

  async generate(
    jobId: string,
    task: Task,
    onEvent: (event: GenerationEvent) => void,
  ): Promise<ChangeSet> {
    const project = this.requireProject();
    if (this.jobs.has(jobId)) {
      throw new BackendError("invalid_input", "同じ ID の生成が既に実行中です。");
    }
    const job = createGenerationJob();
    this.jobs.set(jobId, job);
    try {
      const changes = await runGeneration(
        project,
        task,
        this.settings.generation,
        job,
        this.delayMs,
        onEvent,
      );
      return { ...changes, project_root: project.folder };
    } catch (error) {
      if (error instanceof GenerationCancelled) {
        throw new BackendError("cancelled", error.message);
      }
      if (error instanceof BackendError) {
        throw error;
      }
      throw new BackendError(
        "internal",
        error instanceof Error ? error.message : "生成中に不明なエラーが発生しました。",
      );
    } finally {
      this.jobs.delete(jobId);
    }
  }

  async cancelGeneration(jobId: string): Promise<void> {
    this.jobs.get(jobId)?.cancel();
  }

  async applyChangeSet(changeSet: ChangeSet): Promise<ProjectOverview> {
    let project = this.requireProject();
    if (changeSet.project_root !== project.folder) {
      throw new BackendError(
        "invalid_input",
        `この変更案は別の作品（${changeSet.project_root}）のものなので、今開いている作品には適用できません。`,
      );
    }
    for (const file of changeSet.files) {
      project = this.applyFileChange(project, file);
    }
    this.setCurrentProject(project);
    return buildOverview(project);
  }

  private applyFileChange(project: ProjectState, file: FileChange): ProjectState {
    this.checkWriteConflict(project, file.path, file.base_hash);
    return writeMockFile(project, file.path, file.content);
  }
}

/** メモリ上で完結する偽バックエンドを作る。画面の開発（`pnpm dev`）とテストで使う。 */
export function createMockBackend(options: MockBackendOptions = {}): Backend {
  return new MockBackend(options);
}
