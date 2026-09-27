//! コマンドライン引数の定義（clap）。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use kataribe_engine::DraftUnit;
use kataribe_project::Rating;

use crate::stage::Stage;
use crate::task_spec::TaskSpec;

/// GUI と同じ執筆エンジンを画面なしで動かす。
#[derive(Debug, Parser)]
#[command(name = "kataribe-cli", about = "kataribe のヘッドレス実行", long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOptions,
    #[command(subcommand)]
    pub command: Command,
}

/// 設定ファイルの値を上書きするグローバルオプション。
#[derive(Debug, Default, Args)]
pub struct GlobalOptions {
    /// 設定ファイル（既定: GUI と同じ場所）。
    #[arg(long, global = true, value_name = "PATH")]
    pub settings: Option<PathBuf>,

    /// LLM サーバーのベース URL。
    #[arg(long, global = true, value_name = "URL")]
    pub base_url: Option<String>,

    /// 使用するモデル名。
    #[arg(long, global = true, value_name = "ID")]
    pub model: Option<String>,

    /// API キーを読む環境変数（未設定なら資格情報マネージャーのキーを使う）。
    #[arg(
        long,
        global = true,
        value_name = "VAR",
        default_value = "KATARIBE_API_KEY"
    )]
    pub api_key_env: String,

    /// 本文の生成単位。
    #[arg(long, global = true, value_enum)]
    pub unit: Option<DraftUnitArg>,

    /// 1 回の生成で書かせる目安の文字数。
    #[arg(long, global = true, value_name = "N")]
    pub chars_per_call: Option<u32>,

    /// モデルに渡せる文脈の長さ（トークン）。
    #[arg(long, global = true, value_name = "N")]
    pub context_tokens: Option<u32>,

    /// 生成の温度。
    #[arg(long, global = true, value_name = "T")]
    pub temperature: Option<f64>,

    /// 本文を書いたあとに推敲パスをかける。
    #[arg(long, global = true)]
    pub polish: bool,

    /// 品質チェックで重大な問題が見つかったときの再生成回数。
    #[arg(long, global = true, value_name = "N")]
    pub quality_retries: Option<u32>,

    /// 生成中の本文を表示しない。
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// 推論モデルの思考を標準エラー出力に薄く表示する。
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 新しい作品を作る。
    New(NewArgs),
    /// 工程の状態と文字数を表示する。
    Status(StatusArgs),
    /// 1 つの工程を生成する。
    Generate(GenerateArgs),
    /// 取りかかれる工程を順に生成・適用する。
    Run(RunArgs),
    /// シーンごとの品質レポートを表示する。
    Quality(QualityArgs),
    /// 本文を章題付きの一つのテキストにまとめる。
    Export(ExportArgs),
    /// LLM サーバーのモデル一覧を表示する。
    Models,
    /// API キーを資格情報マネージャーに保存・削除・確認する。
    ApiKey(ApiKeyArgs),
}

impl Command {
    /// clap 自身では表せない、引数の組み合わせの誤りを確かめる。
    ///
    /// clap のパースが通ったあと、コマンドを実行する前に呼ぶ。エラーは使い方の誤り
    /// （終了コード 2）として扱う。
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Command::Generate(args) => args.validate(),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Args)]
pub struct NewArgs {
    /// 作品フォルダ（空のフォルダ、または存在しないフォルダ）。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 題名。
    #[arg(long)]
    pub title: String,

    /// 著者名。
    #[arg(long)]
    pub author: Option<String>,

    /// ジャンルプリセットの id（既定: general）。
    #[arg(long)]
    pub genre: Option<String>,

    /// ジャンルの補足。
    #[arg(long)]
    pub genre_note: Option<String>,

    /// 年齢区分。
    #[arg(long, value_enum, default_value_t = RatingArg::General)]
    pub rating: RatingArg,

    /// 目標総文字数（既定 30,000 字）。
    #[arg(long, default_value_t = 30_000)]
    pub length: u32,

    #[command(flatten)]
    pub idea_source: IdeaSource,
}

/// 企画の種は、直接の文章かファイルのどちらか一方で指定する。
#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct IdeaSource {
    /// 企画の種（最初に LLM へ渡す指示）。
    #[arg(long, value_name = "TEXT")]
    pub idea: Option<String>,

    /// 企画の種を書いたファイル。
    #[arg(long, value_name = "PATH")]
    pub idea_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// `PipelineStep` と `ProjectOverview` を JSON で出力する。
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 生成する工程。
    ///
    /// concept | style | world | cast | character:<id> | synopsis | outline
    /// | scenes:<NN> | draft:<NN>/<sNN> | revise:<path>
    #[arg(value_name = "TASK")]
    pub task: TaskSpec,

    /// `revise:<path>` を書き直す指示（`revise:<path>` のときは必須。それ以外では指定できない）。
    #[arg(long, value_name = "TEXT")]
    pub instruction: Option<String>,

    /// 変更案を表示するだけで、原稿と資料は書き換えない（要約などの中間データのキャッシュは更新する）。
    #[arg(long)]
    pub dry_run: bool,
}

