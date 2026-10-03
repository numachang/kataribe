# kataribe 設計書

## 1. 目的と方針

- 日本語の小説（純文学・SF・ミステリ・ライトノベル・美少女ノベル・BL・官能など）を、LLM と一緒に書く。
- 作品は **テキストファイルだけで構成されたフォルダ**（＝プロジェクト）として管理し、Git でそのまま扱える。
- 設定資料（企画・文体・世界観・登場人物・あらすじ・章立て・シーン構成）は、まず LLM に出力させ、
  人が直接編集するか、LLM に指示して書き直させる。
- 本文は **ローカル LLM でも精度が落ちない単位** に分けて生成する。単位と 1 回あたりの分量は設定で変えられる。
- GUI と CLI（ヘッドレス）は同じ執筆エンジン（`kataribe-engine`）を使う。GUI でできる生成はすべて CLI でもできる。
- LLM は OpenAI 互換 API（ローカル LLM・クラウド）のほか、Claude Code（`claude -p`）でも動かせる（§4.5）。

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
    ├─ trash/                削除したファイル・フォルダ
    ├─ staging/              反映の途中の置き場（一時ファイル・改名の途中で預けるもの・操作の記録。通常は空）
    └─ cache/                生成の中間データ（シーン要約など。消えても再生成できる）
```

- フォルダ名・ファイル名は英数字。**題名・章題・シーン題・人物名はすべて日本語で**ファイル内に書く。
- 設定資料は Markdown。先頭に YAML の front matter（`---` で囲む）を置けるのは次の 3 種類。
  それ以外の Markdown は front matter を省略してよい（`title` だけ書いてもよい）。
- 本文は **プレーンテキスト**。ルビは `|漢字《かんじ》` または `漢字《かんじ》`、傍点は `《《強調》》`
  （カクヨム・小説家になろう互換）。
- 文字コードは UTF-8（BOM なし）、改行は LF。読み込み時は BOM と CRLF を許容して正規化する。
- 章の順序はファイル名（`01`, `02`, …）の順。章を途中に足す・消すときは、後ろの章の番号を振り直す
  （`plot/chapters/<NN>.md` と `manuscript/<NN>/` を改名する。ファイル名＝順番のまま。§4.8）。
  章を並べ替えるときも、動く範囲の章の番号を割り当て直す（同じ改名）。
  シーンの順序は章ファイルの `scenes` の並び順。人物の順序は front matter の `order`（小さい順。無ければ最後）。
  シーンと人物は、並べ替えても本文のファイル名（シーンの `id`）や人物資料のファイル名は変わらない（§4.8）。
  シーン本文のファイル名はシーンの `id` なので、`scenes` を手で並べ替えても本文との対応は崩れない。
- 人が追加した未知の YAML 項目は、アプリが書き戻すときも保持する。画面から人物資料・章立てを保存するときは、
  項目（front matter）に変更がなく本文だけが変わったなら、YAML を解釈し直さず書かれたまま
  （コメント・項目の順番・引用符やブロック表記も）残して本文だけを差し替える。項目が変わったときは YAML を書き直す
  （未知の項目は残るが、コメントと順番は残らない）。
- 章立ての `scenes` で `id` は一意でなければならない。重複した章立て（手で複製して直し忘れたなど）は、
  画面では項目に分けず文字列のまま開き（理由を添える）、項目に分けた保存は受け付けない（§3.3）。
- 人物・世界観の資料・章・シーンは、利用者が自分で書いて追加・削除できる（§4.8）。追加するときの名前は次のように決める。
  - 人物の `id`（`characters/<id>.md` の名前）は、読み（かな）をローマ字にして作る
    （`きりしま りん` → `kirishima-rin`。変換の規則は §3.1）。読みが無ければ名前から、それでも作れなければ `character`。
    使用済みなら `-2`, `-3`, … を付ける。利用者が自分で決めてもよく、そのときは規則（下の世界観の資料と同じ）に合わなければ
    `InvalidInput` と理由（「ID「Rin」は使えません。小文字の英数字とハイフンで、…」）を返す。空白だけなら自動で決める。
    使用済みかどうかは、ファイル名を小文字にして数える（Windows は大文字小文字を区別しないので、`characters/Kirishima-Rin.md` が
    あれば `kirishima-rin` は使用済み。世界観の資料の名前と、シーンの id も同じ）。
  - 世界観の資料のファイル名（`world/<name>.md` の `name`）は、人物の `id` と同じ規則（小文字の英数字とハイフン、48 文字以内）。
    `overview` は世界観の概要（`world/overview.md`）の名前なので使えない。自動で決めるときは、題が全部かな・英数字・区切りで
    書けているときだけローマ字にし、それ以外（漢字を含む題）は `doc`、重なれば `doc-2`, `doc-3`, …にする。
    手で足したファイルは、この規則に合わない名前（日本語など）でもよい。
  - 世界観の資料は front matter を持たず文字列のまま開く文書なので、題は本文の先頭の見出し（`# 題`）にする
    （生成した資料と同じ形で、目次の表示名は見出しから拾われる）。
- 削除は完全には消さず、ゴミ箱 `.kataribe/trash/<日時>/<元の相対パス>` へ移す。1 回の適用につき `<日時>` のフォルダは 1 つで
  （同じ日時があれば `-1`, `-2`, … を付ける）、同時に消した本文なども同じフォルダに入る。
  フォルダ（章を消すときの `manuscript/<NN>/`）は、中のファイルごと同じ `<日時>` のフォルダへ移る。
  ゴミ箱から戻す操作はまだ無い（手で戻せる）。ゴミ箱へ移すファイルはバックアップを取らないので、実物は `.kataribe/trash/` にしかない。

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
settings:                 # 任意。作品ごとの設定（§4.6）。書いた項目だけ、アプリ全体の設定を上書きする
  provider: claude_code
  claude_model: haiku
  context_tokens: 100000
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
| `repetition` | `overused_phrases(&str, &OverusedOptions) -> Vec<RepeatedPhrase>` | 繰り返し使っている表現の検出（漢字かカタカナを含む 3〜10 字。人物名は除く） |
| `romaji` | `to_romaji(&str) -> String`、`is_romanizable(&str) -> bool` | かな → ローマ字（ヘボン式の簡略形。LLM が作る id の `sato-kenji` と書き方をそろえる）。人物の id や世界観の資料のファイル名を、読みから自動で作るために使う（§2）。規則は下の箇条書き |

`romaji` の規則:

- カタカナはひらがなに直してから変換する（ヴは `vu`）。し `shi`・ち `chi`・つ `tsu`・ふ `fu`・じぢ `ji`・づ `zu`・を `o`。
- 拗音は `kya`・`sha`・`cha`・`ja`、外来音は `fa`・`ti`・`di`・`tu`・`wi`・`we`・`va`・`she`・`je`・`che`。
  単独の小さい母音はその母音。
- 促音は次の子音を重ねる（`っか` → `kka`、`っち` → `tchi`）。語の末尾や、母音・ん・区切りの前では落とす。
- ん は常に `n`（`b`・`m`・`p` の前でも `m` にしない。アポストロフィも付けない）。`しんいち` → `shinichi`。
- 長音の「ー」「〜」は落とす。同じ語の中の `ou`・`oo` は `o`、`uu` は `u` にまとめる
  （`さとう` → `sato`、`おおの` → `ono`、`ゆうこ` → `yuko`）。`ei`・`ii`・`aa` はそのまま。
- 英数字はそのまま使い、全角は半角にする。空白（半角・全角）・「・」「＝」「=」「‐」は語の区切り（半角の空白 1 つ）にする。
  漢字・記号・絵文字は区切りとして落とす。
- `is_romanizable` は、全体がかな・英数字・区切りだけで書かれているか。漢字を含む題をローマ字にすると漢字が落ちて別の語に
  なるので、ローマ字を名前に使ってよいかの判断に使う。

`TextStats`・`Segment`・`QualityReport` などは `serde::Serialize` と（`ts` feature で）`ts_rs::TS` を実装する。

### 3.2 kataribe-llm

LLM を呼ぶ層。OpenAI 互換 Chat Completions API（LM Studio / Ollama / KoboldCpp / OpenRouter / OpenAI など）への
ストリーミングクライアントと、Claude Code の `claude -p` を使うモデル（§3.2.1）を持つ。

