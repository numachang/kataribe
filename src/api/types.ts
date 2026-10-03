// 画面とアプリ本体（Rust）の間でやり取りする型。
// Rust 側の serde 表現と一致させること（フィールド名は snake_case、列挙は小文字の文字列）。

// ---- 作品 ----

export type Rating = "general" | "r15" | "r18";

export interface Manifest {
  format: number;
  title: string;
  author: string | null;
  genre: string;
  genre_note: string | null;
  rating: Rating;
  target_length: number;
  idea: string;
}

export interface NewProject {
  title: string;
  author: string | null;
  genre: string;
  genre_note: string | null;
  rating: Rating;
  target_length: number;
  idea: string;
}

export interface GenrePreset {
  id: string;
  label: string;
  description: string;
}

export type SectionKind = "planning" | "world" | "characters" | "plot" | "manuscript";

export type EntryKind =
  | "manifest"
  | "concept"
  | "style"
  | "world"
  | "character"
  | "synopsis"
  | "chapter"
  | "scene"
  | "other";

export interface OverviewEntry {
  /** 作品フォルダからの相対パス（`/` 区切り）。ファイルに対応しない見出しは null。 */
  path: string | null;
  label: string;
  kind: EntryKind;
  exists: boolean;
  /** 本文の文字数（ルビの読み・空白を除く）。 */
  chars: number;
  target_chars: number | null;
  /** 読み込みや YAML の解析に失敗したときの説明。 */
  error: string | null;
  children: OverviewEntry[];
}

export interface OverviewSection {
  kind: SectionKind;
  label: string;
  entries: OverviewEntry[];
}

export interface ProjectOverview {
  root: string;
  title: string;
  target_length: number;
  total_chars: number;
  sections: OverviewSection[];
}

/** `characters/<id>.md` の front matter。 */
export interface CharacterMeta {
  name: string;
  reading?: string | null;
  role: string;
  summary: string;
  order?: number | null;
}

/** 1 シーンぶんの設計（`plot/chapters/<NN>.md` の `scenes` の要素）。 */
export interface ScenePlan {
  /** シーン id。本文ファイル名にも使う。 */
  id: string;
  title: string;
  summary: string;
  /** 視点人物（人物の id ではなく名前）。 */
  pov?: string | null;
  /** 登場人物の名前の一覧。 */
  characters?: string[];
  place?: string | null;
  time?: string | null;
  target_chars?: number | null;
  /** ビート単位で生成したときの展開の一覧。 */
  beats?: string[];
}

/** `plot/chapters/<NN>.md` の front matter。 */
export interface ChapterMeta {
  title: string;
  /** シーン構成。並び順がそのままシーンの順序になる。 */
  scenes?: ScenePlan[];
}

/**
 * 画面で編集する文書。人物資料と章立ては front matter を項目に分け、それ以外は文字列のまま扱う。
 * 章立ての `body` は、その章のストーリーライン。
 */
export type EditableDocument =
  | { kind: "text"; content: string }
  | { kind: "character"; meta: CharacterMeta; body: string }
  | { kind: "chapter"; meta: ChapterMeta; body: string };

export interface DocumentFile {
  document: EditableDocument;
  /** 読み込んだ時点のファイル全体のハッシュ。上書き時の競合検出に使う。 */
  hash: string;
  /** 人物資料・章立てなのに front matter を解釈できず、文字列として返したときの理由。 */
  parse_error: string | null;
}

// ---- テキスト ----

export interface TextStats {
  chars: number;
  paragraphs: number;
  dialogue_lines: number;
  manuscript_pages: number;
}

export type Segment =
  | { kind: "text"; text: string }
  | { kind: "ruby"; base: string; reading: string }
  | { kind: "emphasis"; text: string };

export type Severity = "info" | "warning" | "error";

export type IssueKind =
  | "meta_commentary"
  | "markdown_artifact"
  | "foreign_script"
  | "repeated_sentence"
  | "repeated_phrase"
  | "monotonous_endings"
  | "too_short"
  | "too_long"
  | "unbalanced_brackets";

export interface QualityIssue {
  kind: IssueKind;
  severity: Severity;
  message: string;
  excerpt: string | null;
}

export interface QualityMetrics {
  dialogue_ratio: number;
  kanji_ratio: number;
  average_sentence_length: number;
  longest_same_ending_run: number;
  repeated_phrase_ratio: number;
}

