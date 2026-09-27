use serde::{Deserialize, Serialize};

/// [`OpenAiCompatClient::list_models`](crate::OpenAiCompatClient::list_models) が返すモデル情報。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct ModelInfo {
    pub id: String,
    /// 文脈長。LM Studio の拡張 API から取得できたときだけ埋まる。
    pub context_length: Option<u32>,
}
