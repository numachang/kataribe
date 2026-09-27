use kataribe_llm::LlmError;
use kataribe_project::ProjectError;

/// 執筆エンジンのエラー。Display は利用者にそのまま見せる日本語。
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Project(#[from] ProjectError),

    #[error(transparent)]
    Llm(#[from] LlmError),

    #[error("YAML を読み書きできません: {0}")]
    Yaml(#[from] kataribe_project::YamlError),

    #[error("生成を中止しました。")]
    Cancelled,

    /// 生成の前提となる資料が足りない。
    #[error("{0}")]
    MissingPrerequisite(String),

    /// 利用者の入力が不正（題名が空、書き直しの対象にできないファイルなど）。
    #[error("{0}")]
    InvalidInput(String),

    /// 資料が長すぎて、モデルの文脈に収まらない。
    #[error("{0}")]
    ContextTooSmall(String),

    /// 指定された章・シーン・人物などが見つからない。
    #[error("{0}")]
    NotFound(String),

    /// LLM の出力を期待した形に解釈できなかった。
    #[error("LLM の出力を解釈できませんでした: {0}")]
    InvalidOutput(String),

    #[error("組み込みのジャンルプリセットを読み込めません: {0}")]
    Preset(String),

    #[error("プロンプトの組み立てに失敗しました: {0}")]
    Prompt(String),

    #[error("API キーを資格情報マネージャーで読み書きできません: {0}")]
    Secret(String),

    #[error("設定ファイルを読み書きできません（{path}）: {reason}")]
    Settings { path: String, reason: String },
}

pub type Result<T, E = EngineError> = std::result::Result<T, E>;

impl From<kataribe_project::PathError> for EngineError {
    fn from(error: kataribe_project::PathError) -> Self {
        Self::Project(error.into())
    }
}
