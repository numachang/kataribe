use serde::{Serialize, Serializer};

use crate::message::Message;

/// サンプリングパラメータ。すべて省略可能で、既定値は「サーバーに送らない」。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sampling {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub seed: Option<i64>,
    /// 空なら送らない。
    pub stop: Vec<String>,
}

/// 構造化出力の指定。
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseFormat {
    /// JSON Schema による構造化出力。常に `strict: true` で送る。
    JsonSchema {
        name: String,
        schema: serde_json::Value,
    },
}

impl Serialize for ResponseFormat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Self::JsonSchema { name, schema } = self;
        serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": name,
                "strict": true,
                "schema": schema,
            }
        })
        .serialize(serializer)
    }
}

/// 1 回のチャット生成リクエスト。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
    pub sampling: Sampling,
    pub response_format: Option<ResponseFormat>,
    /// サーバー固有の追加パラメータ（例: LM Studio の `top_k`・`min_p`・`repeat_penalty`）。
    /// リクエスト本体の JSON にそのまま展開される。
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// ワイヤー形式でのストリーミング指定。
#[derive(Serialize)]
struct WireStreamOptions {
    include_usage: bool,
}

/// サーバーに送る JSON 本体そのままの形。
///
/// [`ChatRequest`] を直接シリアライズしない（`extra` の展開や `stream`/`stream_options` の
/// 付与など、ワイヤー形式固有の事情を公開型に持ち込まないため）。
#[derive(Serialize)]
pub(crate) struct WireChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
    stream_options: WireStreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
    #[serde(skip_serializing_if = "is_empty_slice")]
    stop: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'a ResponseFormat>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

fn is_empty_slice(value: &&[String]) -> bool {
    value.is_empty()
}

/// 本体が使う JSON キー。`extra` にこれらと同じキーがあると、そのまま展開すると
/// JSON オブジェクトの中で同じキーが 2 回出てしまうため、`extra` 側を無視する。
const RESERVED_KEYS: &[&str] = &[
    "model",
    "messages",
    "stream",
    "stream_options",
    "max_tokens",
    "temperature",
    "top_p",
    "frequency_penalty",
    "presence_penalty",
    "seed",
    "stop",
    "response_format",
];

/// `extra` から、本体の項目と同じキーを取り除く。取り除いた場合は警告を出す。
fn filter_extra(
    extra: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut filtered = serde_json::Map::with_capacity(extra.len());
    for (key, value) in extra {
        if RESERVED_KEYS.contains(&key.as_str()) {
            tracing::warn!(
                key = %key,
                "extra の項目がリクエスト本体の項目と重複するため無視します"
            );
            continue;
        }
        filtered.insert(key.clone(), value.clone());
    }
    filtered
}

impl<'a> WireChatRequest<'a> {
    pub(crate) fn new(model: &'a str, request: &'a ChatRequest) -> Self {
        Self {
            model,
            messages: &request.messages,
            stream: true,
            stream_options: WireStreamOptions {
                include_usage: true,
            },
            max_tokens: request.max_tokens,
            temperature: request.sampling.temperature,
            top_p: request.sampling.top_p,
            frequency_penalty: request.sampling.frequency_penalty,
            presence_penalty: request.sampling.presence_penalty,
            seed: request.sampling.seed,
            stop: &request.sampling.stop,
            response_format: request.response_format.as_ref(),
            extra: filter_extra(&request.extra),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    #[test]
    fn 最小構成のリクエストは省略可能な項目を送らない() {
        let request = ChatRequest {
            messages: vec![Message::user("こんにちは")],
            ..Default::default()
        };
        let wire = WireChatRequest::new("test-model", &request);
        let json = serde_json::to_value(&wire).unwrap();

        assert_eq!(json["model"], "test-model");
        assert_eq!(json["stream"], true);
        assert_eq!(json["stream_options"]["include_usage"], true);
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"], "こんにちは");
        assert!(json.get("max_tokens").is_none());
        assert!(json.get("temperature").is_none());
        assert!(json.get("stop").is_none());
        assert!(json.get("response_format").is_none());
    }

    #[test]
    fn サンプリングと_extra_と_response_format_が展開される() {
        let mut extra = serde_json::Map::new();
        extra.insert("top_k".to_string(), serde_json::json!(40));

        let request = ChatRequest {
            messages: vec![Message::system("規則"), Message::user("本文")],
            max_tokens: Some(256),
            sampling: Sampling {
                temperature: Some(0.8),
                top_p: Some(0.9),
                frequency_penalty: Some(0.1),
                presence_penalty: Some(0.2),
                seed: Some(42),
                stop: vec!["\n\n".to_string()],
            },
            response_format: Some(ResponseFormat::JsonSchema {
                name: "cast".to_string(),
                schema: serde_json::json!({"type": "object"}),
            }),
            extra,
        };
        let wire = WireChatRequest::new("test-model", &request);
        let json = serde_json::to_value(&wire).unwrap();

        assert_eq!(json["max_tokens"], 256);
        assert_eq!(json["temperature"], 0.8);
        assert_eq!(json["top_p"], 0.9);
        assert_eq!(json["frequency_penalty"], 0.1);
        assert_eq!(json["presence_penalty"], 0.2);
        assert_eq!(json["seed"], 42);
        assert_eq!(json["stop"], serde_json::json!(["\n\n"]));
        assert_eq!(json["top_k"], 40);
        assert_eq!(
            json["response_format"],
            serde_json::json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "cast",
                    "strict": true,
                    "schema": {"type": "object"},
                }
            })
        );
    }

    #[test]
    fn extra_が本体と同じキーを持っていても_json_のキーが重複しない() {
        let mut extra = serde_json::Map::new();
        extra.insert("model".to_string(), serde_json::json!("なりすましモデル"));
        extra.insert("max_tokens".to_string(), serde_json::json!(999));
        extra.insert("messages".to_string(), serde_json::json!("なりすまし本文"));
        extra.insert("stream".to_string(), serde_json::json!(false));
        extra.insert("top_k".to_string(), serde_json::json!(40));

        let request = ChatRequest {
            messages: vec![Message::user("本文")],
            max_tokens: Some(256),
            extra,
            ..ChatRequest::default()
        };
        let wire = WireChatRequest::new("test-model", &request);
        let json = serde_json::to_value(&wire).unwrap();

        // 本体側の値が優先され、extra の同名キーは無視される。
        assert_eq!(json["model"], "test-model");
        assert_eq!(json["max_tokens"], 256);
        assert_eq!(json["stream"], true);
        assert_eq!(json["messages"][0]["content"], "本文");
        // 重複しない extra の項目はそのまま残る。
        assert_eq!(json["top_k"], 40);

        // シリアライズ結果の JSON オブジェクトが、本当にキーの重複なく 1 つずつしか
        // 出ていないことを、生成された JSON テキストの出現回数で確認する。
        let text = serde_json::to_string(&wire).unwrap();
        assert_eq!(text.matches("\"model\"").count(), 1);
        assert_eq!(text.matches("\"max_tokens\"").count(), 1);
        assert_eq!(text.matches("\"messages\"").count(), 1);
        assert_eq!(text.matches("\"stream\"").count(), 1);
    }
}
