use std::time::Duration;

/// LM Studio の既定の待受アドレス。
const DEFAULT_BASE_URL: &str = "http://localhost:1234/v1";

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
// LM Studio はモデルの読み込みで最初の応答が遅いことがあるため、長めに取る。
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_mins(5);
const DEFAULT_MAX_RETRIES: u32 = 2;

/// [`OpenAiCompatClient`](crate::OpenAiCompatClient) の接続設定。
#[derive(Clone)]
pub struct ClientConfig {
    /// API のベース URL（例: `"http://localhost:1234/v1"`）。末尾のスラッシュの有無は問わない。
    pub base_url: String,
    /// 送信する場合は `Authorization: Bearer <api_key>` になる。
    pub api_key: Option<String>,
    /// 使用するモデル名。
    pub model: String,
    /// TCP 接続確立のタイムアウト。
    pub connect_timeout: Duration,
    /// ストリーミング中、次のチャンクが届くまでの無通信を許容する上限。
    pub idle_timeout: Duration,
    /// 接続失敗・5xx・429 を再試行する最大回数（初回の試行は含まない）。
    pub max_retries: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key: None,
            model: String::new(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }
}

// API キーをログに漏らさないよう、Debug は手で実装する。
impl std::fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .field("connect_timeout", &self.connect_timeout)
            .field("idle_timeout", &self.idle_timeout)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_は_lm_studio_を指す() {
        let config = ClientConfig::default();
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.model, "");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn debug_は_api_key_の中身を出さない() {
        let config = ClientConfig {
            api_key: Some("sk-super-secret".to_string()),
            ..ClientConfig::default()
        };
        let debug_text = format!("{config:?}");
        assert!(!debug_text.contains("sk-super-secret"));
        assert!(debug_text.contains("redacted"));
    }
}
