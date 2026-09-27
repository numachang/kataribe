# kataribe 設計書

## 1. 目的と方針

- 日本語の小説（純文学・SF・ミステリ・ライトノベル・美少女ノベル・BL・官能など）を、LLM と一緒に書く。
- 作品は **テキストファイルだけで構成されたフォルダ**（＝プロジェクト）として管理し、Git でそのまま扱える。
- 設定資料（企画・文体・世界観・登場人物・あらすじ・章立て・シーン構成）は、まず LLM に出力させ、
  人が直接編集するか、LLM に指示して書き直させる。
- 本文は **ローカル LLM でも精度が落ちない単位** に分けて生成する。単位と 1 回あたりの分量は設定で変えられる。
- GUI と CLI（ヘッドレス）は同じ執筆エンジン（`kataribe-engine`）を使う。GUI でできる生成はすべて CLI でもできる。

## 2. 作品フォルダの形式（format: 1）

```
<作品フォルダ>/
├─ kataribe.yaml             作品情報（題名・ジャンル・年齢区分・目標文字数・企画の種）
├─ concept.md                企画
├─ style.md                  文体ガイド（文体見本を含む）
├─ world/
│   ├─ overview.md           世界観の概要
│   └─ *.md                  用語集など、自由に追加してよい
├─ characters/
│   └─ <id>.md               登場人物（1 人 1 ファイル。id はローマ字の slug）
├─ plot/
│   ├─ synopsis.md           全体あらすじ
│   └─ chapters/<NN>.md      章のストーリーライン＋シーン構成（NN は 01, 02, …）
├─ manuscript/
│   └─ <NN>/<scene-id>.txt   本文（シーンごと。scene-id は s01, s02, …）
├─ .gitignore                `.kataribe/` を除外
├─ .gitattributes            改行を LF に固定
└─ .kataribe/                アプリの内部データ（Git 管理外）
    ├─ backups/              上書き前のバックアップ
    ├─ trash/                削除したファイル
    └─ cache/                生成の中間データ（シーン要約など。消えても再生成できる）
```

- フォルダ名・ファイル名は英数字。**題名・章題・シーン題・人物名はすべて日本語で**ファイル内に書く。
- 設定資料は Markdown。先頭に YAML の front matter（`---` で囲む）を置けるのは次の 3 種類。
  それ以外の Markdown は front matter を省略してよい（`title` だけ書いてもよい）。
- 本文は **プレーンテキスト**。ルビは `|漢字《かんじ》` または `漢字《かんじ》`、傍点は `《《強調》》`
  （カクヨム・小説家になろう互換）。
- 文字コードは UTF-8（BOM なし）、改行は LF。読み込み時は BOM と CRLF を許容して正規化する。
- 章の順序はファイル名（`01`, `02`, …）の順。シーンの順序は章ファイルの `scenes` の並び順。
  シーン本文のファイル名はシーンの `id` なので、`scenes` を手で並べ替えても本文との対応は崩れない。
- 人が追加した未知の YAML 項目は、アプリが書き戻すときも保持する。

### kataribe.yaml

```yaml
format: 1
title: みさき館の殺人
author: 沼田              # 任意
genre: mystery            # ジャンルプリセットの id（§4.4）
genre_note: 館もの。本格   # 任意。ジャンルの補足
rating: general           # general（全年齢）| r15 | r18
target_length: 30000      # 目標総文字数
idea: |                   # 企画の種（最初に LLM へ渡す指示）
  嵐で孤立した岬の洋館で…
```

### characters/<id>.md

```markdown
---
name: 霧島 凛
reading: きりしま りん
role: 主人公
summary: 盲目の少女探偵。声の揺れで嘘を聞き分ける。
order: 1
---
## 外見
…
## 口調
一人称は「わたし」。…
```

### plot/chapters/<NN>.md

```markdown
---
title: 雨の匂い
scenes:
  - id: s01
    title: 事務所に届いた依頼
    summary: 雨の夜、凛の事務所に…
    pov: 霧島 凛
    characters: [霧島 凛, 佐藤 健二]
    place: 凛の探偵事務所
    time: 六月の雨の夜
    target_chars: 2000
    beats:                # ビート単位で生成したときだけ入る
      - 依頼人が現れる…
---
（この章のストーリーライン）
```

## 3. crate 構成と依存関係

```
kataribe-text ────┐
kataribe-llm  ────┼──> kataribe-engine ──> kataribe-cli
kataribe-project ─┘          │
                             └──────────> src-tauri（kataribe） ──IPC──> src（React）
```

