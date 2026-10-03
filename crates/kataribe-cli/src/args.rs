//! コマンドライン引数の定義（clap）。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use kataribe_engine::{DraftUnit, LlmProvider, Task};
use kataribe_project::Rating;

use crate::stage::Stage;
use crate::structure_args::{AddArgs, MoveArgs, RemoveArgs};
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
// コマンドラインのフラグ（--polish・--no-polish・--quiet・--verbose）をそのまま表すため、bool が並ぶ
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Args)]
pub struct GlobalOptions {
    /// 設定ファイル（既定: GUI と同じ場所。明示したのに無ければエラー）。
    #[arg(long, global = true, value_name = "PATH")]
    pub settings: Option<PathBuf>,

    /// 生成に使う LLM の種類。
    #[arg(long, global = true, value_enum)]
    pub provider: Option<ProviderArg>,

    /// LLM サーバーのベース URL（OpenAI 互換 API のとき）。
    #[arg(long, global = true, value_name = "URL")]
    pub base_url: Option<String>,

    /// 使用するモデル名（Claude Code では sonnet・opus・haiku など）。選んでいる接続先のモデルを変える。
    #[arg(long, global = true, value_name = "ID")]
    pub model: Option<String>,

    /// Claude Code の claude コマンド（PATH に無いときに実行ファイルの場所を指定する）。
    #[arg(long, global = true, value_name = "PATH")]
    pub claude_command: Option<String>,

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

    /// 推敲パスをかけない（設定ファイルや作品の設定で推敲すると決めていても止める）。
    #[arg(long, global = true, conflicts_with = "polish")]
    pub no_polish: bool,

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
    /// 登場人物・世界観の資料・章・シーンを、自分で書いて足す（LLM は使わない）。
    Add(AddArgs),
    /// 登場人物・世界観の資料・章・シーンを、ゴミ箱（.kataribe/trash/）へ移して消す（LLM は使わない）。
    Remove(RemoveArgs),
    /// 登場人物・章・シーンの順番を変える（LLM は使わない）。章を動かすと、動く範囲の章の番号を振り直す。
    Move(MoveArgs),
    /// シーンごとの品質レポートを表示する。
    Quality(QualityArgs),
    /// 本文を章題付きの一つのテキストにまとめる。
    Export(ExportArgs),
    /// 選べるモデルの一覧を表示する（接続の確認を兼ねる）。
    Models,
    /// 作品ごとの設定（kataribe.yaml の settings）を表示する。--save で、指定したオプションを作品に保存する。
    ProjectSettings(ProjectSettingsArgs),
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
pub struct ProjectSettingsArgs {
    /// 作品フォルダ。
    #[arg(value_name = "FOLDER")]
    pub folder: PathBuf,

    /// グローバルオプションで指定した値（--provider・--model・--unit・--chars-per-call・
    /// --context-tokens・--temperature・--polish・--no-polish・--quality-retries）を作品に保存する。
    #[arg(long)]
    pub save: bool,
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
    /// | scenes:<NN> | draft:<NN>/<sNN> | revise:<path> | add-character | add-world
    ///
    /// add-character・add-world は、指示から人物・世界観の資料を 1 つ作って足す。
    #[arg(value_name = "TASK")]
    pub task: TaskSpec,

    /// 書き直す指示、または作らせたい内容の指示（`revise:<path>`・add-character・add-world のときは必須。
    /// それ以外では指定できない）。
    #[arg(long, value_name = "TEXT")]
    pub instruction: Option<String>,

    /// 足す世界観の資料のファイル名（world/<SLUG>.md。add-world のときだけ指定できる。省略すると題から決める）。
    #[arg(long, value_name = "SLUG")]
    pub name: Option<String>,

    /// 変更案を表示するだけで、原稿と資料は書き換えない（要約などの中間データのキャッシュは更新する）。
    #[arg(long)]
    pub dry_run: bool,
}

impl GenerateArgs {
    /// 引数から生成のタスクを組み立てる。`--instruction`・`--name` の組み合わせの誤りは、使い方の誤りとして返す。
    pub fn to_task(&self) -> Result<Task, String> {
        self.task
            .clone()
            .into_task(self.instruction.clone(), self.name.clone())
    }

