# kataribe への協力

## issue（不具合の報告・要望）

不具合の報告と機能の要望は、[issue](https://github.com/numachang/kataribe/issues) で受け付ける。

不具合を報告するときは、次のことを書いてほしい。

- 使った kataribe の版（インストーラのファイル名にある版）。ソースから作った場合はコミット
- Windows の版
- LLM の接続先の種類とモデル（例: LM Studio の `google/gemma-4-12b-qat`、Claude Code の `sonnet`）
- 何をしたら何が起きたか。本来はどうなってほしかったか
- エラーの文言や画面、ログ（`%LOCALAPPDATA%\io.github.numachang.kataribe\logs\kataribe.log`）

issue は誰でも読める。API キー、接続先 URL に書いた認証情報、公開したくない作品の本文などは、消してから貼ってほしい。

## プルリクエスト

今は一人で設計を決めながら開発しているので、プルリクエストは原則として受け付けていない。
直し方の案があれば、issue に書いてもらえると助かる。

## セキュリティの問題

公開の issue には書かず、[SECURITY.md](SECURITY.md) の方法で知らせてほしい。