各 crate の公開 API は以下のとおり（実装時に細部が変わったら、この文書も直す）。

### 3.1 kataribe-text

外部依存なしの純粋な関数群。

| モジュール | 公開 API | 内容 |
|---|---|---|
| `count` | `count_chars(&str) -> usize`、`stats(&str) -> TextStats` | 本文の文字数（ルビ記法の読み・空白・改行を除き、書記素単位で数える）、段落数、会話行数、400 字詰め原稿用紙換算枚数 |
| `ruby` | `parse(&str) -> Vec<Segment>`、`to_plain(&str) -> String` | `\|親《よみ》`・`｜親《よみ》`・`漢字《よみ》`・`《《傍点》》`・`\|《`（記法のエスケープ）を解析 |
| `tokens` | `estimate_tokens(&str) -> usize` | トークン数の保守的な見積もり（かな・漢字 1 文字 ≒ 1 トークン） |
| `normalize` | `normalize(&str, &NormalizeOptions) -> String` | 段落頭の全角字下げ、`…`/`...`→`……`、`--`→`――`、`!?`→`！？` と後続の全角空白、空行の整理、行末空白の除去 |
| `quality` | `analyze(&str, &QualityOptions) -> QualityReport` | 品質指標と問題点の検出（§7） |

`TextStats`・`Segment`・`QualityReport` などは `serde::Serialize` と（`ts` feature で）`ts_rs::TS` を実装する。

### 3.2 kataribe-llm

OpenAI 互換 Chat Completions API（LM Studio / Ollama / KoboldCpp / OpenRouter / OpenAI など）への
ストリーミングクライアント。

```rust
pub struct ClientConfig { base_url, api_key: Option<String>, model, connect_timeout, idle_timeout, max_retries }
pub struct OpenAiCompatClient;               // new(ClientConfig) -> Result<Self, LlmError>
impl OpenAiCompatClient { async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> }

pub trait ChatModel: Send + Sync + Debug {
    fn stream_chat(&self, request: ChatRequest) -> ChatStream;
}
pub type ChatStream = BoxStream<'static, Result<ChatEvent, LlmError>>;
pub enum ChatEvent { Content(String), Reasoning(String), Finished(Finish) }

pub async fn collect(stream, on_event) -> Result<Completion, LlmError>;   // 全文を集める補助
```

- `ChatRequest` は `messages`・`max_tokens`・サンプリング（temperature / top_p / frequency_penalty /
  presence_penalty / seed / stop）・`response_format`（JSON Schema）・`extra`（サーバー固有の追加パラメータ）を持つ。
- 推論モデルの思考は `delta.reasoning_content` / `delta.reasoning` / 本文中の `<think>…</think>` のいずれでも
  `ChatEvent::Reasoning` に振り分け、本文（`Content`）に混ぜない。
- 接続失敗・5xx・429 は、正常な応答（2xx）を受け取る前に限り指数バックオフで再試行する。応答を読み始めた後に切れた場合は再試行しない。
- 応答ヘッダーを待つ間と、ストリームの次のチャンクを待つ間の両方に `idle_timeout` を適用する。
- ストリームを drop すると HTTP 接続も閉じる（＝キャンセル）。
- `testing` feature で、台本どおりに応答し受け取ったリクエストを記録する `ScriptedChatModel` を提供する。

### 3.3 kataribe-project

作品フォルダの読み書き。**原稿を失わないこと** を最優先にする。

