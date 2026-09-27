use futures_util::stream::BoxStream;

use crate::error::LlmError;
use crate::event::ChatEvent;
use crate::request::ChatRequest;

/// チャット生成の結果を非同期に受け取るストリーム。
///
/// 呼び出し側がこのストリームを drop すると、内部の HTTP 接続も中断される。
pub type ChatStream = BoxStream<'static, Result<ChatEvent, LlmError>>;

/// チャット生成モデルへの、ストリーミングでの問い合わせ口。
///
/// [`OpenAiCompatClient`](crate::OpenAiCompatClient) と、テスト用の
/// [`ScriptedChatModel`](crate::testing::ScriptedChatModel)（`testing` feature）が実装する。
pub trait ChatModel: Send + Sync + std::fmt::Debug {
    /// リクエストを送り、応答をストリームで返す。
    ///
    /// 何も送信していない状態でも即座に返る。実際の通信は、返したストリームが
    /// 初めてポーリングされたときに始まる。
    fn stream_chat(&self, request: ChatRequest) -> ChatStream;
}
