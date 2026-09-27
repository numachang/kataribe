import type {
  AppSettings,
  ChangeSet,
  GenerationEvent,
  GenrePreset,
  LlmSettings,
  ModelInfo,
  NewProject,
  PipelineStep,
  ProjectOverview,
  ProjectSettings,
  ProjectSettingsFile,
  QualityReport,
  Segment,
  Task,
  TextFile,
  TextStats,
} from "./types";

/**
 * 画面から見たアプリ本体の機能。
 * 本番は Tauri コマンド、ブラウザ単体での開発とテストではメモリ上の偽実装を使う。
 * 失敗は日本語のメッセージを持つ `BackendError` として reject される。
 */
export interface Backend {
  // 設定
  loadSettings(): Promise<AppSettings>;
  saveSettings(settings: AppSettings): Promise<void>;
  /** API キーを OS の資格情報ストアに保存する。null で削除。 */
  setApiKey(apiKey: string | null): Promise<void>;
  hasApiKey(): Promise<boolean>;
  /** 開いている作品の設定（kataribe.yaml の settings）と、読んだ時点の kataribe.yaml のハッシュ。 */
  loadProjectSettings(): Promise<ProjectSettingsFile>;
  /**
   * 開いている作品の設定を保存し、新しいハッシュを返す。空なら settings の項目ごと消す。
   * expectedHash（読んだときのハッシュ）から kataribe.yaml が変わっていれば kind = "conflict" で失敗する。
   */
  saveProjectSettings(settings: ProjectSettings, expectedHash: string): Promise<string>;
  /**
   * LLM サーバーのモデル一覧。接続テストにも使う。
   * llm を渡すと、保存前の入力中の接続先で試す（API キーは保存済みのものを使う）。
   * 渡さなければ、開いている作品の設定を重ねた接続先を使う。
   */
  listModels(llm?: LlmSettings): Promise<ModelInfo[]>;
  listGenres(): Promise<GenrePreset[]>;

  // 作品
  pickFolder(): Promise<string | null>;
  createProject(folder: string, project: NewProject): Promise<ProjectOverview>;
  openProject(folder: string): Promise<ProjectOverview>;
  closeProject(): Promise<void>;
  overview(): Promise<ProjectOverview>;
  pipeline(): Promise<PipelineStep[]>;

  // ファイル
  readFile(path: string): Promise<TextFile>;
  /**
   * ファイルを保存し、新しいハッシュを返す。
   * expectedHash が現在の内容と一致しなければ kind = "conflict" で失敗する。新規作成なら null。
   */
  writeFile(path: string, content: string, expectedHash: string | null): Promise<string>;

  // テキスト
  textStats(text: string): Promise<TextStats>;
  parseRuby(text: string): Promise<Segment[]>;
  analyzeQuality(text: string, targetChars: number | null): Promise<QualityReport>;

  // 生成
  /** 生成を実行し、変更案を返す。途中経過は onEvent で届く。jobId は呼び出し側が採番する。 */
  generate(
    jobId: string,
    task: Task,
    onEvent: (event: GenerationEvent) => void,
  ): Promise<ChangeSet>;
  cancelGeneration(jobId: string): Promise<void>;
  applyChangeSet(changeSet: ChangeSet): Promise<ProjectOverview>;
}

export type BackendErrorKind =
  | "conflict"
  | "not_found"
  | "invalid_input"
  | "llm"
  | "cancelled"
  | "io"
  | "internal";

/**
 * アプリ本体から返されたエラー。message は利用者にそのまま見せられる日本語。
 * 作品を開いていない状態で作品の操作を呼んだときは kind = "not_found"。
 */
export class BackendError extends Error {
  readonly kind: BackendErrorKind;

  constructor(kind: BackendErrorKind, message: string) {
    super(message);
    this.name = "BackendError";
    this.kind = kind;
  }
}