| モジュール | 公開 API | 内容 |
|---|---|---|
| `path` | `RelPath` | 作品フォルダ内の相対パス。`..`・絶対パス・ドライブ指定・`\`・Windows 予約名・末尾のドット／空白・制御文字を拒否する |
| `store` | `ProjectStore`、`TextFile { content, hash }`、`ContentHash`、`WriteCondition`、`BackupMode` | フォルダ外に出られないファイル操作。アトミック書き込み、競合検出、バックアップ、ゴミ箱 |
| `frontmatter` | `Document<M> { meta, body }`、`parse`、`render` | YAML front matter の分解・合成（未知の項目を保持） |
| `layout` | パス定数と `character_path(id)` などの関数 | §2 のフォルダ構成の唯一の定義 |
| `model` | `Manifest`・`Rating`・`MarkdownDoc`・`Character`/`CharacterMeta`・`Chapter`/`ChapterMeta`・`ScenePlan`・`ChapterId`・`SceneId`・`CharacterId` | 各ファイルの型。`render()` でファイル内容を生成 |
| `project` | `Project` | 作品の作成・読み込み・型付きの取得 |

- `ProjectStore::write_text(path, content, WriteOptions { condition, backup })`
  - `WriteCondition::{Any, Absent, Matches(ContentHash)}`。条件に合わなければ `Conflict` エラー（外部で変更された可能性）。
  - 同じフォルダの一時ファイルに書いて fsync してから置き換える。Windows で一時的にロックされていたら短く再試行する。
  - `BackupMode::Throttled`（同じファイルは 10 分に 1 回まで）/ `Always`（LLM による置き換え時）/ `Never`。
    バックアップは `.kataribe/backups/<相対パス>/<日時>.<拡張子>`、1 ファイルあたり最新 20 件を残す。
- `ProjectStore::write_all(&[PendingWrite { path, content, condition }], backup)` は複数のファイルを
  **すべて書くか、何も書かないか** のどちらかで書く。先にすべての条件を確かめ、全ファイルを一時ファイルに書いてから
  順に置き換える。置き換えの途中で失敗したら書き終えたファイルを元に戻し、戻せなければ `PartialWrite` を返す。
- 「条件の確認から置き換えまで」は同じプロセスの中で排他する（自動保存と変更案の適用が重なっても、
  両方が同じ内容を前提に通って片方の変更が消えることがないように）。
- `remove` は削除せず `.kataribe/trash/<日時>/<相対パス>` へ移す。
- 作品全体の一覧（文字数や生成工程の状態を含む `ProjectOverview`）は engine が組み立てる。
- `Project` はキャッシュを持たず、毎回ファイルを読む（外部エディタや `git checkout` による変更を常に反映するため）。

## 4. 執筆エンジン（kataribe-engine）

### 4.1 生成タスク

| タスク | 入力（主なもの） | 出力 |
|---|---|---|
| `Concept` | 作品情報・企画の種 | `concept.md` |
| `Style` | 作品情報・企画 | `style.md`（文体見本を含む） |
| `World` | 作品情報・企画 | `world/overview.md` |
| `Cast` | 企画・世界観 | 登場人物の一覧（JSON Schema）→ `characters/<id>.md` の骨組み |
| `Character { id }` | 企画・世界観・人物一覧 | その人物の詳細（本文） |
| `Synopsis` | 企画・世界観・人物 | `plot/synopsis.md` |
| `Outline` | あらすじほか | 章の一覧（JSON Schema）→ `plot/chapters/<NN>.md` |
| `ScenePlan { chapter }` | 章のストーリーライン・前後の章・人物 | その章の `scenes`（JSON Schema） |
| `Draft { chapter, scene }` | §4.3 の文脈 | `manuscript/<NN>/<scene-id>.txt` |
| `Revise { path, instruction }` | 対象ファイル・指示 | 書き直した同じファイル |

どのタスクも **ファイルを直接書き換えず**、変更案 `ChangeSet { summary, files: Vec<FileChange>, project_root }` を返す。
GUI は変更案を見せてから適用し、CLI は自動で適用する。

- 適用（`ChangeSet::apply`）は `write_all` を使い、すべてのファイルを書くか、何も書かないかのどちらかにする。
  各ファイルは生成を始めたときの内容のハッシュを条件にするので、その後に利用者が編集していれば `Conflict` になる。
- `project_root` は生成元の作品フォルダ（正規化した絶対パス）。生成中に別の作品を開き直しても、
  前の作品の変更案を今の作品に書き込まないよう、適用時に照合する。

### 4.2 本文の生成単位（設定で切り替え）

| 単位 | 1 回の生成 | 向いている環境 |
|---|---|---|
| `chapter` | 指定したシーンから、次に本文のあるシーンの手前まで（シーン区切りで分割して保存）。手で書いたシーンは置き換えない | 長い文脈に強い大きなモデル |
| `scene` | 1 シーン。`chars_per_call` を超えるシーンは複数回に分けて書き継ぐ | 中規模モデル（既定） |
| `beat` | シーンを展開（ビート）に分け、1 ビートずつ書く | 小さなモデル。設定から外れにくい |

`chars_per_call`（1 回あたりの目安文字数）と `context_tokens`（モデルに渡せる文脈の長さ）も設定で変えられる。

### 4.2.1 推論モデルの思考を止める

ローカルの推論モデル（Qwen 系・Gemma 系など）は、何も指定しないと出力の上限まで「思考」して本文が空になることがある。
`disable_thinking`（既定 true）のとき、次の 2 つを同時に指定する。LM Studio 0.4 で実測したところ、
Gemma には前者だけ、Qwen には後者だけが効き、両方を指定すると両方で安定した。

- リクエストに `reasoning_effort: "none"`
- 最後のメッセージとして、応答の書き出しに空の思考ブロック（`<think>

