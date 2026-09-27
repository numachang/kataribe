//! `api-key` サブコマンド: API キーを資格情報マネージャーに保存・削除・確認する。
//!
//! キーの値そのものは、保存のとき以外は決して表示・ログ出力しない。

use std::io::{BufRead, IsTerminal};

use anyhow::Context;
use kataribe_engine::ApiKeyStore;

use crate::args::{ApiKeyAction, ApiKeyArgs};
use crate::output::Console;

use super::Outcome;

pub fn run(args: &ApiKeyArgs, console: &dyn Console) -> anyhow::Result<Outcome> {
    match args.action {
        ApiKeyAction::Set => set(console),
        ApiKeyAction::Clear => clear(console),
        ApiKeyAction::Status => status(console),
    }
}

fn set(console: &dyn Console) -> anyhow::Result<Outcome> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        console
            .eprint(
                "注意: 端末から直接入力すると、入力したキーがそのまま画面に表示されます。\n\
                 表示したくない場合は、パイプで渡してください\n\
                 （例: Get-Content key.txt | kataribe-cli api-key set）。\n",
            )
            .context("標準エラー出力への書き込みに失敗しました")?;
    }
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("標準入力から API キーを読み取れません")?;
    let key = line.trim();
    if key.is_empty() {
        anyhow::bail!("API キーが入力されませんでした。");
    }
    ApiKeyStore::default()
        .save(key)
        .context("API キーを資格情報マネージャーに保存できません")?;
    console
        .print("API キーを保存しました。\n")
        .context("標準出力への書き込みに失敗しました")?;
    Ok(Outcome::Success)
}

fn clear(console: &dyn Console) -> anyhow::Result<Outcome> {
    ApiKeyStore::default()
        .delete()
        .context("API キーを削除できません")?;
    console
        .print("API キーを削除しました。\n")
        .context("標準出力への書き込みに失敗しました")?;
    Ok(Outcome::Success)
}

fn status(console: &dyn Console) -> anyhow::Result<Outcome> {
    let has_key = ApiKeyStore::default()
        .load()
        .context("API キーの設定状況を確認できません")?
        .is_some();
    let message = if has_key {
        "API キーは設定されています。\n"
    } else {
        "API キーは設定されていません。\n"
    };
    console
        .print(message)
        .context("標準出力への書き込みに失敗しました")?;
    Ok(Outcome::Success)
}
