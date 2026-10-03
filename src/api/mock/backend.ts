import { hashText } from "../../lib/hash";
import { applyProjectSettings } from "../../lib/projectSettings";
import { analyzeQualityText } from "../../lib/quality";
import { parseRubySegments } from "../../lib/ruby";
import { computeTextStats } from "../../lib/textStats";
import type { Backend } from "../backend";
import { BackendError } from "../backend";
import type {
  AppSettings,
  ChangeSet,
  DocumentFile,
  EditableDocument,
  GenerationEvent,
  GenrePreset,
  LlmSettings,
  ModelInfo,
  NewProject,
  ParsedDocument,
  PipelineStep,
  ProjectOverview,
  ProjectSettings,
  ProjectSettingsFile,
  QualityReport,
  Segment,
  StructureEdit,
  StructurePlan,
  Task,
  TextStats,
  TrashFileChange,
} from "../types";
import { parseMockDocument, readMockDocument, writeMockDocument } from "./document";
import {
  createGenerationJob,
  GenerationCancelled,
  type GenerationJob,
  runGeneration,
} from "./generation";
import { GENRE_PRESETS } from "./genres";
import { buildOverview } from "./overview";
import { isProjectRelativePath, MANIFEST_PATH } from "./paths";
import { buildPipeline } from "./pipeline";
import { readMockFile } from "./render";
import {
  createEmptyProjectState,
  createSampleProjectState,
  SAMPLE_PROJECT_FOLDER,
} from "./sampleProject";
import type { ProjectState } from "./state";
import { planMockStructureEdit, suggestMockCharacterId } from "./structure";
import { trashMockFile, writeMockFile } from "./write";

const MAX_RECENT_PROJECTS = 10;
const DEFAULT_CHUNK_DELAY_MS = 30;

