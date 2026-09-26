//! LLM サーバーの API キーを、OS の資格情報ストア（Windows では資格情報マネージャー）に保存する。
//! 設定ファイルには書かないので、作品フォルダや設定を共有してもキーは漏れない。
//! GUI と CLI は同じ場所を使うため、どちらかで設定すればもう一方でも使える。

use crate::error::{EngineError, Result};

const ACCOUNT: &str = "llm-api-key";

/// API キーの保存場所。
#[derive(Debug, Clone)]
pub struct ApiKeyStore {
    service: String,
    account: String,
}

impl Default for ApiKeyStore {
    fn default() -> Self {
        Self {
            service: crate::APP_IDENTIFIER.to_owned(),
            account: ACCOUNT.to_owned(),
        }
    }
}

impl ApiKeyStore {
    /// 保存されている API キー。無ければ `None`。
    pub fn load(&self) -> Result<Option<String>> {
        match self.entry()?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(secret_error(&error)),
        }
    }

    /// API キーを保存する。空文字列なら削除する。
    pub fn save(&self, key: &str) -> Result<()> {
        let key = key.trim();
        if key.is_empty() {
            return self.delete();
        }
        self.entry()?
            .set_password(key)
            .map_err(|error| secret_error(&error))
    }

    /// 保存されている API キーを削除する。無ければ何もしない。
    pub fn delete(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(secret_error(&error)),
        }
    }

    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.account).map_err(|error| secret_error(&error))
    }
}

fn secret_error(error: &keyring::Error) -> EngineError {
    EngineError::Secret(error.to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// 本物の資格情報マネージャーを使うので、テストごとに別のアカウント名にして後始末する。
    fn temporary_store(name: &str) -> ApiKeyStore {
        ApiKeyStore {
            service: format!("{}.test", crate::APP_IDENTIFIER),
            account: format!("{name}-{}", std::process::id()),
        }
    }

    #[test]
    fn key_round_trips_through_the_credential_store() {
        let store = temporary_store("round-trip");
        store.save("  secret-token  ").unwrap();
        assert_eq!(store.load().unwrap().as_deref(), Some("secret-token"));
        store.delete().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn saving_an_empty_key_deletes_it() {
        let store = temporary_store("empty");
        store.save("token").unwrap();
        store.save("   ").unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn deleting_a_missing_key_succeeds() {
        temporary_store("missing").delete().unwrap();
    }
}
