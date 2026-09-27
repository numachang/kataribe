# kataribe（語り部）

LLM と一緒に日本語の小説を書くための Windows 向けエディタ。

名前の由来は「語り部」。書き手のそばで LLM が物語の語り手として執筆を手伝う、というイメージから。

## 特徴

- **設定資料から本文まで、工程を順に進める。** 企画 → 文体ガイド → 世界観 → 登場人物 → あらすじ → 章立て → シーン構成 → 本文。
  どの資料も LLM に下書きさせ、直接直すか、指示を出して書き直させる。
- **ローカル LLM でも崩れにくい。** 本文は「章」「シーン」「ビート（シーンの中の展開）」のどの単位でも書ける。
  1 回に書かせる分量と、モデルに渡す文脈の長さも設定で変えられる。
- **作品はテキストファイルだけのフォルダ。** 設定資料は Markdown、本文はプレーンテキスト（カクヨム・なろう互換のルビ・傍点記法）。
  そのまま Git で管理できる。上書きの前にはバックアップを取り、外部で変更されたファイルは上書きしない。
- **縦書き・ルビのプレビュー。** 本文はエディタで縦書きのまま書ける。
- **LLM は OpenAI 互換 API ならどれでも。** LM Studio・Ollama・KoboldCpp・OpenRouter・OpenAI など。API キーは Windows の資格情報マネージャーに保存する。
- **Claude Code でも書ける。** Claude Code（`claude -p`）にログインしていれば、API キーなしで Claude に書かせられる。
- **画面なしでも同じことができる。** CLI（`kataribe-cli`）は GUI と同じ執筆エンジンを使う。

## 状態

開発中（0.1.0）。仕組みの全体は [docs/architecture.md](docs/architecture.md) を参照。

## 動作環境

- Windows 10 / 11（64 bit）。WebView2 ランタイム（Windows 11 には最初から入っている）。
- OpenAI 互換 API の LLM サーバー、または Claude Code（ログイン済みのもの）。

### おすすめの設定（ローカル LLM）

VRAM 12GB の環境で実際に短編ミステリを書かせて比べた結果、次の組み合わせが最も安定した（生成単位の既定もビート）。

| 項目 | 値 |
|---|---|
| モデル | `google/gemma-4-12b-qat`（LM Studio） |
| 本文の生成単位 | ビート |
| 1 回あたりの文字数 | 1,500 字 |
| 文脈の長さ | 16,384 トークン |

推論モデル（Qwen 系・Gemma 系など）は、何も指定しないと「思考」に出力を使い切って本文が空になることがある。
既定の「思考を止める」設定のままで使う。クラウドの API でこの指定がエラーになる場合は、設定で切る。

### Claude Code で書く

Claude Code をインストールしてログインしておけば、
設定の「LLM」で「Claude Code」を選ぶだけで使える。API キーはいらず、ログインしているアカウントの利用枠を使う。

- モデルは `sonnet`（既定）・`opus`・`haiku` などの別名で選ぶ。
- `claude` コマンドが PATH に無ければ、設定で実行ファイルの場所を指定する（npm で入れた場合は `claude.cmd`）。
- 「接続テスト」は `claude auth status` でログインしているかを確かめるだけで、利用枠は使わない。
- 呼び出しのたびにツール・MCP サーバー・Claude Code の設定を読み込まない状態で起動し、会話の履歴も残さない。
- temperature と「思考を止める」は使われない。長い文脈を扱えるので、文脈の長さを 100,000 トークン程度まで増やせる。

## インストール

リリースはまだ無い。main の CI（GitHub Actions）が作るインストーラ（成果物 `kataribe-installer`）を使うか、
ソースからインストーラを作る（「開発」を参照）。CI の「Run workflow」から手動で作ることもできる。
`pnpm tauri build` で `target/release/bundle/nsis/kataribe_<版>_x64-setup.exe` ができる。ユーザー単位でインストールされ、管理者権限はいらない。

## 使い方

### アプリ

1. 「新しい作品」で、題名・ジャンル・目標の文字数・企画の種（どんな話にしたいか）を入れ、保存先のフォルダを選ぶ。
2. 「設定」で LLM の接続先を選ぶ。OpenAI 互換 API ならサーバーの URL とモデル（API キーが要るサーバーならキーも）、
   Claude Code ならモデルを選ぶ。
3. 右の「工程」タブで「次の工程を実行」を押すと、次に取りかかれる資料や本文を生成する。
   生成した内容は変更案として表示されるので、確かめてから適用する。「自動で進める」で続けて生成もできる。
