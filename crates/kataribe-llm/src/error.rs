/// 文字数の目安でエラー本文を切り詰める上限（ログ・画面表示が長くなりすぎないため）。
const MAX_ERROR_BODY_CHARS: usize = 500;

/// [`OpenAiCompatClient`](crate::OpenAiCompatClient) が返すエラー。
///
/// [`Display`](std::fmt::Display) の文言はそのまま利用者に見せられる日本語にしてある。
/// API キーは決してここに含めない。
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// [`ClientConfig`](crate::ClientConfig) の値が不正で、クライアントを作れない。
    #[error("設定が不正です: {0}")]
    InvalidConfig(String),

    /// サーバーに接続できない（未起動・アドレス間違い・ネットワーク断など）。
    #[error(
        "LLM サーバー（{base_url}）に接続できません。LM Studio などのサーバーが起動しているか確認してください。"
    )]
    Connection {
        /// 接続しようとした `base_url`。
        base_url: String,
        /// 元になったエラー（reqwest の内部事情を公開 API に漏らさないよう型消去してある）。
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// 次のチャンクを [`ClientConfig::idle_timeout`](crate::ClientConfig::idle_timeout) の間待っても届かなかった。
    #[error(
        "LLM サーバーからの応答がありません（タイムアウト）。モデルの読み込みに時間がかかっている可能性があります。"
    )]
    Timeout,

    /// サーバーが 2xx 以外のステータスを返した。
    #[error("LLM サーバーがエラーを返しました（HTTP {status}）: {body}")]
    Status {
        /// HTTP ステータスコード。
        status: u16,
        /// 応答本文（500 文字程度に切り詰め済み）。
        body: String,
    },

    /// 応答の形式が想定と違う（SSE の構文・JSON の形・ストリーム中のエラー通知など）。
    #[error("LLM サーバーの応答を解釈できません: {0}")]
    Protocol(String),
}

impl LlmError {
    /// テスト（`testing` feature）などで、接続エラーを任意の理由から組み立てる。
    ///
    /// 実際の通信では [`reqwest::Error`] が理由になるが、公開 API はそれに依存しない
    /// ようにしているので、ここでは任意の [`std::error::Error`] を受け取れる。
    pub(crate) fn connection(
        base_url: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Connection {
            base_url: base_url.into(),
            source: Box::new(source),
        }
    }
}

/// エラー本文表示用に、文字数（書記素ではなく char 単位で十分）で切り詰める。
pub(crate) fn truncate_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() <= MAX_ERROR_BODY_CHARS {
        return trimmed.to_string();
    }
    let mut truncated: String = trimmed.chars().take(MAX_ERROR_BODY_CHARS).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_body_は上限以下ならそのまま返す() {
        assert_eq!(truncate_body("  短い本文  "), "短い本文");
    }

    #[test]
    fn truncate_body_は上限を超えたら省略記号を付けて切り詰める() {
        let long_body = "あ".repeat(600);
        let truncated = truncate_body(&long_body);
        assert_eq!(truncated.chars().count(), MAX_ERROR_BODY_CHARS + 1);
        assert!(truncated.ends_with('…'));
    }
}