export interface QualityReport {
  stats: TextStats;
  metrics: QualityMetrics;
  issues: QualityIssue[];
}

// ---- 設定 ----

export type DraftUnit = "chapter" | "scene" | "beat";

/** 生成に使う LLM の種類。 */
export type LlmProvider = "openai_compatible" | "claude_code";

export interface LlmSettings {
  provider: LlmProvider;
  /** OpenAI 互換 API のベース URL。例: http://localhost:1234/v1 */
  base_url: string;
  /** OpenAI 互換 API のモデル。 */
  model: string;
  /** Claude Code の claude コマンド（PATH に無ければ実行ファイルの場所）。 */
  claude_command: string;
  /** Claude Code のモデル（sonnet・opus・haiku など）。 */
  claude_model: string;
}

/**
 * 作品ごとの設定（kataribe.yaml の settings）。書いた項目だけが入り、無い項目はアプリ全体の設定を使う。
 * 接続先 URL・claude コマンドの場所・API キーは PC ごとの設定なので持たない。
 */
export interface ProjectSettings {
  provider?: LlmProvider;
  /** OpenAI 互換 API のモデル。 */
  model?: string;
  /** Claude Code のモデル。 */
  claude_model?: string;
  draft_unit?: DraftUnit;
  chars_per_call?: number;
  context_tokens?: number;
  temperature?: number;
  polish?: boolean;
  quality_retries?: number;
  disable_thinking?: boolean;
}

/** 画面が読み込んだ作品の設定。保存するときの競合の検出に、読んだ時点の kataribe.yaml のハッシュを使う。 */
export interface ProjectSettingsFile {
  settings: ProjectSettings;
  hash: string;
}

export interface GenerationSettings {
  draft_unit: DraftUnit;
  /** 1 回の生成で書かせる目安の文字数。 */
  chars_per_call: number;
  /** モデルに渡せる文脈の長さ（トークン）。 */
  context_tokens: number;
  temperature: number;
  /** 本文を書いたあとに推敲パスをかけるか。 */
  polish: boolean;
  /** 品質チェックで重大な問題が見つかったときの再生成回数。 */
  quality_retries: number;
  /**
   * 推論モデルの「思考」を止める（既定 true）。ローカル LLM では思考に出力の上限を使い切って
   * 本文が空になることがあるため。クラウドの API でエラーになる場合は false にする。
   */
  disable_thinking: boolean;
}

export type FontStyle = "mincho" | "gothic";

export interface EditorPreferences {
  vertical: boolean;
  font_style: FontStyle;
  font_size: number;
  line_height: number;
}

export interface AppSettings {
  llm: LlmSettings;
  generation: GenerationSettings;
  editor: EditorPreferences;
  recent_projects: string[];
}

export interface ModelInfo {
  id: string;
  context_length: number | null;
}

// ---- 生成 ----

export type Task =
  | { kind: "concept" }
  | { kind: "style" }
  | { kind: "world" }
  | { kind: "cast" }
  | { kind: "character"; id: string }
  | { kind: "synopsis" }
  | { kind: "outline" }
  | { kind: "scene_plan"; chapter: string }
  | { kind: "draft"; chapter: string; scene: string }
  | { kind: "revise"; path: string; instruction: string };

export type StepState = "done" | "ready" | "blocked";

export interface PipelineStep {
  task: Task;
  label: string;
  state: StepState;
  /** blocked のとき、先に済ませる必要がある工程の説明。 */
  blocked_by: string | null;
}

export type GenerationEvent =
  /** 生成を始めた。model は使う LLM の名前（作品の設定を重ねた後の、実際の接続先とモデル）。 */
  | { kind: "started"; model: string }
  | { kind: "step_started"; label: string; index: number; total: number }
  | { kind: "content"; text: string }
  | { kind: "reasoning"; text: string }
  | {
      kind: "step_finished";
      prompt_tokens: number | null;
      completion_tokens: number | null;
      elapsed_ms: number;
    }
  | { kind: "notice"; level: "info" | "warning"; message: string };

export interface FileChange {
  path: string;
  content: string;
  /** 変更前の内容。新規ファイルなら null。 */
  previous: string | null;
  /** 変更前の内容のハッシュ。適用時に競合を検出するために使う。 */
  base_hash: string | null;
}

export interface ChangeSet {
  summary: string;
  files: FileChange[];
  /** この変更案を作った作品フォルダ。別の作品を開き直したあとに適用すると拒否される。 */
  project_root: string;
}
