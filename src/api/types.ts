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
  /**
   * 章に属する項目（章立て・本文の章見出し・シーンの本文）の章。
   * 本文の章見出しにはパスが無いので、画面が章を知るために持つ。
   */
  chapter: string | null;
  /** シーンの本文の項目のシーン。 */
  scene: string | null;
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

/** 文字列を、画面で編集する形に分けた結果。`parse_error` は `DocumentFile` と同じ意味。 */
export interface ParsedDocument {
  document: EditableDocument;
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

/** ゴミ箱へ移すファイル。 */
export interface TrashedFile {
  path: string;
  /** 移す前の内容のハッシュ。適用時に競合を検出するために使う。テキストとして読めなかったファイルは null（適用できない）。 */
  base_hash: string | null;
  /** 内容の文字数（ルビの読み・空白を除く）。何が失われるかを利用者に見せるため。 */
  chars: number;
}

/** 作品フォルダのファイルへの 1 つの変更。 */
export type FileChange =
  | {
      kind: "write";
      path: string;
      content: string;
      /** 変更前の内容。新規ファイルなら null。 */
      previous: string | null;
      /** 変更前の内容のハッシュ。適用時に競合を検出するために使う。 */
      base_hash: string | null;
    }
  | {
      kind: "trash";
      path: string;
      /**
       * 移すファイルの一覧。ファイルを移すときは path のファイル 1 つだけ。フォルダを移すときは、
       * フォルダの中のファイル全部。
       */
      files: TrashedFile[];
    }
  | {
      kind: "move";
      /** 移動元（ファイルまたはフォルダ）。 */
      from: string;
      /** 移動先。 */
      to: string;
    }
  | {
      kind: "expect";
      path: string;
      /** そのファイルの今のハッシュ。null なら「何も無いこと」。 */
      base_hash: string | null;
    };

/** 変更案のうち、ファイルの新規作成・上書き。 */
export type WriteFileChange = Extract<FileChange, { kind: "write" }>;

/** 変更案のうち、ゴミ箱へ移す変更。 */
export type TrashFileChange = Extract<FileChange, { kind: "trash" }>;

/** 変更案のうち、ファイルまたはフォルダの改名（章の番号の振り直し）。 */
export type MoveFileChange = Extract<FileChange, { kind: "move" }>;

/** 変更案のうち、書かずに状態だけを確かめる変更。 */
export type ExpectFileChange = Extract<FileChange, { kind: "expect" }>;

export interface ChangeSet {
  summary: string;
  /** ファイルへの変更。適用の順は並び順に頼らず、確かめる（expect）→ ゴミ箱へ移す → 移動 → 書く。 */
  files: FileChange[];
  /** この変更案を作った作品フォルダ。別の作品を開き直したあとに適用すると拒否される。 */
  project_root: string;
}

// ---- 構成の操作（人物・世界観の資料・章・シーンの追加・削除と、人物・章・シーンの並べ替え） ----

/** 足すシーンの設計。ScenePlan から、足すときに決まる項目（id・ビート）を除いたもの。 */
export interface NewScenePlan {
  title: string;
  summary: string;
  pov: string | null;
  characters: string[];
  place: string | null;
  time: string | null;
  target_chars: number | null;
}

/** 構成に対する 1 つの操作。 */
export type StructureEdit =
  | {
      kind: "add_character";
      /** ID（ファイル名）。null なら、読み（無ければ名前）からローマ字で決める。 */
      id: string | null;
      meta: CharacterMeta;
      body: string;
    }
  | {
      kind: "remove_character";
      /** 人物資料のパス（`characters/` 直下の `.md`）。ファイル名が ID の規則に合わない資料も、目次に出ていれば消せる。 */
      path: string;
    }
  | {
      kind: "add_world_document";
      /** ファイル名（英小文字・数字・ハイフン。拡張子なし）。null なら題から決める。 */
      name: string | null;
      title: string;
      body: string;
    }
  | { kind: "remove_world_document"; path: string }
  | {
      kind: "add_chapter";
      /** この章の前に足す。null なら末尾。 */
      before: string | null;
      title: string;
      /** ストーリーライン（章立ての本文）。空でもよい。 */
      storyline: string;
    }
  | { kind: "remove_chapter"; chapter: string }
  | {
      kind: "add_scene";
      chapter: string;
      /** このシーンの前に足す。null なら章の末尾。 */
      before: string | null;
      scene: NewScenePlan;
    }
  | { kind: "remove_scene"; chapter: string; scene: string }
  | {
      kind: "move_character";
      /** 動かす人物資料。`characters/` 直下の Markdown で、YAML が読めるもの。 */
      path: string;
      /** 並べ替えたあとに、目次の人物の何番目に来るか（0 始まり）。範囲外・今と同じ位置は `invalid_input`。 */
      position: number;
    }
  | {
      kind: "move_chapter";
      chapter: string;
      /** 並べ替えたあとに、章の何番目に来るか（0 始まり）。範囲外・今と同じ位置は `invalid_input`。 */
      position: number;
    }
  | {
      kind: "move_scene";
      chapter: string;
      scene: string;
      /** 並べ替えたあとに、章のシーンの何番目に来るか（0 始まり）。範囲外・今と同じ位置は `invalid_input`。 */
      position: number;
    };

/** 人物の名前を挙げているシーン。 */
export interface SceneReference {
  chapter: string;
  chapter_title: string;
  scene: string;
  scene_title: string;
  /** 視点人物として挙げている。 */
  as_pov: boolean;
  /** 登場人物として挙げている。 */
  as_character: boolean;
}

/** 番号が変わる章。 */
export interface RenumberedChapter {
  from: string;
  to: string;
  /** 章題。章立てが読めなければ null。 */
  title: string | null;
}

/** 構成の操作の変更案と、利用者に見せる材料。 */
export interface StructurePlan {
  /** 作る変更案。summary は適用する前に見せる説明（「人物「霧島 凛」を追加します。」）。 */
  change_set: ChangeSet;
  /** 適用したあとに利用者へ知らせる文（「人物「霧島 凛」を追加しました。」）。 */
  completed_summary: string;
  /** 適用したあとに開く文書。何も開かなければ null。 */
  created: string | null;
  /** 人物を消すとき、その人物の名前を挙げているシーン。 */
  references: SceneReference[];
  /** 章を足す・消す・並べ替えるときに、番号が変わる章（後ろの章。並べ替えでは動く範囲の章）。番号の小さい順。 */
  renumbered: RenumberedChapter[];
  /** 利用者への注意書き。 */
  notices: string[];
}