</think>

`）を置く

クラウドの API ではこれがエラーになりうるので、その場合は設定で切る。

### 4.3 文脈の組み立て

本文生成では、`context_tokens` から出力分と余白を引いた予算の中に、優先度の高い順に詰める。
入りきらない低優先の資料は要約・切り詰め・省略する。

1. 役割と規則（ジャンル・年齢区分・出力形式）、文体ガイド
2. このシーンの設計（あらすじ・視点・登場人物・場所・時間、ビート単位なら今回のビート）
3. 直前の本文の末尾（同じシーンの書きかけ、または前のシーン）
4. 登場する人物の資料（視点人物を最優先）
5. この章のストーリーラインと、前後のシーンの予定
6. ここまでの要約（直近ほど詳しく）
7. 世界観、企画

### 4.4 プロンプト

- プロンプトはコードに埋め込まず、`crates/kataribe-engine/prompts/*.j2`（minijinja）に置く。
- ジャンル・年齢区分ごとの書き方の指針は `crates/kataribe-engine/presets/genres.yaml` に置く。
- 年齢区分 `r18` でも、性的な場面の登場人物は全員成人として描く規則をプロンプトに必ず含める。

## 5. アプリ（src-tauri）と画面（src）

- Rust 側は engine を Tauri コマンドとして公開するだけの薄い層にする。
- ストリーミング出力は `tauri::ipc::Channel` で画面へ送る。キャンセルは画面が採番したジョブ ID で行う。
- API キーは Windows 資格情報マネージャーに保存し、画面側には渡さない。
- 画面の型は ts-rs が Rust の型から生成する `src/bindings/` を使う。手書きの契約（`src/api/types.ts`・`backend.ts`）を
  残す場合は、`src/api/bindingsContract.ts` で生成物と完全に一致することを型検査で保証する。
  生成物が Rust の型と一致していることは、CI で `cargo test --all-features` の後に `src/bindings` に差分が無いことで確かめる。
- ブラウザ単体（`pnpm dev`）では、メモリ上の偽バックエンドで動く（画面の開発とテスト用）。
- 起動オプション `--settings=<PATH>`（`--settings <PATH>` も可）で、既定の場所の代わりに使う設定ファイルを指定できる
  （CLI の同名のオプションと同じ意味）。E2E テストが利用者の設定に触れずに動くためにも使う。

### 5.1 IPC の契約

画面側のインターフェースは [src/api/backend.ts](../src/api/backend.ts)、型は [src/api/types.ts](../src/api/types.ts)。
Tauri コマンド名と引数（JS 側の名前。Rust 側は snake_case で受ける）は次のとおり。

| Backend のメソッド | コマンド | 引数 |
|---|---|---|
| loadSettings / saveSettings | `load_settings` / `save_settings` | — / `settings` |
| setApiKey / hasApiKey | `set_api_key` / `has_api_key` | `apiKey` / — |
| listModels / listGenres | `list_models` / `list_genres` | `llm`（省略可。保存前の接続先で試す）/ — |
| createProject / openProject / closeProject | `create_project` / `open_project` / `close_project` | `folder, project` / `folder` / — |
| overview / pipeline | `overview` / `pipeline` | — |
| readFile / writeFile | `read_file` / `write_file` | `path` / `path, content, expectedHash` |
| textStats / parseRuby / analyzeQuality | `text_stats` / `parse_ruby` / `analyze_quality` | `text` / `text` / `text, targetChars` |
| generate / cancelGeneration | `generate` / `cancel_generation` | `jobId, task, onEvent`（`Channel<GenerationEvent>`）/ `jobId` |
| applyChangeSet | `apply_change_set` | `changeSet` |

コマンドの失敗は `{ kind: BackendErrorKind, message: string }` で返り、画面側で `BackendError` に変換する。
作品を開いていない状態で作品の操作を呼んだときは `not_found`。
フォルダ選択は `@tauri-apps/plugin-dialog` の `open({ directory: true })` を画面側から直接呼ぶ。

### 5.2 E2E テスト

本物のアプリ（WebView2 と Rust のバックエンド）を tauri-driver（WebDriver）で操作する。`e2e/` に置き、`pnpm e2e` で動かす。
一時フォルダに作品と設定ファイルを作り、`--settings=<PATH>` を付けて起動するので、利用者の設定や作品には触れない。
msedgedriver は引数を Chromium のスイッチとして扱い、値だけの引数に `--` を付けて並べ替えるため、
アプリへの引数は必ず `--name=value` の 1 つの引数で渡す。

## 6. ヘッドレス実行（kataribe-cli）

GUI と同じ engine を使い、画面なしで作品を作る・生成する・検査する。GUI と同じ設定ファイルを読み、
コマンドラインの指定で上書きできる。`main.rs` は薄くし、処理は `lib.rs` の `run` に置いてテストできるようにする。

```
kataribe-cli [グローバルオプション] <サブコマンド>

グローバルオプション（設定ファイルの値を上書き）
  --settings <PATH>          設定ファイル（既定: GUI と同じ場所。明示したのに無ければエラー）
  --base-url <URL>  --model <ID>
  --api-key-env <VAR>        API キーを読む環境変数（既定 KATARIBE_API_KEY。未設定なら資格情報マネージャーのキーを使う）
  --unit <chapter|scene|beat>  --chars-per-call <N>  --context-tokens <N>
  --temperature <T>  --polish  --quality-retries <N>
  -q, --quiet                生成中の本文を表示しない

サブコマンド
  new <FOLDER> --title <T> [--author <A>] [--genre <ID>] [--genre-note <T>]
               [--rating general|r15|r18] [--length <N>] (--idea <TEXT> | --idea-file <PATH>)
      --length は省略でき、既定は 30,000 字（GUI の新規作成と同じ値）
  status <FOLDER> [--json]            工程の状態と文字数
  generate <FOLDER> <TASK> [--instruction <TEXT>] [--dry-run]
      TASK = concept | style | world | cast | character:<id> | synopsis | outline
           | scenes:<NN> | draft:<NN>/<sNN> | revise:<path>
      --instruction は revise:<path> のときだけ必須。それ以外に付けると使い方の誤りにする
      --dry-run は原稿と資料を書き換えない（要約などの中間データのキャッシュは更新する）
  run <FOLDER> [--until <STAGE>] [--max-steps <N>]
      取りかかれる工程（ready）を順に生成・適用し続ける。ready が無くなるか上限で止まる。
      --until があるときは、それより後ろの段階の工程を候補にしない
      （候補が無くなり、かつ完了していない工程が残っていれば行き詰まりとして失敗にする）。
      STAGE = concept | style | world | cast | characters | synopsis | outline | scenes | draft
  quality <FOLDER> [--json]           シーンごとの品質レポート
  export <FOLDER> [--output <FILE>] [--force]
      本文を章題付きの一つのテキストにまとめる。--output は作品フォルダの外を指定すること
      （中を指すと拒否する）。既存ファイルへの上書きは --force を指定したときだけ許す
  models                              LLM サーバーのモデル一覧
  api-key set | clear | status        API キーを資格情報マネージャーに保存・削除・確認
                                      （set は標準入力から読む。端末から直接入力すると
                                       画面にそのまま表示されるので、表示したくなければ
                                       パイプで渡す）
```

- API キーは `--api-key-env` の環境変数 → 資格情報マネージャー（GUI と共有、`kataribe_engine::ApiKeyStore`）の順に探す。

- 生成中の本文は標準出力、進捗・注意・エラーは標準エラー出力に出す。工程が切り替わるときは
  標準出力側にも区切りの空行を入れる。`--dry-run` のときは生成そのものを流さず、最後に
  変更案だけをまとめて出す。
- Ctrl+C で実行中のサブコマンドを中止する（キャンセルトークン）。もう一度 Ctrl+C を受けたら、
  応答しなくなった処理を待たずにその場で終了する。
- 標準出力への書き込みに失敗したら（ディスクが一杯など）、少なくとも `status` / `quality` /
  `export` の結果を出す箇所では失敗として扱う。
- 変更案の適用が競合などで失敗したときは、生成した内容を失わないよう標準出力に表示してから
  エラーで終わる。
- 終了コード: 0 成功、1 失敗、2 使い方の誤り、130 中止。

## 7. 品質チェック

本文生成のたびに `kataribe_text::quality::analyze` で機械的に検査し、重大な問題があれば再生成する（回数は設定）。

| 検出するもの | 例 |
|---|---|
| メタ発言・Markdown の混入 | 「以下は本文です」「承知しました」「# 第一章」「**」 |
| 外国語の混入 | 簡体字、ハングル、長い英文 |
| 同じ文・言い回しの反復 | 同一文の繰り返し、長い n-gram の多用 |
| 文末の単調さ | 「〜た。」が何文も連続 |
| 分量の過不足 | 目標の 7 割未満、1.5 倍超 |
| 括弧の不整合 | 「 と 」の数の不一致 |
