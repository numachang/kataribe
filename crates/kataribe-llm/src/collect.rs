use futures_util::StreamExt;

use crate::chat_model::ChatStream;
use crate::error::LlmError;
use crate::event::{ChatEvent, Finish, FinishReason};

const CLOSE_THINK_TAG: &str = "</think>";

/// ストリームを最後まで読み切った結果。
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub content: String,
    pub reasoning: String,
    pub finish: Finish,
}

/// [`ChatStream`] を最後まで読み、全文を組み立てる。
///
/// `on_event` は届いた [`ChatEvent`] ごとに呼ばれる（画面への逐次表示などに使う）。
/// ストリームがエラーを返した時点で、そのエラーをそのまま返す。
pub async fn collect(
    mut stream: ChatStream,
    mut on_event: impl FnMut(&ChatEvent),
) -> Result<Completion, LlmError> {
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut finish = None;

    while let Some(event) = stream.next().await {
        let event = event?;
        on_event(&event);
        match event {
            ChatEvent::Content(text) => content.push_str(&text),
            ChatEvent::Reasoning(text) => reasoning.push_str(&text),
            ChatEvent::Finished(f) => finish = Some(f),
        }
    }

    // 念のため: モデルによっては開きタグ <think> を出さないまま思考を書き始め、
    // 閉じタグ </think> だけを出すことがある。その場合、ストリーミングのフィルタは
    // 開きタグを見つけられないため、閉じタグより前が本文として content に残ってしまう。
    // ここで最終結果に対してだけ、その分を reasoning に付け替える。
    if let Some(close_at) = content.find(CLOSE_THINK_TAG) {
        let missed_reasoning = &content[..close_at];
        reasoning = format!("{missed_reasoning}{reasoning}");
        content = content[close_at + CLOSE_THINK_TAG.len()..].to_string();
    }

    let finish = finish.unwrap_or(Finish {
        reason: FinishReason::Other("eof".to_string()),
        usage: None,
    });

    Ok(Completion {
        content,
        reasoning,
        finish,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Usage;
    use futures_util::stream;

    fn ok_stream(events: Vec<ChatEvent>) -> ChatStream {
        stream::iter(events.into_iter().map(Ok)).boxed()
    }

    #[tokio::test]
    async fn 本文と推論を分けて集める() {
        let stream = ok_stream(vec![
            ChatEvent::Reasoning("考え中".to_string()),
            ChatEvent::Content("結論".to_string()),
            ChatEvent::Finished(Finish {
                reason: FinishReason::Stop,
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 2,
                }),
            }),
        ]);

        let mut observed = Vec::new();
        let completion = collect(stream, |event| observed.push(event.clone()))
            .await
            .unwrap();

        assert_eq!(completion.content, "結論");
        assert_eq!(completion.reasoning, "考え中");
        assert_eq!(completion.finish.reason, FinishReason::Stop);
        assert_eq!(observed.len(), 3);
    }

    #[tokio::test]
    async fn finish_reason_がなくても_eof_として終える() {
        let stream = ok_stream(vec![ChatEvent::Content("途中".to_string())]);
        let completion = collect(stream, |_| {}).await.unwrap();
        assert_eq!(
            completion.finish.reason,
            FinishReason::Other("eof".to_string())
        );
    }

    #[tokio::test]
    async fn ストリーム中のエラーはそのまま返す() {
        let stream = stream::iter(vec![
            Ok(ChatEvent::Content("途中まで".to_string())),
            Err(LlmError::Protocol("壊れた応答".to_string())),
        ])
        .boxed();

        let result = collect(stream, |_| {}).await;
        assert!(matches!(result, Err(LlmError::Protocol(_))));
    }

    #[tokio::test]
    async fn 開きタグを見落としても閉じタグの前を推論として扱う() {
        let stream = ok_stream(vec![ChatEvent::Content(
            "考えている最中</think>ここからが本文".to_string(),
        )]);
        let completion = collect(stream, |_| {}).await.unwrap();
        assert_eq!(completion.reasoning, "考えている最中");
        assert_eq!(completion.content, "ここからが本文");
    }
}