    /// 組み合わせの誤り（指示が要る種類に無い・要らない種類にある・`--name` を付けられない種類にある）を確かめる。
    fn validate(&self) -> Result<(), String> {
        self.to_task().map(drop)
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

/// [`kataribe_engine::LlmProvider`] の clap 版。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProviderArg {
    /// OpenAI 互換の API（LM Studio など）。
    OpenaiCompatible,
    /// Claude Code の claude -p（API キーの代わりに Claude Code のログインを使う）。
    ClaudeCode,
}

impl From<ProviderArg> for LlmProvider {
    fn from(value: ProviderArg) -> Self {
        match value {
            ProviderArg::OpenaiCompatible => LlmProvider::OpenaiCompatible,
            ProviderArg::ClaudeCode => LlmProvider::ClaudeCode,
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
    use crate::structure_args::AddTarget;
    use crate::target_spec::{MoveTarget, RemoveTarget};
    use kataribe_project::{ChapterId, RelPath, SceneId};

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
    fn providers_use_kebab_case_spelling() {
        let cli = parse(&["--provider", "claude-code", "models"]).unwrap();
        assert_eq!(cli.global.provider, Some(ProviderArg::ClaudeCode));

        let cli = parse(&["--provider", "openai-compatible", "models"]).unwrap();
        assert_eq!(cli.global.provider, Some(ProviderArg::OpenaiCompatible));

        assert!(parse(&["--provider", "claude", "models"]).is_err());
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
            name: None,
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

    #[test]
    fn validate_requires_an_instruction_for_the_generated_additions() {
        for task in [TaskSpec::AddCharacter, TaskSpec::AddWorld] {
            assert!(generate_args(task.clone(), None).validate().is_err());
            assert!(
                generate_args(task, Some("港町の歴史を足す"))
                    .validate()
                    .is_ok()
            );
        }
    }

    #[test]
    fn validate_accepts_a_name_only_for_add_world() {
        let named = |task| GenerateArgs {
            name: Some("port-town".to_owned()),
            ..generate_args(task, Some("指示"))
        };

        assert!(named(TaskSpec::AddWorld).validate().is_ok());
        assert!(named(TaskSpec::AddCharacter).validate().is_err());
        assert!(
            named(TaskSpec::Revise(RelPath::new("concept.md").unwrap()))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn generate_add_world_takes_an_instruction_and_a_name() {
        let cli = parse(&[
            "generate",
            "folder",
            "add-world",
            "--instruction",
            "港町の歴史",
            "--name",
            "port-town",
        ])
        .unwrap();
        let Command::Generate(args) = cli.command else {
            panic!("Generate が来るはず");
        };
        assert_eq!(
            args.to_task(),
            Ok(Task::AddWorldDocument {
                name: Some("port-town".to_owned()),
                instruction: "港町の歴史".to_owned(),
            })
        );
    }

    #[test]
    fn add_character_takes_the_fields_and_an_optional_id() {
        let cli = parse(&[
            "add",
            "character",
            "folder",
            "--name",
            "霧島 凛",
            "--reading",
            "きりしま りん",
            "--order",
            "2",
            "--id",
            "rin",
            "--body",
            "本文",
            "--dry-run",
        ])
        .unwrap();

        let Command::Add(AddArgs {
            target: AddTarget::Character(args),
        }) = cli.command
        else {
            panic!("add character が来るはず");
        };
        assert_eq!(args.name, "霧島 凛");
        assert_eq!(args.reading.as_deref(), Some("きりしま りん"));
        assert_eq!(args.order, Some(2));
        assert_eq!(args.id.as_deref(), Some("rin"));
        assert_eq!(args.body.body.as_deref(), Some("本文"));
        assert!(args.dry_run);
    }

    #[test]
    fn add_character_requires_a_name() {
        assert!(parse(&["add", "character", "folder"]).is_err());
    }

    #[test]
    fn an_unusable_id_is_left_to_the_engine_so_the_reason_is_told_in_one_place() {
        let cli = parse(&["add", "character", "folder", "--name", "凛", "--id", "Rin"]).unwrap();

        let Command::Add(AddArgs {
            target: AddTarget::Character(args),
        }) = cli.command
        else {
            panic!("add character が来るはず");
        };
        assert_eq!(args.id.as_deref(), Some("Rin"));
    }

    #[test]
    fn the_body_can_be_given_as_text_or_as_a_file_but_not_both() {
        let both = [
            "add",
            "world",
            "folder",
            "--title",
            "題",
            "--body",
            "a",
            "--body-file",
            "b",
        ];
        assert!(parse(&both).is_err());
        assert!(
            parse(&[
                "add",
                "world",
                "folder",
                "--title",
                "題",
                "--body-file",
                "b"
            ])
            .is_ok()
        );
        assert!(parse(&["add", "world", "folder", "--title", "題"]).is_ok());
    }

    #[test]
    fn add_scene_splits_the_characters_at_commas_and_validates_the_ids() {
        let cli = parse(&[
            "add",
            "scene",
            "folder",
            "01",
            "--title",
            "題",
            "--characters",
            "霧島 凛,佐藤 健二",
            "--before",
            "s02",
        ])
        .unwrap();

        let Command::Add(AddArgs {
            target: AddTarget::Scene(args),
        }) = cli.command
        else {
            panic!("add scene が来るはず");
        };
        assert_eq!(args.chapter, ChapterId::from_number(1));
        assert_eq!(args.characters, vec!["霧島 凛", "佐藤 健二"]);
        assert_eq!(args.before, Some(SceneId::from_number(2)));
        assert!(
            parse(&["add", "scene", "folder", "1", "--title", "題"]).is_err(),
            "章番号は 2〜3 桁"
        );
        assert!(
            parse(&[
                "add", "scene", "folder", "01", "--title", "題", "--before", "02"
            ])
            .is_err()
        );
    }

    #[test]
    fn add_chapter_takes_a_title_a_storyline_and_the_chapter_to_go_before() {
        let cli = parse(&[
            "add",
            "chapter",
            "folder",
            "--title",
            "雨の匂い",
            "--storyline",
            "あらすじ",
            "--before",
            "02",
            "--dry-run",
        ])
        .unwrap();

        let Command::Add(AddArgs {
            target: AddTarget::Chapter(args),
        }) = cli.command
        else {
            panic!("add chapter が来るはず");
        };
        assert_eq!(args.title, "雨の匂い");
        assert_eq!(args.storyline.storyline.as_deref(), Some("あらすじ"));
        assert_eq!(args.before, Some(ChapterId::from_number(2)));
        assert!(args.dry_run);
    }

    #[test]
    fn add_chapter_goes_to_the_end_by_default_and_checks_its_arguments() {
        let cli = parse(&["add", "chapter", "folder", "--title", "題"]).unwrap();
        let Command::Add(AddArgs {
            target: AddTarget::Chapter(args),
        }) = cli.command
        else {
            panic!("add chapter が来るはず");
        };
        assert_eq!(args.before, None);

        assert!(parse(&["add", "chapter", "folder"]).is_err(), "章題は必須");
        assert!(
            parse(&["add", "chapter", "folder", "--title", "題", "--before", "1"]).is_err(),
            "章番号は 2〜3 桁"
        );
        assert!(
            parse(&[
                "add",
                "chapter",
                "folder",
                "--title",
                "題",
                "--storyline",
                "a",
                "--storyline-file",
                "b"
            ])
            .is_err(),
            "ストーリーラインは直接かファイルのどちらか"
        );
    }

    #[test]
    fn remove_takes_a_target_and_a_dry_run_flag() {
        let cli = parse(&["remove", "folder", "scene:01/s02", "--dry-run"]).unwrap();

        let Command::Remove(args) = cli.command else {
            panic!("remove が来るはず");
        };
        assert_eq!(
            args.target,
            RemoveTarget::Scene {
                chapter: ChapterId::from_number(1),
                scene: SceneId::from_number(2),
            }
        );
        assert!(args.dry_run);
        assert!(parse(&["remove", "folder", "chapter:1"]).is_err());
    }

    #[test]
    fn remove_takes_a_chapter() {
        let cli = parse(&["remove", "folder", "chapter:03"]).unwrap();

        let Command::Remove(args) = cli.command else {
            panic!("remove が来るはず");
        };
        assert_eq!(
            args.target,
            RemoveTarget::Chapter(ChapterId::from_number(3))
        );
    }

    #[test]
    fn move_takes_a_target_a_one_based_position_and_a_dry_run_flag() {
        let cli = parse(&["move", "folder", "chapter:03", "--to", "1", "--dry-run"]).unwrap();

        let Command::Move(args) = cli.command else {
            panic!("move が来るはず");
        };
        assert_eq!(args.target, MoveTarget::Chapter(ChapterId::from_number(3)));
        assert_eq!(args.to.get(), 1);
        assert!(args.dry_run);
    }

    #[test]
    fn move_needs_a_position_of_at_least_one() {
        assert!(parse(&["move", "folder", "chapter:03"]).is_err());
        assert!(parse(&["move", "folder", "chapter:03", "--to", "0"]).is_err());
        assert!(parse(&["move", "folder", "chapter:03", "--to", "-1"]).is_err());
        assert!(parse(&["move", "folder", "world:glossary", "--to", "1"]).is_err());
    }
}
