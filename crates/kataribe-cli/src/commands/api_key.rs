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
        console.eprint("注意: 依存を増やしていないため、入力はそのまま画面に表示されます。\n");
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
    console.print("API キーを保存しました。\n");
    Ok(Outcome::Success)
}

fn clear(console: &dyn Console) -> anyhow::Result<Outcome> {
    ApiKeyStore::default()
        .delete()
        .context("API キーを削除できません")?;
    console.print("API キーを削除しました。\n");
    Ok(Outcome::Success)
}

fn status(console: &dyn Console) -> anyhow::Result<Outcome> {
    let has_key = ApiKeyStore::default()
        .load()
        .context("API キーの設定状況を確認できません")?
        .is_some();
    if has_key {
        console.print("API キーは設定されています。\n");
    } else {
        console.print("API キーは設定されていません。\n");
    }
    Ok(Outcome::Success)
}