const DEFAULT_SETTINGS: AppSettings = {
  llm: {
    provider: "openai_compatible",
    base_url: "http://localhost:1234/v1",
    model: "",
    claude_command: "claude",
    claude_model: "sonnet",
  },
  generation: {
    draft_unit: "beat",
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

const CLAUDE_CODE_MODELS: ModelInfo[] = [
  { id: "sonnet", context_length: null },
  { id: "opus", context_length: null },
  { id: "haiku", context_length: null },
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

  async loadProjectSettings(): Promise<ProjectSettingsFile> {
    const project = this.requireProject();
    return { settings: structuredClone(project.settings), hash: manifestHash(project) };
  }

  /** 本物と違い、数値を範囲に収めることや、この版が知らない項目を残すことはしない（画面のテスト用の簡略版）。 */
  async saveProjectSettings(settings: ProjectSettings, expectedHash: string): Promise<string> {
    const project = this.requireProject();
    if (manifestHash(project) !== expectedHash) {
      throw new BackendError("conflict", "kataribe.yaml が外部で変更されています。");
    }
    project.settings = structuredClone(settings);
    return manifestHash(project);
  }

  /** 生成に使う設定。作品を開いていれば、アプリ全体の設定に作品の設定を重ねる。 */
  private effectiveSettings() {
    return this.project
      ? applyProjectSettings(this.settings, this.project.settings)
      : { llm: this.settings.llm, generation: this.settings.generation };
  }

  async listModels(llm?: LlmSettings): Promise<ModelInfo[]> {
    const effective = llm ?? this.effectiveSettings().llm;
    if (effective.provider === "claude_code") {
      return CLAUDE_CODE_MODELS;
    }
    if (effective.base_url.trim().length === 0) {
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

  async readDocument(path: string): Promise<DocumentFile> {
    const project = this.requireProject();
    const document = readMockDocument(project, path);
    const content = readMockFile(project, path);
    if (document === null || content === null) {
      throw new BackendError("not_found", `「${path}」はまだ生成されていません。`);
    }
    return { document, hash: hashText(content), parse_error: null };
  }

  /**
   * 返すハッシュは、保存した後に readDocument で読んだときのハッシュと同じ。
   * 渡された文書ではなく、状態から組み立て直したファイルの内容で数える
   * （人物資料・章立ては、保存した文書と書き出される文字列が一致するとは限らないため）。
   */
  async writeDocument(
    path: string,
    document: EditableDocument,
    expectedHash: string | null,
  ): Promise<string> {
    const project = this.requireProject();
    this.checkWriteConflict(project, path, expectedHash);
    const next = writeMockDocument(project, path, document);
    const written = readMockFile(next, path);
    if (written === null) {
      throw new BackendError("invalid_input", `「${path}」は偽バックエンドでは保存できません。`);
    }
    this.setCurrentProject(next);
    return hashText(written);
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

  /** 作品を開いていなくても使える。本物の RelPath と同じく、作品の外を指すパスは invalid_input にする。 */
  async parseDocument(path: string, content: string): Promise<ParsedDocument> {
    if (!isProjectRelativePath(path)) {
      throw new BackendError("invalid_input", `「${path}」は作品内の相対パスではありません。`);
    }
    return parseMockDocument(path, content);
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
    onEvent({ kind: "started", model: describeLlm(this.effectiveSettings().llm) });
    try {
      const changes = await runGeneration(
        project,
        task,
        this.effectiveSettings().generation,
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
    checkChangeSetShape(changeSet);
    // 本物と同じく、ゴミ箱へ移す → 書く の順。状態は書き換えずに進め、競合したら何も変えない。
    for (const file of changeSet.files) {
      if (file.kind === "trash") {
        project = this.trashFile(project, file);
      }
    }
    for (const file of changeSet.files) {
      if (file.kind === "write") {
        this.checkWriteConflict(project, file.path, file.base_hash);
        project = writeMockFile(project, file.path, file.content);
      }
    }
    this.setCurrentProject(project);
    return buildOverview(project);
  }

  private trashFile(project: ProjectState, file: TrashFileChange): ProjectState {
    const current = readMockFile(project, file.path);
    const expectedHash = file.files[0]?.base_hash ?? null;
    if (current === null || hashText(current) !== expectedHash) {
      throw new BackendError(
        "conflict",
        `「${file.path}」が確かめたあとに変更されたため、ゴミ箱へ移しませんでした。`,
      );
    }
    return trashMockFile(project, file.path);
  }

  async planStructureEdit(edit: StructureEdit): Promise<StructurePlan> {
    return planMockStructureEdit(this.requireProject(), edit);
  }

  async suggestCharacterId(reading: string, name: string): Promise<string> {
    return suggestMockCharacterId(this.requireProject(), reading, name);
  }
}

const PROTECTED_TRASH_PATHS = [MANIFEST_PATH];
const PROTECTED_TRASH_FOLDER = ".kataribe/";

/** 本物と同じく、画面から戻ってくる変更案の形を確かめる（同じパスへの変更の重なり、ゴミ箱へ移せないパス）。 */
function checkChangeSetShape(changeSet: ChangeSet): void {
  const seen = new Set<string>();
  for (const file of changeSet.files) {
    const key = file.path.toLowerCase();
    if (seen.has(key)) {
      throw new BackendError("invalid_input", `「${file.path}」への変更が重なっています。`);
    }
    seen.add(key);
    const isProtected =
      PROTECTED_TRASH_PATHS.includes(file.path) || file.path.startsWith(PROTECTED_TRASH_FOLDER);
    if (file.kind === "trash" && isProtected) {
      throw new BackendError("invalid_input", `「${file.path}」はゴミ箱へ移せません。`);
    }
  }
}

/** メモリ上で完結する偽バックエンドを作る。画面の開発（`pnpm dev`）とテストで使う。 */
export function createMockBackend(options: MockBackendOptions = {}): Backend {
  return new MockBackend(options);
}

/** 偽の作品の kataribe.yaml のハッシュ（作品の設定の競合検出に使う）。 */
function manifestHash(project: ProjectState): string {
  return hashText(readMockFile(project, MANIFEST_PATH) ?? "");
}

/** 偽の生成で知らせる、使っている LLM の名前（本物の ChatModel::describe と同じ形）。 */
function describeLlm(llm: LlmSettings): string {
  if (llm.provider === "claude_code") {
    return `Claude Code（${llm.claude_model}）`;
  }
  return `OpenAI 互換 API（${serverName(llm.base_url)}）・${llm.model.trim() || "サーバーの既定のモデル"}`;
}

/** 接続先の URL のうちホスト名とポートだけを返す。認証情報やクエリは秘密を含みうるので出さない（本物と同じ）。 */
function serverName(baseUrl: string): string {
  const unreadable = "接続先の URL を解釈できません";
  try {
    // "localhost:1234/v1" のようにスキームが無いと、例外にならずホスト名が空になる
    return new URL(baseUrl).host || unreadable;
  } catch {
    return unreadable;
  }
}
