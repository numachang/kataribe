# kataribe 開発ガイド

LLM と一緒に日本語の小説を書く Windows 向けエディタ。Tauri 2（Rust）＋ React（TypeScript）。
設計の全体像は [docs/architecture.md](docs/architecture.md) を読むこと。

## 第一原則：きれいなコード

- **読み手のために書く。** 名前だけで意図がわかるようにする。略語・一文字変数（ループ添字を除く）は使わない。
- **一つの関数・型・モジュールは一つの責務。** 関数は短く、抽象度をそろえる。深いネストは早期 return で平らにする。
- **重複させない。** ただし、たまたま似ているだけのコードを無理に共通化しない。
- **境界をはっきりさせる。** crate / モジュールの公開 API は最小限にし、内部は `pub(crate)` 以下に閉じる。
- **エラーを握りつぶさない。** ライブラリ crate は `thiserror` で意味のあるエラー型を返す（`anyhow` はバイナリのみ）。本番コードで `unwrap` / `expect` / `panic!` を使わない（テストは可）。
- **コメントは「なぜ」を書く。** 「何を」はコードで表す。公開 API には短い doc コメント（日本語）を付ける。
- **死んだコード・TODO の放置・デバッグ出力を残さない。**
- **テストを書く。** 振る舞いを外から検証する。正常系・境界値・異常系を押さえる。テスト名は何を保証するかを表す。
- **識別子は英語、コメント・doc・ユーザー向け文言は日本語。**

## コマンド

| 目的 | コマンド |
|---|---|
| Rust テスト | `cargo test --workspace` |
| Rust lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust 整形 | `cargo fmt --all` |
| TS テスト | `pnpm test` |
| TS 型検査 / lint | `pnpm typecheck` / `pnpm lint`（整形は `pnpm format`） |
| TS 型定義の再生成 | `pnpm bindings`（ts-rs → `src/bindings/`） |
| アプリ起動（開発） | `pnpm tauri dev` |
| インストーラ作成 | `pnpm tauri build` |
| ヘッドレス実行 | `cargo run -p kataribe-cli -- --help` |

変更を終える前に、触った範囲のテスト・clippy・lint がすべて通ることを確認する。

## リポジトリ構成

```
crates/kataribe-text     文字数・ルビ・表記整形・品質指標（依存なし）
crates/kataribe-llm      LLM クライアント（OpenAI 互換 API・Claude Code。依存なし）
crates/kataribe-project  作品フォルダ・安全なファイル操作・設定資料モデル（依存なし）
crates/kataribe-engine   プロンプト・文脈構築・生成工程（上の 3 つに依存）
crates/kataribe-cli      ヘッドレス CLI（engine を使う）
src-tauri                デスクトップアプリ（engine を Tauri コマンドとして公開）
src                      フロントエンド（React + TypeScript）
```