```rust
pub struct ClientConfig { base_url, api_key: Option<String>, model, connect_timeout, idle_timeout, max_retries }
pub struct OpenAiCompatClient;               // new(ClientConfig) -> Result<Self, LlmError>
impl OpenAiCompatClient { async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> }

pub struct ClaudeCodeConfig { program, model, idle_timeout }  // 既定は "claude"・"sonnet"・5 分
pub struct ClaudeCodeModel;                  // new(ClaudeCodeConfig)
impl ClaudeCodeModel { async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> }

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

#### 3.2.1 Claude Code（`claude -p`）

- 呼び出しごとに、空の作業フォルダ（一時フォルダの下の `kataribe-claude`）で次のように起動する。ツール・MCP サーバー・
  利用者やプロジェクトの設定を読み込ませず、会話の履歴も残さない（小説の文章を書かせるだけで、ファイルを読み書きさせないため）。
  作業フォルダは `claude` が動いている間は消せない（Windows）ので、呼び出しごとに作って消さずに使い回す。

  ```
  claude -p --output-format stream-json --verbose --include-partial-messages --no-session-persistence
         --tools "" --strict-mcp-config --setting-sources "" --model <モデル>
         --system-prompt-file <一時フォルダ内のファイル> [--json-schema <スキーマ>]
  ```

- `system` のメッセージはシステムプロンプトのファイル（Claude Code 既定のシステムプロンプトを置き換える）、
  `user` のメッセージは標準入力で渡す。`assistant` のメッセージ（思考を止めるための書き出しの指定）は渡す手段がないので使わない。
  temperature などのサンプリング・`max_tokens`・`extra` も同じ理由で使わない。
- 出力の `text_delta` を `Content`、`thinking_delta` を `Reasoning` にし、最後の `result` 行で `Finished`
  （`stop_reason` と使用トークン数。入力にはプロンプトキャッシュの分も足す）にする。
- 差分をつなげたものは `result` 行の全文と突き合わせる。差分が届かなければ全文を本文にし、食い違えば
  （途中でやり直されたなど）重複や欠けのある本文を原稿に入れないようエラーにする（実測では常に一致する）。
- 次の場合はエラーにする。`is_error` の結果、または `subtype` が `error` で始まる結果（説明は `result` → `errors` →
  `subtype` の順に探す）。`result` 行を出さずに終わったとき（エラー出力と終了コードを添える）。`idle_timeout` の間なにも出力がないとき。
- JSON Schema を指定したときは、前置きの文章を捨て、`result` 行の `structured_output` だけを本文にする。
  スキーマからは `$schema` を外して渡す（Claude Code の検証器は draft 2020-12 を名乗るスキーマを拒むため）。
- `--bare` は使わない。付けるとログイン（OAuth）を読まなくなり、API キーが必須になる。
- ストリームを drop するとプロセスも止める（中止）。`claude.cmd`（npm 版）を指定したときに止まるのは間の `cmd.exe` で、
  その下の `claude` は出力先が閉じたことで数秒のうちに自分で終わる（実測 6 秒以内）。
  正常に終わったときは、`claude` が自分で終わるのを裏で待ち（10 秒まで。過ぎたら止める）、利用者は待たせない。
  待っている間にアプリや CLI が終わるときは、待たずに止める（結果はもう受け取っているので失うものはない）。
- GUI から起動してもコンソールの窓を出さない（`CREATE_NO_WINDOW`）。
- `list_models` は `claude auth status --json` でログインしているかを確かめ（利用枠を使わない）、モデルの別名
  （`sonnet` / `opus` / `haiku`）を返す。GUI の接続テストと CLI の `models` がこれを使う。

### 3.3 kataribe-project

作品フォルダの読み書き。**原稿を失わないこと** を最優先にする。

| モジュール | 公開 API | 内容 |
|---|---|---|
| `path` | `RelPath` | 作品フォルダ内の相対パス。`..`・絶対パス・ドライブ指定・`\`・Windows 予約名・末尾のドット／空白・制御文字を拒否する |
| `store` | `ProjectStore`、`TextFile { content, hash }`、`ContentHash`、`WriteCondition`、`BackupMode`、`PendingChange`、`PendingWrite`、`EntryCondition`、`FolderFile { path, text }`、`normalize_text(&str) -> String` | フォルダ外に出られないファイル操作。アトミック書き込み、競合検出、バックアップ、ゴミ箱、改名。基本の読み書きは `store/mod.rs`、フォルダの中身の一覧（`read_folder`）は `store/folder.rs`、複数の変更を「全部か無しか」で反映する処理は `store/batch/`（形の検証 `shape`・パスの対応 `namespace`・条件の確認 `prepare`・反映 `commit`・巻き戻し `applied`・置き場 `staging`・操作の記録 `journal`） |
| `frontmatter` | `Document<M> { meta, body }`、`parse`、`render`、`replace_body(text, body)` | YAML front matter の分解・合成（未知の項目を保持）。`replace_body` は front matter を書かれたまま残して本文だけを差し替える |
| `layout` | パス定数（`STAGING_DIR` を含む）と `character_path(id)`・`world_document_path(name)`・`manuscript_chapter_dir(chapter)`・`is_character_document(path)`・`is_additional_world_document(path)` などの関数、`document_kind(&RelPath) -> DocumentKind` | §2 のフォルダ構成の唯一の定義。`document_kind` は `characters/<有効な id>.md` を人物資料、`plot/chapters/<有効な NN>.md` を章立て、それ以外（サブフォルダの下・id として無効な名前・ほかのファイル）を「その他」と判定する。`Project::characters` / `chapters` が拾うファイルと同じ条件。`is_character_document` は `characters/` 直下の Markdown、`is_additional_world_document` は `world/` 直下の概要以外の Markdown で、どちらもファイル名の規則には照らさず（手で足した `Rin.md` や `凛.md` も消せるように）、拡張子と概要かどうかは大文字小文字を無視して調べる（Windows では `world/Overview.md` も概要と同じファイル） |
| `model` | `Manifest`・`Rating`・`MarkdownDoc`・`Character`/`CharacterMeta`・`Chapter`/`ChapterMeta`・`ScenePlan`・`ChapterId`・`SceneId`・`CharacterId`・`WorldDocumentName` | 各ファイルの型。`render()` でファイル内容を生成。`ChapterId::shifted(delta)` は番号をずらした id（`from_number` の規則で桁数を付け直す。0 未満・999 超は `None`）。`CharacterId` と `WorldDocumentName`（世界観の資料のファイル名。`overview` は不可）は slug の検証と、重なったときに番号を付ける処理を共有する |
| （crate 直下） | `EditableDocument`、`LoadedDocument { document, hash, parse_error }`、`ParsedDocument { document, parse_error }`、`parse_document(&RelPath, &str) -> ParsedDocument` | 画面で編集する文書と、読み込んだ結果。人物資料と章立ては front matter を項目に分け、それ以外は文字列のまま扱う。実装は非公開の `document` モジュールにあり、型と関数を `lib.rs` から公開している（`Project::read_document` / `write_document` から使う） |
| `project` | `Project` | 作品の作成・読み込み・型付きの取得。`update_manifest` で作品情報（`kataribe.yaml`）を書き換える。`read_document` / `write_document` で画面で編集する文書を読み書きする。`character_ids()`・`chapter_ids()`・`scene_text_ids(chapter)`・`world_document_names()` は、ファイル名だけから id の一覧を返す（中身は読まないので、YAML が壊れたファイルがあっても失敗しない。
`character_ids()`・`scene_text_ids(chapter)`・`world_document_names()` は、名前を小文字にしてから id として読み（`Kirishima-Rin.md` も `kirishima-rin` として数える）、使用済みの一覧として使う。`characters()` / `chapters()` は 1 つでも壊れていると全体が失敗するので、追加の前に使用済みの id を知るのには使えない） |

- `ProjectStore::write_text(path, content, WriteOptions { condition, backup })`
  - `WriteCondition::{Any, Absent, Matches(ContentHash)}`。条件に合わなければ `Conflict` エラー（外部で変更された可能性）。
  - 同じフォルダの一時ファイルに書いて fsync してから置き換える。Windows で一時的にロックされていたら短く再試行する。
  - `BackupMode::Throttled`（同じファイルは 10 分に 1 回まで）/ `Always`（LLM による置き換え時）/ `Never`。
    バックアップは `.kataribe/backups/<相対パス>/<日時>.<拡張子>`、1 ファイルあたり最新 20 件を残す。
- `ProjectStore::apply_changes(&[PendingChange], backup)` は複数の変更を **すべて反映するか、何も反映しないか** のどちらかで行う。
  `PendingChange` は次の 4 種類。
  - `Write(PendingWrite { path, content, condition })`: 新規作成・上書き。
  - `Trash { path, expected: EntryCondition }`: ゴミ箱へ移す。`EntryCondition` はファイルなら `File(今のハッシュ)`、
    フォルダなら `Folder(中のファイル全部（サブフォルダの下も含む）のパスとハッシュの一覧)`。
  - `Move { from, to }`: ファイルまたはフォルダの改名（章の番号の振り直し）。中身は変えない。
  - `Expect { path, expected: Option<ContentHash> }`: 何も書かずに、今の状態を確かめるだけ（`None` は「無いこと」）。
    計画のあとに外で状態が変わったら競合にするため。

  反映の順は並び順に頼らず「確認（Expect）→ ゴミ箱へ移す → 改名 → 書く」。
  - 先に形を検証する（`InvalidChangeSet`。`batch/shape.rs`）。ゴミ箱へ移せる・改名できるのは `.kataribe/` の外で、
    `kataribe.yaml` も不可（改名の行き先にもできない）。同じパス（大文字小文字の違いは同じとみなす）への変更や、
    フォルダとその中のパスへの変更は重ねられない。例外は、改名の元と先がつながる場合（`02 → 03` と `03 → 04`、入れ替え）、
    ゴミ箱へ移すパスが改名の先になる場合（章を消して後ろの章をつめる）、書き込みが改名の元・先（の下）と重なる場合
    （空いた場所に新しく書く、移した先のファイルを書き換える）、状態の確認が改名と重なる場合。
    フォルダを自分の中へ（や同じ場所へ）移す変更、改名の先がゴミ箱へ移す・改名するフォルダの中にある変更、
    ゴミ箱へ移すフォルダの下へ書く変更も断る。画面から戻ってくる値なので、ここで必ず検証する。
  - 次に条件を確かめる（`Conflict`）。Expect は今の状態（ハッシュ、または無いこと）。ゴミ箱へ移すのは、ファイルなら今のハッシュが
    `expected` と一致すること、フォルダなら中のファイルの一覧（パスとハッシュ）が計画のときと完全に一致すること
    （増えても変わっても競合。利用者が確かめた中身だけを移すため）。改名は、移動元があり、移動先が（同じ変更で
    ゴミ箱や改名によって空く場所を除いて）空いていること。中身のハッシュは見ない（移動では中身が失われず、
    中身まで条件にすると関係のない自動保存のたびに競合になるため）。書き込みは `condition` を、**改名した後の状態に対して**
    確かめる。書き先が改名の行き先なら移動元の今の中身（のハッシュ）、改名で空く場所なら「無いこと」として扱う
    （「03 を 04 へ移し、空いた 03 に新しい章を書く」を表すため）。
  - 反映は `.kataribe/staging/<日時>/` を使う。この置き場とゴミ箱の `<日時>` フォルダは、「無いことを確かめてから作る」のではなく
    フォルダの作成そのものの成否で確保する（同じ日時があれば `-1`, `-2`, … へ進む）。GUI と CLI が同じミリ秒に反映しても、
    同じ置き場を共有して互いの操作の記録や預け先を上書きしない。
    書き込む内容を一時ファイルとして置き（置き換え先と同じフォルダではなくここ。
    置き換え先のフォルダ自体が改名で動くことがあるため。同じボリュームなので移動は改名で済む）、バックアップを取り
    （書き込みが改名の行き先なら、移動元のパスの名前で残る）、ゴミ箱へ移す（1 回の呼び出しにつき `.kataribe/trash/<日時>/` は
    1 つ。フォルダは中身ごと）。改名は 2 段階で、まず移動元をすべて置き場（`staging/<日時>/m<n>`）へ預け、次にすべてを
    行き先へ置く（入れ替えのような循環も同じ処理で扱える）。最後に一時ファイルで置き換える。ゴミ箱や改名を伴うときは、
    何かを動かす前に、これから行う操作を `staging/<日時>/journal.json` に書く（途中でプロセスが落ちたときに、人が見て
    戻せるようにするため。自動では使わない）。終わったら置き場を片付ける（失敗しても無視。中に何かが残っていれば消さない）。
  - 途中で失敗したら、反映済みの変更（置き換え・ゴミ箱への移動・改名）を逆順に元へ戻し、戻せなければ `PartialWrite` を返す。
    `PartialWrite` は、書き込み前の内容に戻せず新しい内容のまま残ったファイル（`not_restored`。バックアップを取っていれば
    `.kataribe/backups` から戻せる）、ゴミ箱から元の場所へ戻せなかったもの（`still_trashed`。元の場所と、実物のある
    `.kataribe/trash/<日時>/…` の置き場所を持ち、メッセージにも出す）、改名の途中で戻せなかったもの（`still_moved`。
    元の場所と、実物のある場所＝預け先の `.kataribe/staging/<日時>/m<n>` か移動先）を分けて持ち、操作の記録（`journal`）の
    場所も知らせる。`PartialWrite` のときだけ、置き場と操作の記録を消さずに残す。
    1 つ目のゴミ箱への移動が失敗したときも、そのために作った `.kataribe/trash/<日時>/…` の空のフォルダを片付ける。
    取り消しの改名（預け先から元の場所、移動先から預け先、ゴミ箱から元の場所）と 2 段階目の改名は、行き先に何も無いことを
    確かめてから行う（Windows の改名は行き先がファイルだと黙って置き換えるため）。埋まっていれば動かさず、実物は預け先・
    移動先・ゴミ箱にあるものとして `still_moved`・`still_trashed` で知らせる。例えば、2 段階目の取り消しに失敗した改名の実物が
    移動先に残っているところへ、別の改名やゴミ箱の取り消しが同じ場所へ戻って、その中身を消すことはない。2 段階目の
    改名の行き先が埋まっていたときは `Conflict`（外で作られたということ）。確認と改名の間に外で作られる場合までは防げない
    （std には置き換えない改名が無い）。
    Windows でほかのアプリがファイルやフォルダの中のファイルを開いていて移せないとき（`is_transient_lock_error` に当たる改名の
    失敗）は、短く再試行してから、全部戻して `FilesInUse { path, restored }` で失敗にする（「ほかのアプリが開いている可能性が
    あります。閉じてからもう一度試してください。」。全部戻せたときは、そのことも添える。戻せなかったものがあれば `PartialWrite`）。
  - 戻り値は、書き込みごとの、書き込み後のハッシュ。
- `ProjectStore::read_folder(path) -> Option<Vec<FolderFile { path, text: Option<TextFile> }>>` は、フォルダの中のファイルを
  サブフォルダの下も含めてパスの順に全部読む（`.` で始まる名前も含む。テキストとして読めないファイルは `text` が `None`）。
  フォルダが無い（またはフォルダではない）なら `None`。フォルダごとゴミ箱へ移す前に、確かめた中身を記録するために使う。
- 「条件の確認から反映まで」は同じプロセスの中で排他する（自動保存と変更案の適用が重なっても、
  両方が同じ内容を前提に通って片方の変更が消えることがないように）。`write_text`・`apply_changes`・`rename`・`remove` がすべて同じ排他を取る。
  GUI と CLI を同時に使ったときは、プロセスをまたぐので守れない。
- ゴミ箱: `remove` と `apply_changes` は、削除せず `.kataribe/trash/<日時>/<元の相対パス>` へ移す
  （1 回の呼び出しにつき `<日時>` のフォルダは 1 つ。同じ日時があれば `-1`, `-2`, … を付ける。フォルダは作成の成否で確保する。§2）。
- `Project::read_document(path) -> LoadedDocument` は、人物資料・章立てのパスなら `EditableDocument::Character { meta, body }` /
  `Chapter { meta, body }`（章の `body` はストーリーライン）を返す。front matter を解釈できなければ、直して保存できるよう
  `Text { content }` のまま返し、理由（パスと行番号を含む）を `parse_error` に入れる。章立てのシーンの `id` が重複しているときも
  同じで（本文ファイルの名前が `id` なので、重複したまま項目に分けて保存すると別のシーンを上書きする）、
  「シーンの id「s01」が重複しているため…」という理由を `parse_error` に入れて `Text { content }` で返す。
  利用者が `id` を直して文字列のまま保存し、開き直せばフォームで開ける。それ以外のパスは `Text { content }`。
  `hash` は正規化したファイル全体のハッシュで、`read_text` と同じもの。
- `parse_document(path, content) -> ParsedDocument` は、ファイルを読まずに文字列を同じ分け方で項目に分ける
  （`read_document` の中身もこの関数）。`path` は種類（人物資料・章立て・それ以外）を決めるためだけに使い、
  ファイルの有無は問わない。解釈できない・シーンの `id` が重複しているときは `Text { content }` と理由、
  それ以外のパスは理由なしの `Text { content }`。生成した変更案のように、まだ書いていない内容を画面で見せるために公開している。
  `content` は BOM と CRLF を正規化済みのものを渡す。
- `Project::write_document(path, &EditableDocument, expected) -> ContentHash` の `expected` は `write_text` の条件に対応する
  （`Some` なら今のハッシュと一致するときだけ、`None` なら新規作成だけ。違えば `Conflict`）。バックアップは `Throttled`。
  - `Text` はパスの種類を問わずそのまま書く（YAML が壊れた人物資料を文字列のまま直せるように）。
  - `Chapter` の `scenes` に同じ `id` があれば `DuplicateSceneId`（何も書かない）。
  - `Character` / `Chapter` はパスの種類が合わなければ `DocumentKindMismatch`。保存されている今のファイルを読み、
    解釈できれば、画面が知らない項目（`extra`）は保存されている側の値を使う（章立てのシーンは `id` で突き合わせ、
    保存側に無い `id` は未知の項目なし。保存側に同じ `id` のシーンが複数あるときは最初のものを使う）。そのうえで項目が保存されているものと等しければ `replace_body` で本文だけを
    差し替え、等しくなければ `render` で書き直す。保存されているファイルが無い（新規作成）か解釈できないときは、
    画面から来た文書をそのまま `render` する。
  - 保存されているファイルを読んだ時点で `expected` と食い違っていれば、項目を引き継がずに `Conflict` にする。
    読んでから書くまでの間の変更は、`write_text` が排他の中で条件を確かめ直して検出する。
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
| `AddCharacter { instruction }` | 企画・世界観・既存の人物・あらすじの抜粋・指示 | 新しい人物 `characters/<id>.md`（項目と本文。LLM を 2 回呼ぶ） |
| `AddWorldDocument { name, instruction }` | 企画・世界観の概要と既存の資料・既存の資料の題・人物・指示 | 新しい世界観の資料 `world/<name>.md`（`name` が `None` なら題から決める） |

`Revise`・`AddCharacter`・`AddWorldDocument` は、取りかかれる工程ではないので工程の一覧（`pipeline`）には出ない。
`AddCharacter` / `AddWorldDocument` は、指示から人物・世界観の資料を 1 つ作って足す（自分で書いて足す構成の操作 §4.8 の、LLM に作らせる版）。
`stages/addition/` が、LLM の出力を検証して `StructureEdit::AddCharacter` / `AddWorldDocument` に組み立て、`plan_structure_edit` に渡して変更案にする。

どのタスクも **ファイルを直接書き換えず**、変更案 `ChangeSet { summary, files: Vec<FileChange>, project_root }` を返す。
GUI は変更案を見せてから適用し、CLI は自動で適用する。

- `FileChange` は `kind` を持つ enum（JSON では `{ "kind": "write" | "trash" | "move" | "expect", … }`）。
  - `Write { path, content, previous, base_hash }`: ファイルの新規作成・上書き。`previous` は変更前の内容（新規なら `null`）。
  - `Trash { path, files: [TrashedFile { path, base_hash, chars }] }`: ファイルまたはフォルダをゴミ箱へ移す（§2）。
    ファイルなら `files` は `path` のファイル 1 つ。フォルダなら、フォルダの中のファイル全部（サブフォルダの下も含む。
    空のフォルダなら空）。`chars` は失われる内容の文字数（利用者に見せるため）。テキストとして読めず `base_hash` が
    無いものは適用できない。
  - `Move { from, to }`: ファイルまたはフォルダの改名（章の番号の振り直し）。中身は変えない。
  - `Expect { path, base_hash }`: 何も書かず、適用するときにこのパスがこの状態であることだけを確かめる（`base_hash` が
    `null` なら「無いこと」）。計画のあとに外で状態が変わったら競合にするため。
  - 項目を別に足すのではなく enum にしているのは、`files` しか見ない表示や判定が削除・改名を黙って見せないままにするのを、
    型の絞り込み（漏れのない `match`）で防ぐため。生成のタスクが作るのは `Write` だけで、`Trash`・`Move`・`Expect` は
    構成の操作（§4.8）が作る。
  - ビルダーは `put`（書き込み）・`trash_file`・`trash_folder`・`move_entry`・`expect`。
- 適用（`ChangeSet::apply`）は `apply_changes`（§3.3）を使い、すべての変更を反映するか、何も反映しないかのどちらかにする。
  反映の順は並び順に頼らず「確認（Expect）→ ゴミ箱へ移す → 改名 → 書く」。それぞれの条件は次のとおり。違えば `Conflict`。

  | 操作 | 適用できる条件 |
  |---|---|
  | Expect | 今のハッシュが `base_hash` と一致する（`null` なら無いこと） |
  | Trash（ファイル） | 今のハッシュが `base_hash` と一致する |
  | Trash（フォルダ） | 中のファイルの一覧（パスとハッシュ）が計画のときと完全に一致する。増えていても変わっていても競合 |
  | Move | `from` がある。`to` が、同じ変更案の Trash や Move で空く場所を除いて空いている。中身のハッシュは見ない |
  | Write | 条件は改名した後の状態に対して確かめる。書き先が Move の行き先なら移動元の今の中身で `base_hash` を照合する。Trash や Move で空いた場所なら「無いこと」として扱う |

  書き込みは生成を始めたときの内容のハッシュ（`base_hash`、新規なら存在しないこと）を条件にするので、その後に利用者が
  編集していれば `Conflict` になる。ただし `AddCharacter` / `AddWorldDocument` の変更案は、構成の関数が最新の状態を読んで作る
  ので、条件は「LLM の出力を受け取ったあとの状態」になる（生成している間に保存された手の編集を、古い状態を前提に上書きしない。
  新規のファイルだけを書くので、そのあとに同じパスができれば適用が `Conflict` になる）。同じパスへの変更が重なる・`.kataribe/` や `kataribe.yaml` をゴミ箱へ移したり改名したりする・
  フォルダを自分の中へ移す・ゴミ箱へ移すフォルダの下へ書く・`Trash` の形が合わない変更案は `InvalidChangeSet`
  （画面から戻ってくる値なので、Rust で必ず検証する）。
- `project_root` は生成元の作品フォルダ（正規化した絶対パス）。生成中に別の作品を開き直しても、
  前の作品の変更案を今の作品に書き込まないよう、適用時に照合する。

### 4.2 本文の生成単位（設定で切り替え）

| 単位 | 1 回の生成 | 向いている環境 |
|---|---|---|
| `chapter` | 指定したシーンから、次に本文のあるシーンの手前まで（シーン区切りで分割して保存）。手で書いたシーンは置き換えない | 長い文脈に強い大きなモデル |
| `scene` | 1 シーン。`chars_per_call` を超えるシーンは複数回に分けて書き継ぐ | 中規模以上のモデル |
| `beat` | シーンを展開（ビート）に分け、1 ビートずつ書く | ローカル LLM（既定）。分量が安定し、設定から外れにくい |

`chars_per_call`（1 回あたりの目安文字数）と `context_tokens`（モデルに渡せる文脈の長さ）も設定で変えられる。

既定を `beat` にしたのは、VRAM 12GB の環境で短編ミステリの第 1 章を書き比べた結果による（gemma-4-12b / qwen3.5-9b）。
`beat` は分量が目標どおりに収まった。`scene` は書き継ぎで前の段落を繰り返して 1.5 倍を超え、
`chapter` は目標の半分ほどしか書けなかった。

### 4.2.1 推論モデルの思考を止める

ローカルの推論モデル（Qwen 系・Gemma 系など）は、何も指定しないと出力の上限まで「思考」して本文が空になることがある。
`disable_thinking`（既定 true）のとき、次の 2 つを同時に指定する。LM Studio 0.4 で実測したところ、
Gemma には前者だけ、Qwen には後者だけが効き、両方を指定すると両方で安定した。

- リクエストに `reasoning_effort: "none"`
- 最後のメッセージとして、応答の書き出しに空の思考ブロック（`<think>