4. 左の目次から資料や本文を開いて、直接書き直す。入力が止まって 1 秒後、または Ctrl+S で保存される。

### CLI

```powershell
# 作品を作る
kataribe-cli new my-novel --title "みさき館の殺人" --genre mystery --length 12000 `
  --idea "嵐で孤立した岬の洋館で、館の主が密室で死ぬ。盲目の少女探偵が音で嘘を聞き分ける。"

# 本文まで、取りかかれる工程を順に生成して適用する
kataribe-cli --model google/gemma-4-12b-qat --unit beat run my-novel --until draft

# Claude Code で書く場合
kataribe-cli --provider claude-code --model sonnet --context-tokens 100000 run my-novel --until draft

# 状態・品質を確かめ、本文を 1 つのテキストにまとめる
kataribe-cli status my-novel
kataribe-cli quality my-novel
kataribe-cli export my-novel --output my-novel.txt
```

設定は GUI と同じファイル（`%APPDATA%\io.github.numachang.kataribe\settings.json`）を読み、コマンドラインの指定で上書きできる。
API キーは環境変数 `KATARIBE_API_KEY`、無ければ資格情報マネージャー（`kataribe-cli api-key set` で保存）から読む
（Claude Code のときは読まない）。`models` でモデルの一覧を出すと、接続できるかも確かめられる。
1 つの工程だけを生成するときは `generate`（例: `generate my-novel draft:01/s02`）、適用せずに変更案だけ見るときは `--dry-run`。
`export` は作品フォルダの外にだけ書き出し、既にあるファイルは `--force` を付けたときだけ上書きする。
`new --length` を省略すると目標は 30,000 字になる。
ジャンルの一覧は [presets/genres.yaml](crates/kataribe-engine/presets/genres.yaml) にある。

## 作品フォルダ

```
my-novel/
├─ kataribe.yaml        作品情報（題名・ジャンル・目標文字数・企画の種）
├─ concept.md           企画
├─ style.md             文体ガイド
├─ world/overview.md    世界観
├─ characters/*.md      登場人物（1 人 1 ファイル）
├─ plot/synopsis.md     あらすじ
├─ plot/chapters/01.md  章のストーリーラインとシーン構成
└─ manuscript/01/s01.txt  本文（シーンごと）
```

アプリの内部データ（バックアップ・ゴミ箱・要約のキャッシュ）は `.kataribe/` に置かれ、Git の管理から外れる。詳しくは [docs/architecture.md §2](docs/architecture.md)。

## 開発

必要なもの: Rust（1.95 以上）、Node.js 24、pnpm。

| 目的 | コマンド |
|---|---|
| アプリの起動（開発） | `pnpm install` → `pnpm tauri dev` |
| 画面だけ（偽のバックエンドで動く） | `pnpm dev` |
| Rust のテスト / lint / 整形 | `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all` |
| 画面のテスト / 型検査 / lint | `pnpm test` / `pnpm typecheck` / `pnpm lint` |
| TypeScript の型定義の再生成 | `pnpm bindings`（Rust の型から `src/bindings/` を作る） |
| インストーラの作成 | `pnpm tauri build` |
| CLI | `cargo run -p kataribe-cli -- --help` |

コードの方針は [CLAUDE.md](CLAUDE.md) を参照。

### E2E テスト

本物のアプリ（WebView2 と Rust のバックエンド）を WebDriver（msedgedriver）で操作する。
一時フォルダに作品と設定ファイルを作ってアプリを起動し、WebView2 のデバッグ用ポートに msedgedriver を接続させる。
自分の設定や作品には触れない。

1. インストール済みの WebView2 と同じ版の [Microsoft Edge WebDriver](https://developer.microsoft.com/microsoft-edge/tools/webdriver/)（`msedgedriver.exe`）を用意する。
   WebView2 の版は、レジストリの `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` の `pv` で分かる。
2. 画面を埋め込んだアプリを作る: `pnpm tauri build --debug --no-bundle`
3. `pnpm e2e`（msedgedriver が PATH に無ければ、環境変数 `KATARIBE_E2E_MSEDGEDRIVER` に場所を指定する）

テスト中に撮った画面は `e2e/artifacts/` に保存される。
CI（GitHub Actions）でも、ランナーの WebView2 と同じ版の msedgedriver を取得して同じテストを動かす。
CI はアプリを管理者として動かすため WebView2 が環境変数を無視するので、デバッグ用ポートはポリシーで開く。

## ライセンス

[MIT](LICENSE)