impl GenerateArgs {
    /// `--instruction` は `revise:<path>` のときだけ必須、それ以外では指定できない。
    fn validate(&self) -> Result<(), String> {
        let is_revise = matches!(self.task, TaskSpec::Revise(_));
        match (is_revise, &self.instruction) {
            (true, None) => {
                Err("generate revise:<path> には --instruction <TEXT> が必要です。".to_owned())
            }
            (false, Some(_)) => {
                Err("--instruction は revise:<path> のときだけ指定できます。".to_owned())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// この段階まで進んだら止まる（工程がすべて済むまで、の意味）。
    #[arg(long, value_enum)]
    pub until: Option<Stage>,

    /// 生成する工程数の上限。
    #[arg(long, value_name = "N")]
    pub max_steps: Option<u32>,
}

#[derive(Debug, Args)]
pub struct QualityArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 品質レポートを JSON で出力する。
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// 書き出し先のファイル（省略時は標準出力。作品フォルダの外を指定すること）。
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// 書き出し先に既存のファイルがあっても上書きする。
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ApiKeyArgs {
    #[command(subcommand)]
    pub action: ApiKeyAction,
}

#[derive(Debug, Subcommand)]
pub enum ApiKeyAction {
    /// 標準入力から読んだ 1 行を API キーとして保存する。
    ///
    /// 端末から直接入力すると、入力したキーがそのまま画面に表示される。
    /// 表示したくない場合は、パイプで渡すこと（例: `Get-Content key.txt | kataribe-cli api-key set`）。
    Set,
    /// 保存されている API キーを削除する。
    Clear,
    /// API キーが設定されているかどうかを表示する。
    Status,
}

/// [`kataribe_project::Rating`] の clap 版。値の綴りを固定するため手で名前を付ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RatingArg {
    #[value(name = "general")]
    General,
    #[value(name = "r15")]
    R15,
    #[value(name = "r18")]
    R18,
}

impl From<RatingArg> for Rating {
    fn from(value: RatingArg) -> Self {
        match value {
            RatingArg::General => Rating::General,
            RatingArg::R15 => Rating::R15,
            RatingArg::R18 => Rating::R18,
        }
    }
}

/// [`kataribe_engine::DraftUnit`] の clap 版。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DraftUnitArg {
    Chapter,
    Scene,
    Beat,
}

impl From<DraftUnitArg> for DraftUnit {
    fn from(value: DraftUnitArg) -> Self {
        match value {
            DraftUnitArg::Chapter => DraftUnit::Chapter,
            DraftUnitArg::Scene => DraftUnit::Scene,
            DraftUnitArg::Beat => DraftUnit::Beat,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_project::RelPath;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once(&"kataribe-cli").chain(args.iter()))
    }

    #[test]
    fn new_requires_exactly_one_of_idea_or_idea_file() {
        assert!(
            parse(&["new", "folder", "--title", "t", "--length", "1000"]).is_err(),
            "idea も idea-file も無いのはエラーのはず"
        );
        assert!(
            parse(&[
                "new",
                "folder",
                "--title",
                "t",
                "--length",
                "1000",
                "--idea",
                "a",
                "--idea-file",
                "b"
            ])
            .is_err(),
            "両方指定はエラーのはず"
        );
        assert!(
            parse(&[
                "new", "folder", "--title", "t", "--length", "1000", "--idea", "a"
            ])
            .is_ok()
        );
    }

    #[test]
    fn global_options_are_accepted_after_the_subcommand() {
        let cli = parse(&["--model", "gemma", "status", "folder"]).unwrap();
        assert_eq!(cli.global.model.as_deref(), Some("gemma"));

        let cli = parse(&["status", "--json", "folder", "--model", "gemma"]).unwrap();
        assert_eq!(cli.global.model.as_deref(), Some("gemma"));
    }

    #[test]
    fn rating_values_use_lowercase_spelling() {
        let cli = parse(&[
            "new", "folder", "--title", "t", "--length", "1000", "--idea", "a", "--rating", "r18",
        ])
        .unwrap();
        let Command::New(new_args) = cli.command else {
            panic!("New が来るはず");
        };
        assert_eq!(new_args.rating, RatingArg::R18);
    }

    #[test]
    fn length_defaults_to_thirty_thousand_characters_when_omitted() {
        let cli = parse(&["new", "folder", "--title", "t", "--idea", "a"]).unwrap();
        let Command::New(new_args) = cli.command else {
            panic!("New が来るはず");
        };
        assert_eq!(new_args.length, 30_000);
    }

    #[test]
    fn length_can_still_be_given_explicitly() {
        let cli = parse(&[
            "new", "folder", "--title", "t", "--idea", "a", "--length", "5000",
        ])
        .unwrap();
        let Command::New(new_args) = cli.command else {
            panic!("New が来るはず");
        };
        assert_eq!(new_args.length, 5000);
    }

    fn generate_args(task: TaskSpec, instruction: Option<&str>) -> GenerateArgs {
        GenerateArgs {
            folder: PathBuf::from("folder"),
            task,
            instruction: instruction.map(str::to_owned),
            dry_run: false,
        }
    }

    #[test]
    fn validate_requires_instruction_for_revise() {
        let args = generate_args(TaskSpec::Revise(RelPath::new("concept.md").unwrap()), None);
        assert!(args.validate().is_err());
    }

    #[test]
    fn validate_accepts_revise_with_instruction() {
        let args = generate_args(
            TaskSpec::Revise(RelPath::new("concept.md").unwrap()),
            Some("もっと短く"),
        );
        assert!(args.validate().is_ok());
    }

    #[test]
    fn validate_rejects_instruction_for_non_revise_tasks() {
        let args = generate_args(TaskSpec::Concept, Some("無視されるはず"));
        assert!(args.validate().is_err());
    }

    #[test]
    fn validate_accepts_non_revise_tasks_without_instruction() {
        let args = generate_args(TaskSpec::Concept, None);
        assert!(args.validate().is_ok());
    }
}
