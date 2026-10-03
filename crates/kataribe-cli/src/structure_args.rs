//! `add` / `remove` / `move` サブコマンドの引数（clap）。作品の構成（人物・世界観の資料・章・シーン）を自分で足したり消したり並べ替えたりする。

use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use kataribe_project::{ChapterId, SceneId};

use crate::target_spec::{MoveTarget, RemoveTarget};

#[derive(Debug, Args)]
pub struct AddArgs {
    #[command(subcommand)]
    pub target: AddTarget,
}

#[derive(Debug, Subcommand)]
pub enum AddTarget {
    /// 登場人物を足す（characters/<id>.md を作る）。
    Character(AddCharacterArgs),
    /// 世界観の資料を足す（world/<name>.md を作る）。
    World(AddWorldArgs),
    /// 章を足す（後ろの章の番号を振り直し、plot/chapters/<NN>.md を作る）。
    Chapter(AddChapterArgs),
    /// 章にシーンを足す（章立てを書き直す）。
    Scene(AddSceneArgs),
}

/// 本文は、直接の文章かファイルのどちらか一方で指定する（どちらも省略すると空）。
#[derive(Debug, Args)]
#[group(multiple = false)]
pub struct BodySource {
    /// 本文。
    #[arg(long, value_name = "TEXT")]
    pub body: Option<String>,

    /// 本文を書いたファイル。
    #[arg(long, value_name = "PATH")]
    pub body_file: Option<PathBuf>,
}

/// ストーリーラインは、直接の文章かファイルのどちらか一方で指定する（どちらも省略すると空）。
#[derive(Debug, Args)]
#[group(multiple = false)]
pub struct StorylineSource {
    /// ストーリーライン（章立ての本文）。
    #[arg(long, value_name = "TEXT")]
    pub storyline: Option<String>,

    /// ストーリーラインを書いたファイル。
    #[arg(long, value_name = "PATH")]
    pub storyline_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AddCharacterArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 名前。
    #[arg(long, value_name = "TEXT")]
    pub name: String,

    /// 読み（かな）。ID を自動で決めるときに、ローマ字にして使う。
    #[arg(long, value_name = "かな")]
    pub reading: Option<String>,

    /// 役回り（主人公、探偵、など）。
    #[arg(long, value_name = "TEXT")]
    pub role: Option<String>,

    /// 一言紹介。
    #[arg(long, value_name = "TEXT")]
    pub summary: Option<String>,

    /// 表示順（省略すると末尾）。
    #[arg(long, value_name = "N")]
    pub order: Option<u32>,

    /// ID（ファイル名。省略すると読み、無ければ名前からローマ字で決める）。
    #[arg(long, value_name = "ID")]
    pub id: Option<String>,

    #[command(flatten)]
    pub body: BodySource,

    /// 変更案を表示するだけで、書き込まない。
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct AddWorldArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 題（本文の先頭の見出しになる）。
    #[arg(long, value_name = "TEXT")]
    pub title: String,

    /// ファイル名（英小文字・数字・ハイフン。拡張子は付けない。省略すると題から決める）。
    #[arg(long, value_name = "SLUG")]
    pub name: Option<String>,

    #[command(flatten)]
    pub body: BodySource,

    /// 変更案を表示するだけで、書き込まない。
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct AddChapterArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 章題。
    #[arg(long, value_name = "TEXT")]
    pub title: String,

    #[command(flatten)]
    pub storyline: StorylineSource,

    /// この章の前に足す。その番号以降の章は、番号が 1 つ後ろへずれる（省略すると末尾）。
    #[arg(long, value_name = "NN")]
    pub before: Option<ChapterId>,

    /// 変更案を表示するだけで、書き込まない。
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct AddSceneArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// シーンを足す章の番号。
    #[arg(value_name = "NN")]
    pub chapter: ChapterId,

    /// シーン題。
    #[arg(long, value_name = "TEXT")]
    pub title: String,

    /// このシーンの要約。
    #[arg(long, value_name = "TEXT")]
    pub summary: Option<String>,

    /// 視点人物の名前。
    #[arg(long, value_name = "名前")]
    pub pov: Option<String>,

    /// 登場人物の名前（カンマ区切り）。
    #[arg(long, value_name = "A,B", value_delimiter = ',')]
    pub characters: Vec<String>,

    /// 場所。
    #[arg(long, value_name = "TEXT")]
    pub place: Option<String>,

    /// 時間。
    #[arg(long, value_name = "TEXT")]
    pub time: Option<String>,

    /// 目標文字数。
    #[arg(long, value_name = "N")]
    pub target_chars: Option<u32>,

    /// このシーンの前に足す（省略すると章の末尾）。
    #[arg(long, value_name = "sNN")]
    pub before: Option<SceneId>,

    /// 変更案を表示するだけで、書き込まない。
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 消すもの（ゴミ箱 .kataribe/trash/ へ移す）。
    ///
    /// character:<id> | world:<name または path> | chapter:<NN> | scene:<NN>/<sNN>
    #[arg(value_name = "TARGET")]
    pub target: RemoveTarget,

    /// 変更案を表示するだけで、何も移さない。
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MoveArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 並べ替えるもの。
    ///
    /// character:<id> | chapter:<NN> | scene:<NN>/<sNN>
    #[arg(value_name = "TARGET")]
    pub target: MoveTarget,

    /// 並べ替えたあとに、一覧の何番目に来るか（1 始まり）。
    ///
    /// 人物は目次の人物の中、章は章の中、シーンは章のシーンの中で数える。
    #[arg(long, value_name = "N")]
    pub to: NonZeroUsize,

    /// 変更案を表示するだけで、何も書き換えない。
    #[arg(long)]
    pub dry_run: bool,
}
