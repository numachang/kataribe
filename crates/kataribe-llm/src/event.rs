/// トークン使用量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// 生成が終わった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// 自然に文章が終わった。
    Stop,
    /// `max_tokens` などの上限に達した。
    Length,
    /// コンテンツフィルタで打ち切られた。
    ContentFilter,
    /// サーバー固有の理由、またはサーバーから理由が得られなかった場合。
    Other(String),
}

impl FinishReason {
    /// `OpenAI` 互換 API の `finish_reason` 文字列から変換する。
    pub(crate) fn from_wire(reason: &str) -> Self {
        match reason {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "content_filter" => Self::ContentFilter,
            other => Self::Other(other.to_string()),
        }
    }
}

/// ストリーム終了時の情報。
#[derive(Debug, Clone, PartialEq)]
pub struct Finish {
    pub reason: FinishReason,
    pub usage: Option<Usage>,
}

/// [`ChatModel::stream_chat`](crate::ChatModel::stream_chat) が生成する 1 つの出来事。
#[derive(Debug, Clone, PartialEq)]
pub enum ChatEvent {
    /// 本文の断片。
    Content(String),
    /// 推論（思考）の断片。本文には混ぜない。
    Reasoning(String),
    /// ストリームの終了。1 回のストリームで必ずちょうど 1 回、最後に出る。
    Finished(Finish),
}