</think>

`）を置く

クラウドの API ではこれがエラーになりうるので、その場合は設定で切る。Claude Code ではどちらも使われない（§3.2.1）。

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

予算とは別に、直前までの本文（約 2 万字）と書きかけの本文から繰り返し使っている表現を機械的に抜き出し、
「別の言い方にする（物語に必要な用語はそのまま使ってよい）」と伝える。ローカル LLM は前のシーンの決まり文句を
場面をまたいで繰り返しがちなため（試作の短編で「嵐の咆哮」11 回）。同じ設計で書き比べると、
文の使い回しが約 3 分の 1 に、最も多い表現の回数が 14 回から 9 回に減った。

### 4.4 プロンプト

- プロンプトはコードに埋め込まず、`crates/kataribe-engine/prompts/*.j2`（minijinja）に置く。
- ジャンル・年齢区分ごとの書き方の指針は `crates/kataribe-engine/presets/genres.yaml` に置く。
- 年齢区分 `r18` でも、性的な場面の登場人物は全員成人として描く規則をプロンプトに必ず含める。

### 4.5 LLM の接続先

設定の `LlmSettings.provider` で切り替える。GUI と CLI は、同じ `kataribe_engine::build_chat_model` / `list_models` で
設定から LLM を作る。

| `provider` | 使う項目 | API キー |
|---|---|---|
| `openai_compatible`（既定） | `base_url`・`model` | 資格情報マネージャー（要るサーバーだけ） |
| `claude_code` | `claude_command`（既定 `claude`）・`claude_model`（既定 `sonnet`） | 使わない（資格情報マネージャーも読まない） |

モデルの指定は接続先ごとに分けて持つ。接続先を切り替えても、もう一方のモデルの指定を失わない。

### 4.6 作品ごとの設定

`kataribe.yaml` の `settings` に、その作品だけの設定を書ける（`kataribe_engine::ProjectSettings`）。
使う値は **アプリ全体の設定 ← 作品の設定 ← CLI の指定** の順に上書きして決め、最後に値を安全な範囲に収める。
作品と一緒に Git で管理でき、どの設定で書いた作品かも残る。

| 書ける項目 | 書けない項目（アプリ全体の設定だけ） |
|---|---|
| `provider`・`model`・`claude_model`・`draft_unit`・`chars_per_call`・`context_tokens`・`temperature`・`polish`・`quality_retries`・`disable_thinking` | `base_url`・`claude_command`（PC ごとに違いうる）、API キー、エディタの見た目、最近の作品 |

- どの項目も省略でき、省略した項目はアプリ全体の設定を使う。空なら `settings` の項目ごと書かない。
- この版が知らない項目は、保存し直しても残す（新しい版で足した設定を古い版で消さないため）。
- 保存は `Project::update_manifest` で行い、作品情報と未知の項目は残す。次のときは競合にする。
  - 画面で読んだとき（`ProjectSettings::load_with_hash` のハッシュ）から、保存までの間に `kataribe.yaml` が変わった。
  - 読んでから書くまでの間に、外で変更された。
- 保存するときは、Rust が数値の項目を使うときと同じ範囲に収める。画面は、作品の設定を何も変えていなければ
  保存しない（手で書かれた値を、触っていないのに書き換えないため）。変えたときは、変えた項目の空欄・範囲外を
  補正して送る。手で書かれた範囲外の値は、画面に「実際には N で使います」と添える。
- 書き直すと、`kataribe.yaml` に手で書いたコメントや項目の順番は残らない。そのため、書き直す前の内容を毎回バックアップに残す。
- 画面は、作品情報をエディタで開いていれば、保存の前にその編集を保存し、保存の後に読み直す
  （古い内容が残ったまま自動保存が競合し、利用者が上書きで設定を消すのを防ぐ）。
- 生成・工程の組み立て（生成単位）・接続テスト（`llm` を渡さないとき）は、作品を開いていれば重ねた後の設定を使う。
  作品の設定は毎回ファイルから読むので、外で `kataribe.yaml` を直しても次の生成から反映される。
- 画面の設定ダイアログは、作品を開いているときだけ「アプリ全体 / この作品」を切り替えられる。
  「この作品」では、項目ごとの「この作品で変える」のチェックで上書きするかを選び、チェックの無い項目はアプリ全体の値を見せる。

### 4.7 生成中のイベント

`Engine::generate` は、生成の経過を `EventSink` に `GenerationEvent` として送る。GUI は右の AI パネルに（どのタブから始めた生成でも同じ場所に）、CLI は標準エラー出力に出す。

| イベント | 中身 | 画面での使い方 |
|---|---|---|
| `started` | `model`: 使う LLM の名前（`ChatModel::describe`。例 `Claude Code（haiku）`） | 見出しに「使う LLM」として出す。作品の設定を重ねた後の、実際の接続先。URL はホスト名とポートだけを出す（認証情報やクエリは秘密を含みうるので出さない） |
| `step_started` | `label`・`index`・`total`（LLM を呼ぶ 1 回ごと） | 回ごとの枠を作り、経過時間を数え始める |
| `content` / `reasoning` | 本文 / 思考の断片 | 本文は流して見せ、受け取った文字数を数える。思考は畳んで見せる |
| `step_finished` | 使用トークン数・かかった時間 | 「完了（23.2 秒、入力 … トークン・出力 … トークン）」 |
| `notice` | 注意書き | その回の下に出す |

- 本文を流さない回（要約や、JSON で答えさせる回）でも、経過時間が進むので、動いていることが分かる。
- 経過時間を数えるのは最後の回だけ。`step_finished` が来ないまま次の `step_started` が来た回
  （JSON Schema の指定を断られて、指示文で JSON を求め直した回）は「やり直しました」と出す。
- 見出しには、工程全体のうち済んだ数（例 工程 13/15 済み）も出す。

### 4.8 構成の操作（structure）

人物・世界観の資料・章・シーンを、利用者が自分で書いて足したり消したり、人物・章・シーンを並べ替えたりする操作。LLM も設定も要らないので、`Engine` の外の関数
（`plan_structure_edit` / `suggest_character_id`）にしてあり、GUI と CLI が同じ関数を使う。作品フォルダは直接書き換えず、
変更案を `StructurePlan` に入れて返す。適用は `ChangeSet::apply`（§4.1）。

人物と世界観の資料は、LLM に作らせて足すときも同じ関数を使う（`Task::AddCharacter` / `AddWorldDocument`、§4.1。`stages/addition/`）。
流れは、①指示が空でないことと、世界観の資料の `name` の指定（規則に合うか・使用済みでないか。`structure` の確認関数
`check_name`）を、LLM を呼ぶ前に確かめる、②文脈を集めて LLM に作らせる、③出力を検証する、④`StructureEdit` にして
`plan_structure_edit` に渡す、⑤`StructurePlan` の変更案を、要約「人物「霧島 凛」を生成しました（characters/kirishima-rin.md）。」
の生成の `ChangeSet` にして返す。構成の操作のエラー文は利用者の入力向けなので、LLM の出力の誤りは `stages/addition/` が先に
`InvalidOutput` にする。
- 人物（`AddCharacter`）: 項目を JSON `{name, reading, role, summary}` で作らせ、続けて人物資料の本文を `character.j2` で作らせる
  （LLM を 2 回呼ぶ。ID は LLM に出させず、読みからローマ字で決める）。名前が空・名前にローマ字が混ざる・読みがかな以外・
  すでにいる人物と同じ名前（空白の違いを無視）のときは、`quality_retries` の回数まで作り直す。それでも残れば警告して使う
  （名前が空のときは `InvalidOutput`）。`order` は付けず、末尾にする。
- 世界観の資料（`AddWorldDocument`）: 文章で作らせ、1 行目を `# 題` にさせる（ローカル LLM は長い Markdown を JSON の文字列に
  入れるのが苦手なため）。見出しが無ければ作り直し、それでも無ければ `InvalidOutput`。ファイル名は §2 の規則（`name` の指定があればそれ）。
- 生成の最後に、古くなった工程の注意書きを Info の `notice` として流す（工程の印は付けない）。人物は、生成済みのあらすじ・章立て・
  シーン構成には、その人物がまだ出てこないこと。世界観の資料は、これからの生成で世界観として使われること（長いと切り詰められる）と、
  生成済みの文書には反映されないこと。

```rust
pub fn plan_structure_edit(project: &Project, edit: &StructureEdit) -> Result<StructurePlan>;
pub fn suggest_character_id(project: &Project, reading: &str, name: &str) -> Result<CharacterId>;

pub struct StructurePlan {
    pub change_set: ChangeSet,           // made_for(project) 済み
    pub created: Option<RelPath>,        // 適用したあとに開く文書
    pub references: Vec<SceneReference>, // 人物を削除するときだけ。その人物の名前を挙げているシーン
    pub renumbered: Vec<RenumberedChapter>, // 章を足す・消す・並べ替えるときだけ。番号が変わる章（変わる前・後の番号と章題）
    pub notices: Vec<String>,            // 「第 3 章は読めないため参照を確かめられませんでした」など
}
```

| 操作（`StructureEdit`） | 変更案 | 備考 |
|---|---|---|
| `AddCharacter { id, meta, body }` | `characters/<id>.md` を `Write`（新規） | 名前が空なら `InvalidInput`。`id` は文字列（`Option<String>`）で受け、ここで `CharacterId::new` により検証する（画面から戻ってくる値なので。使えなければ `InvalidInput` と日本語の理由「ID「Rin」は使えません。小文字の英数字とハイフンで、…」）。`None` か空白だけなら読み（無ければ名前）からローマ字で決める（§2）。使用済みなら「ID「x」はもう使われています。」。`meta.order` が `None` なら今の最大の次（末尾）。本文が空でもよく、その人物は生成の工程に「人物資料: X」が取りかかれる工程として出る |
| `RemoveCharacter { path }` | `Trash` | ID ではなくパスで指す（世界観の資料の削除と同じ形）。`characters/` 直下の `.md` ならよく、ファイル名が ID の規則に合わない資料（手で足した `characters/Rin.md`・`characters/凛.md`。目次には出る）も消せる。それ以外のパスは `InvalidInput`。YAML が読めれば名前でシーンの参照を確かめ、読めなければ（名前を読めないので）参照は調べられず `notices` に出す |
| `AddWorldDocument { name, title, body }` | `world/<name>.md` を `Write`（新規）。中身は `# 題` と本文 | `name` が `None`（空欄）なら題から決める（§2）。題が空・複数行なら `InvalidInput` |
| `RemoveWorldDocument { path }` | `Trash` | `world/` 直下の `.md` だけ。`world/overview.md` は `InvalidInput`（`Overview.md` など大文字小文字の違いも同じ。Windows では同じファイル） |
| `AddChapter { before, title, storyline }` | 後ろの章を `Move`、新しい章の章立てを `Write`（新規。改名のあとの「無いこと」が条件） | `before` が `None` なら末尾。題が空・複数行なら `InvalidInput`、`before` の章が無ければ `NotFound`。章立ては `title` と `storyline`（`scenes` は無し）。番号の振り直しは下記 |
| `RemoveChapter { chapter }` | 章立てを `Trash`、本文のフォルダがあれば中のファイルごと `Trash`（フォルダ）、後ろの章を `Move` | 章が無ければ `NotFound`。本文のフォルダの中にテキストとして読めないファイルがあれば `InvalidInput`。ほかの章の YAML は読まない |
| `AddScene { chapter, before, scene }` | 章立てを `Write`（今の内容を条件にする） | `scene` は `ScenePlan` から id とビートを除いたもの（`NewScenePlan`）。`before` が `None` なら章の末尾。新しい id は、章立てにある id と、本文のフォルダに残っている本文（章立てから消えたシーンのもの）の id を避けて決める（消したシーンの本文を引き継がないため）。章立てが壊れている・シーンの id が重複しているときは `InvalidInput`（直してから操作する） |
| `RemoveScene { chapter, scene }` | 章立てを `Write`、本文があれば本文を `Trash`、無ければ `Expect`（本文が無いこと） | 確認している間に外のエディタや CLI で本文ができたら、章立てだけが書き換わって本文が章立てに無いまま残らないよう、適用のときに競合にする |
| `MoveCharacter { path, position }` | 読める人物資料のうち `order` が変わるものだけを `Write`（今の内容を条件にする） | ID ではなくパスで指す。`position` は、並べ替えたあとに目次の人物の何番目に来るか（0 始まり。目次と同じ並び＝`order` → ファイル名の順、`order` の無い人物は最後）。読める人物の `order` を 1, 2, 3… に振り直す（`order` の欠番・重複・無しもここでそろう）。YAML が読めない人物資料は動かせず `InvalidInput`（`characters/Rin.md` のようにファイル名が ID の規則に合わない資料も、目次と同じく読めないものとして数える）。ほかの人物を動かすときは、読めない資料の `order` を変えずに飛ばし、`notices` で知らせる（読めない資料は目次の最後に並ぶので、その位置へ動かした人物は読める人物の最後になる）。`characters/` 直下の Markdown 以外のパスは `InvalidInput`、資料が無ければ `NotFound` |
| `MoveChapter { chapter, position }` | `Move`（動く範囲の章の章立てと本文のフォルダ。本文のフォルダの無い章は `Expect`） | `position` は、並べ替えたあとに章（番号順）の何番目に来るか（0 始まり）。章が無ければ `NotFound`。章立てを読まず、番号だけで決まる。番号の振り直しは下記 |
| `MoveScene { chapter, scene, position }` | 章立てを `Write`（今の内容を条件にする） | 章立ての `scenes` の並びだけを変える。シーンの `id` と本文のファイル（`manuscript/<NN>/<id>.txt`）は変えない（本文との対応は `id` で決まるので崩れない）。`position` は章のシーンの何番目か（0 始まり）。章立てが壊れている・シーンの `id` が重複しているときは `InvalidInput`（`AddScene` / `RemoveScene` と同じ）、章・シーンが無ければ `NotFound` |

- 並べ替えの `position` は、どれも「並べ替えたあとに、その項目が一覧の何番目に来るか」（0 始まり）。範囲外と、今と同じ位置は
  `InvalidInput` にする（空の変更案にはしない。何も起きない操作を成功として返すと、呼び出し側の数え間違いが見えなくなるため。
  画面は今と同じ位置を送らない）。人物は、並べ替えても `order` が 1 つも変わらないとき（読めない資料の位置へ動かしたなど）も同じ。

- 章立ての YAML は書き直すので、利用者が手で書いたコメントや項目の順番は残らない（アプリが知らない項目は残る。画面から項目を
  変えて保存したときと同じ）。
- 章の番号の振り直し（`structure/chapters.rs`）。章の順序はファイル名なので、途中に足す・消す・動かすときは
  動く範囲の章の `plot/chapters/<NN>.md` と `manuscript/<NN>/` を改名する。
  - `before = X` で足すなら、X 以上の章を 1 つ後ろへずらし、新しい章を X にする。末尾に足すときは、最大の番号の次（無ければ 01）で、
    ほかの章は改名しない。消すときは、それより後ろの章を 1 つ前へずらす。
  - 途中が抜けた番号は抜けたまま残す（手で作った番号を勝手に詰めない）。ずらすのは操作した位置より後ろの章だけ。
    `001` のように手で付けた 3 桁の番号は、ずらした章だけが 2 桁になる（`ChapterId::shifted`。並び順は番号で決まるので崩れない）。
    999 を超えるときは `InvalidInput`。
  - 章を動かす（`MoveChapter`）ときは、動く範囲（今の位置と行き先の位置の間）の章が持っている番号の集合を、並べ替えたあとの
    並びへそのまま割り当てる（01・02・03 で 03 を先頭へなら 03→01、01→02、02→03。01・02・05 で 05 を先頭へなら
    05→01、01→02、02→05）。番号の集合は変わらないので、抜けた番号は抜けたまま、桁数（`001` など）も番号ごとに保たれ
    （章についていくのではなく、その番号に残る。`001`・`02` で入れ替えても、番号は `001`・`02` のまま）、
    999 を超えることもない（`shifted` を使わない）。範囲の外の章は改名しない。入れ替えのような循環の改名は、適用が
    2 段階の移動（§3.3）で扱うので、全部か無しかになる。行き先は全部、同じ変更案で動く章の元の場所なので、
    塞がっているかの確認（下の項目）は通り、行き先を足す `Expect` は要らない（本文のフォルダの無い章の
    移動元の `Expect` は、ほかの章の行き先になっても重複させない）。
  - 本文のフォルダは、フォルダごと 1 回の `Move`（中のファイルを 1 つずつ移すと、章立てに載っていない本文が古い番号のフォルダに
    残り、別の章の本文と混ざる）。本文がまだ無い章（フォルダが無い章）は `Move` も `Trash` も作らず、適用のときに移動元にまだ無いことを
    `Expect` で確かめる（計画のあとに外で本文ができて、改名から取り残されないように）。移動先も、同じ変更案の `Move`（移動元）・
    `Trash`・`Expect` で扱っていなければ、「無いこと」の `Expect` を足す（番号が抜けていて行き先が空いているとき、確認の間に
    外で `manuscript/04/` ができると、新しい第 4 章が関係のない本文を自分の本文として読むため。`Expect` どうしが同じパスで
    重ならないよう、全部の改名を並べ終えてから足す）。
  - 章が置かれる場所（改名の行き先と新しい章。章立てのファイルと本文のフォルダ）が、章立ての無い `manuscript/<NN>` などで塞がって
    いれば、計画の段階で「manuscript/02 が既にあるため…」と断る（本文のフォルダが無い章をずらすときも、行き先に残っていると
    ずらした章の本文として読まれてしまうため）。同じ変更案のゴミ箱や改名で空く場所は塞がっていない。
  - 利用者に見せる材料は、`Trash` の `files`（本文のフォルダの中のファイルと字数）と、`StructurePlan.renumbered`
    （変わる前・後の番号と、章題。章立てが読めなければ章題は `null`。番号の小さい順）。要約は「第3章「…」を追加します。」「第3章「…」をゴミ箱へ
    移します。」「第3章「…」を第1章へ移します。」（章題が読めなければ番号だけ。行き先は、並べ替えたあとにその章が持つ番号）。
    並べ替えの `renumbered` には、動く範囲の章（動かした章自身を含む）が入る。
  - 影響（設計上の割り切り）。要約のキャッシュ（`.kataribe/cache` の `NN/sNN` の鍵）は番号が変わると合わなくなり、
    作り直すぶん LLM の呼び出しが増える（誤った要約は使われない）。バックアップは古いパスの名前で残るので、
    改名した章の履歴は別の番号に見える。生成の変更案は章番号を含むパスに書くので、画面は生成のセッションが落ち着いていない間は
    章の追加・削除・並べ替えを止める。GUI と CLI を同時に使ったときの排他は、今と同じく効かない。
    章の並べ替えも同じで、要約のキャッシュの鍵が合わなくなり（作り直しで LLM の呼び出しが増える）、バックアップは
    古い名前のまま別の章の履歴に見える。
- 人物の並べ替えは、`order` が変わる人物資料の YAML を書き直す。front matter に手で書いたコメントや項目の順番は残らない
  （アプリが知らない項目と本文は残る。画面から人物資料の項目を変えて保存したときと同じ）。`order` の行だけを行単位で
  書き換えて YAML を保つ方法は、引用符・ブロック表記・同じ項目の重なりなどを文字列として追うことになり壊れやすいので採らない。
  `order` が変わらない人物資料は書き直さない（コメントを不必要に失わせない）。シーンの並べ替えが章立てを書き直すのも同じ。
  人物の並びを決める規則（`order` の昇順、無ければ最後、同じ値はファイル名の順）は、目次（`overview`）と並べ替えが
  `overview::sort_for_display` を共有する。人物の要約の「N 番目」は、並べ替えたあとに読める人物の中で何番目かで数える
  （読めない資料は `order` を振り直さず目次の最後へ回るので、手前にあっても数えない）。
- 目次の「プロット」「本文」の章の並びは、ファイル名の文字列順ではなく `ChapterId` の順（番号の昇順。`Project::chapter_ids` と
  同じ）。文字列順だと `100` が `10` と `11` の間に来て、番号順の位置を送る章の並べ替えが、意図しない範囲の改名になる。
- 参照の確認（人物を消すとき）: 全部の章を 1 つずつ解釈し、各シーンの `pov` と `characters` を、名前の一致（空白の違いと、
  「凛」のような姓や名だけの書き方を同一人物とみなす。`names.rs`。本文の生成が名前から人物を探す規則と同じ）で調べる。
  読めない章は `notices` に入れて続ける。シーンの名前は自動では書き換えず、知らせるだけにする。
- 目次の項目 `OverviewEntry` は、章立て・本文の章見出し・シーンの本文の項目に `chapter`、シーンの本文の項目に `scene` を持つ
  （本文の章見出しにはパスが無いので、画面が章番号を知るため）。

## 5. アプリ（src-tauri）と画面（src）

- Rust 側は engine を Tauri コマンドとして公開するだけの薄い層にする。
- ストリーミング出力は `tauri::ipc::Channel` で画面へ送る。キャンセルは画面が採番したジョブ ID で行う。
- API キーは Windows 資格情報マネージャーに保存し、画面側には渡さない。
- 画面の型は ts-rs が Rust の型から生成する `src/bindings/` を使う。手書きの契約（`src/api/types.ts`・`backend.ts`）を
  残す場合は、`src/api/bindingsContract.ts` で生成物と完全に一致することを型検査で保証する。
  生成物が Rust の型と一致していることは、CI で `cargo test --all-features` の後に `src/bindings` に差分が無いことで確かめる。
- ブラウザ単体（`pnpm dev`）では、メモリ上の偽バックエンドで動く（画面の開発とテスト用）。人物資料・章立てのパスの
  id は本物と同じ規則で判定し（人物は小文字の英数字とハイフンの slug、章は 2〜3 桁の数字）、合わないパスに保存すると
  `invalid_input` にする。人物の本文は保存したまま読み直せる（空にしても空のまま。生成していないときのプレースホルダー
  の文は、本文として保存しない）。ただし項目の「無い」と空文字は区別せず、空文字は `null` で返す（本物の `reading: ""`
  との違い）。`parseDocument` も、作品を開かずに、偽の front matter の書式（`render.ts` が作るもの）から同じ形に分ける
  （ビートと、アプリが知らない項目は書式に無いので分けられない）。本物と同じく、front matter が無い、または必須の項目
  （人物の `name`、章の `title`。id では補わない）が無い人物資料・章立ては、`text` のまま `parse_error` を添えて返し、
  本文は書かれたまま（空やプレースホルダーの文も、そのまま）返す。
  構成の操作（§4.8）も同じ形で動く。世界観の資料（`world/<name>.md`）は概要とは別に持ち、`planStructureEdit` は Rust と同じ形の変更案
  （`Write` / `Trash` / `Move` / `Expect`。ハッシュは `hashText`）と、人物を消すときの参照（名前の一致の規則は `names.rs` と同じ）を返す。
  `applyChangeSet`（`mock/apply.ts`）は、並び順に頼らず「Expect（今の状態で確かめる）→ ゴミ箱へ移す → 移動 → 書く」の順に、それぞれ競合を確かめ
  （状態を書き換えずに進め、競合したら何も変えない。ゴミ箱の確認は、移す前の状態に対してまとめて行う。書き込みの条件は、
  移動のあとの状態に対して確かめる。空いた場所への書き込みは「無いこと」が条件になる）、
  `kataribe.yaml` や `.kataribe/` のゴミ箱への移動・移動と、ゴミ箱へ移す変更の `files` が対象に合わないこと（ファイルなら対象と同じ 1 つ、
  フォルダなら中のファイルだけ）・同じ種類の変更が同じパスに重なること・ゴミ箱へ移すパスの下への書き込み・フォルダを自分の中へ移すことは
  `invalid_input` にする（Windows は大文字小文字を区別しないので、`Kataribe.yaml` や `.Kataribe/…` も、パスの重なりも、区別せずに拒む）。
  並べ替えも同じ形（`mock/structure.ts` の人物・シーン、`mock/chapters.ts` の章）。章は、動く範囲の章が持つ番号を並べ替えたあとの並びへ
  そのまま割り当て（抜けた番号は抜けたまま）、`renumbered` も同じ規則で作る。本物のような、行き先が章立てのない本文のフォルダで
  塞がっていることの検出はしない。人物は、`order` が変わる人物資料だけを書き直す。偽の作品には YAML が読めない人物資料が無いので、
  本物のような「読めない資料を飛ばす」扱いと、その注意書き（`notices`）は無い。人物資料の書き直しは偽の書式（`render.ts`）で行うので、
  アプリの知らない項目は残らない（本物は残る）。
  目次の並びは本物と同じ。人物は `order` の順で、同じ値（無いものどうしを含む）はファイル名の順（本物の `sort_for_display` と同じく、
  読み込んだ順のまま。`rin-a.md` は `rin.md` より前になるので、id の順とは限らない）。章は、プロットの節も本文の節も番号の順
  （`100` は `99` の後ろ）。
  偽の作品はファイルの木ではなく章ごとの構造なので、本物との違いがある。章の本文のフォルダ（`manuscript/<NN>`）は章と一緒に動く
  （章立てのファイルの `Move` が章 id の付け替えを一度にまとめて行い、入れ替えのような循環も扱う。本文のフォルダの `Move` は
  移動元があることの確認だけ）。フォルダの `Trash` は、中のファイル（本文のあるシーン）の一覧がパスもハッシュも計画のときと一致することを確かめる。
  フォルダの `Expect` は、本文が 1 つも無いことの確認になる。パスの重なりの検証は同じパスだけを見て、フォルダとその中のパスの重なりは調べない。
  章の追加・削除の変更案（`mock/chapters.ts`）は本物と同じ形で、番号の振り直しと `renumbered` も同じ規則で作る（章の番号は 0〜999。
  本物の `ChapterId::shifted` と同じく、00 も許す）。本文のフォルダが無い章をずらすときは、元の場所のほかに、移す先も、同じ変更案の
  `Move`・`Trash`・`Expect` で扱っていなければ「無いこと」の `Expect` を足す（`Expect` どうしが同じパスで重ならないように）。
  番号が抜けていて行き先が空いているときに、外でできた本文のフォルダの上へ取り残された本文を作らないため。
  本物のような、章が置かれる場所が章立ての無い `manuscript/<NN>` で塞がっていることの検出はしない（偽の作品にはありえないため）。
  世界観の概要の判定も大文字小文字を無視し、`world/Overview.md` を足した資料として扱わない。
  人物の ID（`add_character` の `id`）は、本物と同じ規則（小文字の英数字とハイフン・48 文字まで・先頭末尾と連続のハイフン不可・Windows の予約名不可。
  `src/lib/slug.ts` を画面と共有する）で確かめ、合わなければ `invalid_input` と理由を返す。空白だけなら自動。人物の削除（`remove_character`）は
  ID ではなくパスで指し、`characters/` 直下の Markdown なら、ファイル名が ID の規則に合わない資料（`characters/Rin.md` など）も消せる。
  本物との違いは、かなをローマ字にしないこと（変換表を二重に持たないため）。`suggestCharacterId` は読み（無ければ名前）の英数字だけを
  slug にし、作れなければ `character`（使用済みなら番号を付ける）。世界観の資料の自動のファイル名も、英数字だけの題なら slug、
  それ以外は `doc`（重なれば `doc-2`, …）になる。かなの変換を画面のテストで確かめるときは、スタブで差し替える。
  壊れた章立てがありえないので、`notices` は常に空。ID・ファイル名が使えないときの文言も本物とは違う。
  指示から人物・世界観の資料を作って足す生成（`add_character` / `add_world_document`。`mock/generation.ts`）も同じ構成の操作の関数
  （`planMockStructureEdit`）で変更案を作り、要約は本物と同じ形（「人物「…」を生成しました（characters/…md）。」）にする。
  指示が空なら、文字を流す前に `invalid_input`。資料のファイル名の指定は、文字を流す前に確かめる（規則に合わない・使用済みなら `invalid_input`）。
  人物は項目の JSON と本文の 2 回に分けて流し、資料は見出し付きの本文を 1 回で流す。本物との違いは、中身が決まった文であること
  （名前は既存の人物と重ならない候補から選び、題は指示の 1 行目）、かなをローマ字にしないので人物の ID が `character`・`character-2`、…になること、
  古くなった文書の注意書きが、実際に何が生成済みかを調べない決まった文であること。
- 中央のエディタは、文書の種類（`EditableDocument.kind`）で出し分ける。人物資料・章立ては、本文の上に front matter の
  項目のフォームを置き、本文（章はストーリーライン）の欄だけを縦書き・横書きの切り替えの対象にする。
  文字数・品質チェック・ルビのプレビューも本文だけを対象にする。項目に分けない文書（`text`）と、YAML を解釈できず
  文字列で開いた人物資料・章立てはファイル全体を 1 つの欄で編集し、後者には理由を添える。
- AI パネルの変更案は、ファイルごとに内容を見せる。人物資料・章立ての変更案は、YAML を画面で解釈せず、
  `parse_document`（§3.3）で分けた結果を、読むだけの項目の一覧と本文で見せる（入力欄にはしない。項目名はフォームと
  共有の定数 `document-form/fieldLabels.ts`。章立てのシーンはシーンごとのまとまり）。
  アプリが知らない項目（利用者が足した項目。Rust の `extra`。JSON の meta 直下に載って届く）は、人物・章・シーンごとに
  「その他の項目」としてキー名と値（文字列以外は JSON）で見せる。書き直しの工程は人物資料・章立ての全文を LLM が
  書き直すので、足した項目が落ちたり変わったりしたことに気付けるようにするため。
  変更前（`previous`）が同じ種類の文書として分けられたときは、変わった項目と、変わった本文に「変更」の印を付ける。
  比べる項目は両側の和集合で、片方にしか無い項目（変更後に消えたビート、落ちた知らない項目）は、無い側で「（なし）」として
  見せて印を付ける。章立てのシーンは `id` で突き合わせ、変更前に無いシーンは「追加」、変更後に無いシーンは「削除」とする
  （変更後を見ているときは、削除されたシーンを末尾に足す）。両方にあるシーンは、並びを保ったまま残る最長の列に入らない
  ものに「順序変更」の印を付ける（1 つを移したときに、間のシーンまで動いたことにしない）。シーンは項目が変わっても
  並びが変わっても印が付き、両方なら両方の印が付く。
  「変更前を見る / 変更後を見る」で切り替えられ、変更前も同じ形で見せる（印は、もう一方との比較）。
  印は色だけでなく文字でも伝える。変更前があるのに比べられないとき（相手が `text`・分けている途中・分解に失敗）は、
  「変更前と比べられないため、印は付けていません」と一行添えて、印が無いことを「変更なし」と取り違えさせない
  （分けている途中は、終われば消える）。
  項目に分けない文書（`text`）と、分けられなかった文書（理由を添える）は、書かれたままの文字列で見せる。
  分けている間と、`parse_document` の呼び出し自体が失敗したときも、同じく書かれたままの内容を見せる
  （何も見えない時間を作らず、分けられないことで変更案を見失わない）。失敗は握りつぶさず、
  「項目に分けられませんでした（理由）」と添える（IPC の失敗やコマンドの登録漏れに気付けるように）。
  変更案の 1 つ 1 つは `kind` で出し分ける（`FileChangeCard`）。書き込みは上のとおり内容を、ゴミ箱へ移す変更は `TrashCard` が
  「削除」の印と、移るファイルの一覧（パスと文字数。フォルダなら中のファイル全部。`TrashedFileList`。削除の確認と共有する部品）を、
  移動は `MoveCard` が「移動」の印と移動元 → 移動先（中身は変わらない）を見せる。状態を確かめるだけの変更（`expect`）は
  何も変えないので見せない。
- 目次（`ProjectTree`）から、人物・世界観の資料・章・シーンを追加・削除し、人物・章・シーンを並べ替える（§4.8）。
  - 世界観・登場人物・あらすじと章立ての節の見出しに「＋」（「資料を追加」「人物を追加」「章を追加」）、行ごとに「⋯」の操作メニュー（`ActionMenu`。
    Escape・メニューの外のクリック・フォーカスが外へ移ったときに閉じる）を置く。メニューの中身は項目の種類で決まる
    （`entryActionsFor`）。人物（`characters/` 直下の Markdown なら、ファイル名が ID の規則に合わなくても。パスで指す）と、
    足した世界観の資料（概要は除く。大文字小文字は区別しない）は「削除」、本文の章見出しは「シーンを追加」（末尾）と「章を削除」、
    章立てのファイル（プロットの節の章の行）は「この前に章を追加」「この後に章を追加」「削除」、
    シーンは「この前にシーンを追加」「この後にシーンを追加」「削除」。企画・あらすじなどにはメニューを出さない。
    人物・章・シーンには、これに加えて「上へ移す」「下へ移す」が出る（本文の章見出しは、シーンの操作と並ぶので
    「章を上へ移す」「章を下へ移す」）。先頭の項目には「上へ」、末尾の項目には「下へ」を出さない。
    章立ての行と本文の章見出しは同じ題で並ぶので、操作メニューの名前は章立ての行にだけ「章立て」を添えて見分ける
    （「『…』の章立ての操作」）。「この後に」は、次の項目（章・シーン）の前を初めの位置にする（最後なら末尾）。
  - 生成のセッションが落ち着いていない間（実行中・変更案の確認中・生成の失敗の表示中・自動で進め中）は、追加・削除・並べ替えを無効にし、
    理由を `title` に出す。確認中の変更案の書き先（新規に書く本文など）がずれるのを防ぐため。
    「空の本文から書き始める」も同じ理由で、同じ間は押せない（まだ無い本文を開いて生成し、実行中や確認中に空の本文を作ると、
    生成した本文の新規作成の条件が合わなくなり、適用が必ず競合するため）。
  - 逆に、構成の操作（追加・削除・並べ替え）を作ってから適用し終えるまでの間は、生成も始められない。章の番号の振り直しの前の
    パスのまま生成が進み、改名されたあとの別の章のフォルダに本文を書いてしまうことがあるため。この間かどうかは、画面のストア
    （`workspaceStore.activeStructureEdits`。`useStructureEdit` の `apply`・`commit` が数える。入れ子でも件数なので、内側が終わっただけでは解けない）に
    置き、工程タブの「次の工程を実行」「生成」「自動で進める」、「この文書」タブの生成・書き直し、「空の本文から書き始める」、
    ほかの構成の操作が、この数を見て無効になる（理由は「目次を変更している間は、○○できません。」）。作品を閉じたら 0 に戻る。
  - 追加は、入力のダイアログ（人物・世界観の資料・章・シーン）を見直しとみなし、「追加」で変更案を作ってそのまま適用する
    （AI パネルの変更案は通さない）。送れるのはボタンだけで、入力欄の Enter では送らない（日本語入力の確定の Enter で送らないため）。
    失敗したら理由をダイアログの中に出して、閉じない。人物の ID の欄は、利用者が触るまでは読み・名前の入力に合わせて
    `suggestCharacterId` の提案に追従し（入力が止まって少し待ってから問い合わせ、古い入力への答えは捨てる）、触ったら追従を止める。
    触っていなければ ID は送らず（`null`）、Rust に決めさせる。触って書いた ID は、送る前に Rust と同じ規則で確かめ
    （`Rin`・`霧島` など。`lib/slug.ts`）、使えなければ欄の下に理由を出して「追加」を無効にする（空欄は自動なので有効）。
    シーンの入力欄は、章立てのシーンのカード（`SceneCard`）と共有する
    （`SceneFields`）。章の追加（`AddChapterDialog`）は、章題・ストーリーライン・位置（既存の章の前か末尾。目次から開いたときは
    その位置を選んである）を入力する。途中に足すときだけ、「第 N 章以降の章は、番号が 1 つ後ろにずれます（本文のフォルダも
    一緒に移ります）」と、「シーン構成の無い章を途中に足すと、それより後ろの本文の工程は、その章のシーン構成ができるまで
    進みません」を出す。
  - 人物と世界観の資料の追加ダイアログには、上部に「自分で書く / AI に作らせる」の切り替え（`AdditionModeSwitch`。ラジオボタン。
    矢印キーでも切り替えられる）がある。開くたびに「自分で書く」から始まり、切り替えても、どちらの入力も残る。
    「AI に作らせる」では、指示の欄（必須）・開始ボタン（`GeneratedAdditionFields`。章・シーンのダイアログにも足せる部品）が出る。
    世界観の資料はファイル名（任意。自分で書くときと同じ欄）も入れられる。開始できるのはボタンだけで、指示の欄の Enter では始めない
    （日本語入力の確定の Enter で始めないため）。開始の手順（`useGeneratedAddition`）は、始められない理由が無いか確かめ
    （構成の操作の途中・自動で進め中・生成の途中や確認中・失敗の表示中は始められず、ボタンを押せなくして理由を欄の下に出す。
    `useStructureEditBlockedReason` と同じ）、開いている文書の保存を済ませ（`documentSaveController.flush`）、
    生成のセッションを `Task`（`add_character` / `add_world_document`。指示は前後の空白を除く）で始めて、ダイアログを閉じる。
    進み具合（見出しに「AI に作らせて追加: 人物」と添える。`describeTask`）と変更案の確認は AI パネルで行い、確認では、
    生成中に出た注意書き（古くなった文書の知らせ・作り直した理由。回をまたいで 1 か所にまとめる。`GenerationNotices`）を
    要約の下に見せる（工程の生成の注意書きも同じ。確認に切り替わると回ごとの表示は消えるため）。
    適用したら、作った人物資料・資料を開く（`documentCreatedBy(task, changeSet)`。変更案の、新規の書き込み）。開いている文書が
    改名される変更案のときは、その文書のまま続けるので開かない（`renamesOpenDocument`。構成の操作の適用 `commitPlan` と共通）。
    自動で進めるときの工程の生成は、何も開かない。「もう一度生成」は同じ指示で作り直す（指示を変えるには、破棄してダイアログから入れ直す）。
  - 削除は、先に変更案を作り（`planStructureEdit`）、確認のダイアログで、ゴミ箱へ移るもの（本文は「本文 N ファイル（計 X 字）も
    ゴミ箱へ移ります」と強調。章を消すときは本文のフォルダの中のファイル全部）・章を消すときに番号が変わる章（「第 3 章「…」 → 第 2 章」。
    `StructurePlan.renumbered`）・人物を消すときにその人物の名前を挙げているシーン・注意書き・ゴミ箱の場所を見せてから、
    「ゴミ箱へ移す」で適用する。確認している間に作品が外で変わって競合したら、削除せず「作品が変わったため削除しませんでした」と
    出し、「もう一度確かめる」で変更案を作り直せる。
  - 並べ替えは、元に戻せる操作なので、確認を挟まず、変更案を作ってそのまま適用する（`useEntryMove`。追加と同じ `useStructureEdit.apply`）。
    行き先（`position`。並べ替えたあとに一覧の何番目に来るか）は、隣の項目と入れ替わる位置として画面が決める
    （`entryActionsFor`。章は番号順の章の一覧、シーンは章のシーンの一覧、人物は目次の人物の一覧の中で数え、あらすじなど
    同じ節の別の種類の行は数えない）。章の一覧は、本文の章見出しからでも、プロットの節の章（YAML が読めない章立ても含む）で数える。
    本文の節は読めない章を出さないが、本物の `MoveChapter` は壊れた章も含む全部の章（番号順）の中で位置を数えるので、
    本文の節の並びで数えると位置がずれる（例: 01 が壊れていると、本文の「第 2 章」の「下へ」が必ず範囲外になる）。
    YAML が読めない人物資料は動かせず、メニューに出さない。隣が読めない人物資料のときは、
    それを飛ばして、その先の読める人物と入れ替える（本物は読めない資料の `order` を変えずに飛ばす。読める人物が末尾なら「下へ」は出さない）。
    失敗したら、ダイアログが無いので、理由をエラーのトーストで知らせる。適用が終わって目次が変わるまでは、続けて押された操作が
    古い目次の位置で変更案を作らないよう、追加・削除・並べ替えを無効にする（理由は「目次を変更している間は、追加・削除・並べ替えできません。」。
    上の、生成を止めるのと同じ印）。変更案に注意書き（`notices`。「読めない人物資料は後ろに並べたまま」など）があれば、
    要約のトーストに続けて、1 件ずつトーストで知らせる（確認のダイアログを通らない操作は、ダイアログで見せられないため。
    削除はダイアログが先に見せるので、適用後には出さない）。
    開いている文書の扱いは追加・削除と同じ。章の移動で改名される文書は新しいパスへ付け替えて続け（`relocatedPath`）、
    人物資料とシーンの並べ替えで書き換えられる文書（人物資料の `order`・章立て）は読み直す（未保存の編集が残っていれば止める）。
  - 適用したあと（`useStructureEdit.commit`）は、目次と工程を直し、作った文書（`created`）を開いて、変更案の要約をトーストで知らせる。
    開いている文書との食い違いを防ぐ手順は変更案の適用と同じ（`writeBesideEditor`）。開いている文書がゴミ箱へ移ったときは、
    読み直さずに閉じて知らせる。
  - 開いている文書が、章の番号の振り直しで改名されるとき（`manuscript/03/s01.txt` を開いたまま第 2 章を消すと
    `manuscript/02/s01.txt` になる。章立てのファイルも同じ）は、中身が変わらないので読み直さず、エディタのパス（`editorStore.relocate`。
    中身・版番号・保存の基準のハッシュ・保存状態はそのまま）と目次の選択を新しいパスへ付け替え、そのまま続けて編集できるようにする。
    次の自動保存は新しいパスへ書く（古いパスには書かない。移動で空いた場所に別の章の文書があるのを壊さないため）。
    新しいパスは、変更案の移動（フォルダの下のパスは前の部分を置き換える）から `relocatedPath` が求める。内容を書き換える文書だけを
    読み直す（`rewritesPath`）。改名される文書を開いていたときは、利用者が編集していた文書のまま続けられるよう、作った文書（`created`）は開かない。
    保存できない編集が残っていれば、改名する前に止める。
  - 書き換えている間（`write` から、読み直し・パスの付け替えが終わるまで）は、エディタの保存の開始を止める
    （`documentSaveController.holdSaves`）。保存先のパスは保存を始めるときに決まるので、その間の入力や Ctrl+S が改名の前の
    パスへの保存になると、書き込みは適用の終わりまで待たされたあとに古いパスへ向かい、そこに別の章の文書があれば競合になって、
    「上書きする」でその文書を利用者の文章で潰してしまう。止めた保存は、解いたときのパスと文書で 1 回だけ行う。
    止めている間の `flush`（文書の切り替えなど）は、解けて保存が済むまで待つ（未保存の編集を失わないため）。
- まだ無い本文のシーンを選んだときの「この文書はまだ生成されていません。」に、「空の本文から書き始める」ボタンを出す。
  空の本文を新規に作って開くので、足したシーンを、生成を待たずに自分で書き始められる。
- フォームは、利用者が触っていない項目を読んだままの値で送り返す（Rust が項目の変更を見分けて YAML を書き直すかを決めるため）。
  編集中の文書には版番号（`revision`）を付け、保存中や、変更案の適用・設定の保存による書き換えの間に編集されたかを、
  文書の中身の比較ではなく版番号で判定する。
- 保存のたびに目次と工程を読み直すわけではない。読み直すのは、目次の見出しや工程の名前に出る項目が変わったときだけ
  （人物資料・章立ての項目が、読み込み時または前回の保存時から変わった。または、人物資料・章立てのパスを文字列
  `text` として保存した＝壊れた YAML を直した）。本文だけの変更は読み直さない。読み直しは保存の直列化の外で
  1 つずつ行うので、遅くても失敗しても、文書の切り替えなどのための保存の完了待ち（`flush`）を待たせない。
  失敗したときは知らせるだけで、保存は済んでいる。読み直している間に作品が閉じられたら、結果は捨てる。
- シーンは `id` ではなく並びの位置で特定して更新する（`id` が重複した章立てでも、直したシーンだけが変わるように。
  Rust は重複した `id` の章立てをフォームで開かせず、文字列で開く）。数値の項目は、Rust の型（`u32`）の範囲を超える入力を
  数字でない入力と同じに扱い、文書に反映しない。任意の文字列の項目は、完全に空欄のときだけ `null` にする
  （空白だけの入力は文字列のまま）。
- 起動オプション `--settings=<PATH>`（`--settings <PATH>` も可）で、既定の場所の代わりに使う設定ファイルを指定できる
  （設定ファイルの場所を指定する点は CLI と同じ。ただし指定したファイルが無い場合、CLI はエラーにするのに対し、
  GUI は既定値から始める）。E2E テストが利用者の設定に触れずに動くためにも使う。

### 5.1 IPC の契約

画面側のインターフェースは [src/api/backend.ts](../src/api/backend.ts)、型は [src/api/types.ts](../src/api/types.ts)。
Tauri コマンド名と引数（JS 側の名前。Rust 側は snake_case で受ける）は次のとおり。

| Backend のメソッド | コマンド | 引数 |
|---|---|---|
| loadSettings / saveSettings | `load_settings` / `save_settings` | — / `settings` |
| loadProjectSettings / saveProjectSettings | `load_project_settings` / `save_project_settings` | — / `settings, expectedHash`（読んだときのハッシュ。変わっていれば `conflict`） |
| setApiKey / hasApiKey | `set_api_key` / `has_api_key` | `apiKey` / — |
| listModels / listGenres | `list_models` / `list_genres` | `llm`（省略可。保存前の接続先で試す）/ — |
| createProject / openProject / closeProject | `create_project` / `open_project` / `close_project` | `folder, project` / `folder` / — |
| overview / pipeline | `overview` / `pipeline` | — |
| readDocument / writeDocument | `read_document` / `write_document` | `path` / `path, document, expectedHash`（`document` は `EditableDocument`。`kind` が `text`・`character`・`chapter` のどれか。`read_document` は `{ document, hash, parse_error }`、`write_document` は新しいハッシュを返す。パスの種類に合わない文書と、シーンの `id` が重複した章立ては `invalid_input`） |
| parseDocument | `parse_document` | `path, content`（作品を開いていなくてもよい。ファイルは読み書きしない。`{ document, parse_error }` を返す。分け方は `read_document` と同じ。`path` が作品内の相対パスとして不正なら `invalid_input`） |
| textStats / parseRuby / analyzeQuality | `text_stats` / `parse_ruby` / `analyze_quality` | `text` / `text` / `text, targetChars` |
| generate / cancelGeneration | `generate` / `cancel_generation` | `jobId, task, onEvent`（`Channel<GenerationEvent>`）/ `jobId`（`task` の `kind` に、工程の一覧には出ない `revise`・`add_character`・`add_world_document` がある。`add_character` は `instruction`、`add_world_document` は `instruction` と `name`（文字列か `null`）を持つ） |
| applyChangeSet | `apply_change_set` | `changeSet`（`files` の各要素は `kind` が `write`・`trash`・`move`・`expect` のどれか。構成の変更もこれで適用する） |
| planStructureEdit | `plan_structure_edit` | `edit`（`StructureEdit`。`kind` が `add_character`・`remove_character`・`add_world_document`・`remove_world_document`・`add_chapter`・`remove_chapter`・`add_scene`・`remove_scene`・`move_character`・`move_chapter`・`move_scene` のどれか。`add_character` の `id` は文字列か `null`、`remove_character`・`move_character` は ID ではなく `path` で指す。`move_*` の `position` は、並べ替えたあとにその項目が一覧の何番目に来るか（0 始まり）で、範囲外・今と同じ位置は `invalid_input`）。`StructurePlan`（`change_set`・`completed_summary`・`created`・`references`・`renumbered`・`notices`）を返す。作品フォルダは書き換えない（§4.8）。入力の誤り・消せない資料は `invalid_input`、対象が無ければ `not_found` |
| suggestCharacterId | `suggest_character_id` | `reading, name`。人物の ID の案（文字列）を返す。使用済みの ID は避ける |

コマンドの失敗は `{ kind: BackendErrorKind, message: string }` で返り、画面側で `BackendError` に変換する。
作品を開いていない状態で作品の操作を呼んだときは `not_found`。
`apply_change_set` に渡した変更案の形が正しくない（同じパスへの変更の重なり・ゴミ箱へ移せない・改名できないパス・フォルダを自分の中へ移す変更・`Trash` の形の誤り）ときは `invalid_input`。
フォルダ選択は `@tauri-apps/plugin-dialog` の `open({ directory: true })` を画面側から直接呼ぶ。

### 5.2 E2E テスト

本物のアプリ（WebView2 と Rust のバックエンド）を WebDriver（msedgedriver）で操作する。`e2e/` に置き、`pnpm e2e` で動かす。
一時フォルダに作品と設定ファイルを作り、`--settings <PATH>` を付けて起動するので、利用者の設定や作品には触れない。

- アプリはテストが自分で起動し、WebView2 のデバッグ用ポートを開かせてから、msedgedriver をそのポートに接続させる
  （`debuggerAddress`）。msedgedriver にアプリを起動させる方式（tauri-driver）は、ポートなどの指定を環境変数で渡すが、
  管理者として動く CI では WebView2 がその環境変数（`WEBVIEW2_*`）を無視するため使わない。
- ポートは、手元では環境変数 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`、CI ではコンピューター単位のポリシー
  （`HKLM\SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments`）で渡す。

## 6. ヘッドレス実行（kataribe-cli）

GUI と同じ engine を使い、画面なしで作品を作る・生成する・検査する。GUI と同じ設定ファイルを読み、
作品フォルダを受け取るサブコマンドではその作品の設定（§4.6）を重ね、コマンドラインの指定で上書きできる。`main.rs` は薄くし、処理は `lib.rs` の `run` に置いてテストできるようにする。

```
kataribe-cli [グローバルオプション] <サブコマンド>

グローバルオプション（設定ファイルの値を上書き）
  --settings <PATH>          設定ファイル（既定: GUI と同じ場所。明示したのに無ければエラー）
  --provider <openai-compatible|claude-code>
  --base-url <URL>  --claude-command <PATH>
  --model <ID>               選んでいる接続先のモデル（Claude Code なら sonnet / opus / haiku など）
  --api-key-env <VAR>        API キーを読む環境変数（既定 KATARIBE_API_KEY。未設定なら資格情報マネージャーのキーを使う）
  --unit <chapter|scene|beat>  --chars-per-call <N>  --context-tokens <N>
  --temperature <T>  --polish | --no-polish  --quality-retries <N>
  -q, --quiet                生成中の本文を表示しない

サブコマンド
  new <FOLDER> --title <T> [--author <A>] [--genre <ID>] [--genre-note <T>]
               [--rating general|r15|r18] [--length <N>] (--idea <TEXT> | --idea-file <PATH>)
      --length は省略でき、既定は 30,000 字（GUI の新規作成と同じ値）
  status <FOLDER> [--json]            工程の状態と文字数
  generate <FOLDER> <TASK> [--instruction <TEXT>] [--name <SLUG>] [--dry-run]
      TASK = concept | style | world | cast | character:<id> | synopsis | outline
           | scenes:<NN> | draft:<NN>/<sNN> | revise:<path> | add-character | add-world
      add-character は指示から人物を 1 人、add-world は指示から世界観の資料を 1 つ作って足す（§4.8）。
      --instruction は revise:<path>・add-character・add-world のときだけ付けられ（付けないと使い方の誤り）、
      それ以外に付けると使い方の誤りにする
      --name は add-world のときだけ付けられる（world/<SLUG>.md のファイル名。省略すると題から決める）。
      それ以外に付けると使い方の誤りにする
      --dry-run は原稿と資料を書き換えない（要約などの中間データのキャッシュは更新する）
  run <FOLDER> [--until <STAGE>] [--max-steps <N>]
      取りかかれる工程（ready）を順に生成・適用し続ける。ready が無くなるか上限で止まる。
      --until があるときは、それより後ろの段階の工程を候補にしない
      （候補が無くなり、かつ完了していない工程が残っていれば行き詰まりとして失敗にする）。
      STAGE = concept | style | world | cast | characters | synopsis | outline | scenes | draft
  add character <FOLDER> --name <T> [--reading <かな>] [--role <T>] [--summary <T>] [--order <N>] [--id <ID>]
                         [--body <T> | --body-file <PATH>] [--dry-run]
  add world     <FOLDER> --title <T> [--name <SLUG>] [--body <T> | --body-file <PATH>] [--dry-run]
  add chapter   <FOLDER> --title <T> [--storyline <T> | --storyline-file <PATH>] [--before <NN>] [--dry-run]
      章を足す。--before <NN> でその章の前に入り、その番号以降の章は 1 つ後ろへずれる（省略すると末尾）
  add scene     <FOLDER> <NN> --title <T> [--summary <T>] [--pov <名前>] [--characters <A,B>] [--place <T>]
                         [--time <T>] [--target-chars <N>] [--before <sNN>] [--dry-run]
      人物・世界観の資料・章・シーンを、自分で書いて足す（LLM は使わない。§4.8）。
      人物の --id を省くと、読み（無ければ名前）からローマ字で決める。--id が使えない文字列なら、エンジンが理由を返す。
      世界観の --name を省くと題から決める（§2）
  remove <FOLDER> <TARGET> [--dry-run]
      TARGET = character:<id> | world:<name または path> | chapter:<NN> | scene:<NN>/<sNN>
      ゴミ箱（.kataribe/trash/）へ移して消す。chapter:<NN> は章立てと本文のフォルダ（中のファイルごと）を移し、
      後ろの章の番号を 1 つずつ前へずらす。書式は generate の TASK と同じ。character:<id> は characters/<id>.md の人物資料を
      指す（ID の規則は確かめないので、手で足した character:Rin のような名前も指定できる）
  move <FOLDER> <TARGET> --to <N> [--dry-run]
      TARGET = character:<id> | chapter:<NN> | scene:<NN>/<sNN>
      人物・章・シーンの順番を変える（LLM は使わない。§4.8）。N は 1 始まりの位置（並べ替えたあとに、その項目が
      人物（目次の人物の中）・章・章のシーンの何番目に来るか）。範囲外・今と同じ位置は使い方の誤りではなく失敗（終了コード 1）にする
      （0 以下・数字でない・world: は使い方の誤り）。character:<id> は remove と同じく characters/<id>.md を指す。
      chapter:<NN> は、動く範囲の章の番号を振り直す（章立てと本文のフォルダを改名する）。
      character: は読める人物の order を 1, 2, 3… に振り直して書き直す（YAML は書き直され、コメントは残らない）
  quality <FOLDER> [--json]           シーンごとの品質レポート
  export <FOLDER> [--output <FILE>] [--force]
      本文を章題付きの一つのテキストにまとめる。--output は作品フォルダの外を指定すること
      （中を指すと拒否する）。既存ファイルへの上書きは --force を指定したときだけ許す
  models                              選べるモデルの一覧（接続の確認を兼ねる）
  project-settings <FOLDER> [--save]  作品ごとの設定（§4.6）と、実際に使う設定を表示する。
                                      --save で、グローバルオプションで指定した値（--provider・--model・
                                      --unit・--chars-per-call・--context-tokens・--temperature・--polish・
                                      --no-polish・--quality-retries）を作品に保存する（--model は保存後に使う
                                      接続先のモデル。数値は使うときと同じ範囲に収める。保存できる項目を
                                      指定しなければ何も変えない）
  api-key set | clear | status        API キーを資格情報マネージャーに保存・削除・確認
                                      （set は標準入力から読む。端末から直接入力すると
                                       画面にそのまま表示されるので、表示したくなければ
                                       パイプで渡す）
```

- `add` / `remove` / `move` は `Project::open` だけで動き（LLM も設定ファイルも使わない）、GUI のような確認の手順は挟まない。
  代わりに、適用の前に必ず標準エラー出力へ、ゴミ箱へ移るもの（フォルダは中のファイルと文字数）・番号が変わる章
  （「第3章「雨の匂い」 → 第2章」）・その人物の名前を挙げているシーン・注意書きを出す。
  `--dry-run` なら、変更案（書き込む内容・ゴミ箱へ移すものの一覧・「移動: A → B」）を標準出力に出して、何も書かない
  （状態の確認は何も変えないので出さない）。
  適用したあとは、書き込んだファイルを「書き込み: <パス>」、ゴミ箱へ移したものを「ゴミ箱へ: <パス>」、改名したものを
  「移動: A → B」と標準エラー出力に出す（`generate` / `run` も同じ出し分け）。
- `generate add-character` / `add-world` の注意書き（古くなった工程など）は、ほかの生成と同じく標準エラー出力に出る。
  適用と `--dry-run` も `generate` のとおり。足す名前の誤り（`--name` が規則に合わない・使用済み）は、LLM を呼ぶ前に失敗にする。
- API キーは `--api-key-env` の環境変数 → 資格情報マネージャー（GUI と共有、`kataribe_engine::ApiKeyStore`）の順に探す。
  Claude Code のときは API キーを使わないので探さない。

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
