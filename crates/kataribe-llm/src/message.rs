use serde::Serialize;

/// チャットメッセージの話者。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// チャットメッセージ 1 件。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    /// システムメッセージ（役割・規則の指示）を作る。
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// ユーザーメッセージを作る。
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// アシスタントメッセージを作る（会話履歴に含める過去の生成結果など）。
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_は小文字の文字列にシリアライズされる() {
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn message_の生成関数は対応する_role_を設定する() {
        assert_eq!(Message::system("a").role, Role::System);
        assert_eq!(Message::user("b").role, Role::User);
        assert_eq!(Message::assistant("c").role, Role::Assistant);
    }
}
