//! Secure storage boundary for credentials.
//!
//! This module is the only location permitted to interact with the operating system credential
//! store. On macOS, `keyring` uses Keychain Services. Tokens are never stored in SQLite, returned
//! from a Tauri command, logged, or included in error details.

use std::{error::Error, fmt};

use keyring::Entry;

const SERVICE_NAME: &str = "com.astockai.workbench";
const TUSHARE_ACCOUNT_NAME: &str = "tushare-token";
const DEEPSEEK_ACCOUNT_NAME: &str = "deepseek-api-key";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecureStorageError {
    Unavailable,
    OperationFailed,
    EmptyToken,
}

impl fmt::Display for SecureStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("系统安全凭据库不可用"),
            Self::OperationFailed => formatter.write_str("系统安全凭据库操作失败"),
            Self::EmptyToken => formatter.write_str("密钥不能为空"),
        }
    }
}

impl Error for SecureStorageError {}

/// A narrow, testable port. Its public status APIs only expose whether a credential exists.
pub trait TushareTokenStore {
    fn read(&self) -> Result<Option<String>, SecureStorageError>;
    fn save(&self, token: &str) -> Result<(), SecureStorageError>;
    fn remove(&self) -> Result<(), SecureStorageError>;
}

pub struct SystemTushareTokenStore;

impl SystemTushareTokenStore {
    fn entry(&self) -> Result<Entry, SecureStorageError> {
        Entry::new(SERVICE_NAME, TUSHARE_ACCOUNT_NAME).map_err(|_| SecureStorageError::Unavailable)
    }
}

impl TushareTokenStore for SystemTushareTokenStore {
    fn read(&self) -> Result<Option<String>, SecureStorageError> {
        match self.entry()?.get_password() {
            Ok(token) if !token.trim().is_empty() => Ok(Some(token)),
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecureStorageError::OperationFailed),
        }
    }

    fn save(&self, token: &str) -> Result<(), SecureStorageError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(SecureStorageError::EmptyToken);
        }
        self.entry()?
            .set_password(token)
            .map_err(|_| SecureStorageError::OperationFailed)
    }

    fn remove(&self) -> Result<(), SecureStorageError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecureStorageError::OperationFailed),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TushareStatusView {
    /// The only credential-related value that may cross the Tauri IPC boundary.
    pub status: String,
}

pub struct TushareTokenService<S = SystemTushareTokenStore> {
    store: S,
}

impl TushareTokenService<SystemTushareTokenStore> {
    pub fn system() -> Self {
        Self {
            store: SystemTushareTokenStore,
        }
    }
}

impl<S: TushareTokenStore> TushareTokenService<S> {
    #[cfg(test)]
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn status(&self) -> Result<TushareStatusView, SecureStorageError> {
        Ok(Self::to_status(self.store.read()?.is_some()))
    }

    pub fn save(&self, token: &str) -> Result<TushareStatusView, SecureStorageError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(SecureStorageError::EmptyToken);
        }
        self.store.save(token)?;
        Ok(Self::to_status(true))
    }

    pub fn remove(&self) -> Result<TushareStatusView, SecureStorageError> {
        self.store.remove()?;
        Ok(Self::to_status(false))
    }

    /// Used exclusively by Rust provider adapters. This method is deliberately not a Tauri
    /// command and no caller may serialize its return value.
    pub fn read_for_adapter(&self) -> Result<Option<String>, SecureStorageError> {
        self.store.read()
    }

    fn to_status(configured: bool) -> TushareStatusView {
        TushareStatusView {
            status: if configured { "已配置" } else { "未配置" }.into(),
        }
    }
}

pub fn load_tushare_token_for_adapter() -> Result<Option<String>, SecureStorageError> {
    TushareTokenService::system().read_for_adapter()
}

pub fn get_tushare_status() -> Result<TushareStatusView, SecureStorageError> {
    TushareTokenService::system().status()
}

pub fn save_tushare_token(token: &str) -> Result<TushareStatusView, SecureStorageError> {
    TushareTokenService::system().save(token)
}

pub fn remove_tushare_token() -> Result<TushareStatusView, SecureStorageError> {
    TushareTokenService::system().remove()
}

/// The DeepSeek credential uses the same operating-system secure-store boundary as Tushare.
/// On macOS this is Keychain Services; neither this API nor its status value exposes the key.
pub trait DeepSeekApiKeyStore {
    fn read(&self) -> Result<Option<String>, SecureStorageError>;
    fn save(&self, key: &str) -> Result<(), SecureStorageError>;
    fn remove(&self) -> Result<(), SecureStorageError>;
}

pub struct SystemDeepSeekApiKeyStore;

impl SystemDeepSeekApiKeyStore {
    fn entry(&self) -> Result<Entry, SecureStorageError> {
        Entry::new(SERVICE_NAME, DEEPSEEK_ACCOUNT_NAME).map_err(|_| SecureStorageError::Unavailable)
    }
}

impl DeepSeekApiKeyStore for SystemDeepSeekApiKeyStore {
    fn read(&self) -> Result<Option<String>, SecureStorageError> {
        match self.entry()?.get_password() {
            Ok(key) if !key.trim().is_empty() => Ok(Some(key)),
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecureStorageError::OperationFailed),
        }
    }

    fn save(&self, key: &str) -> Result<(), SecureStorageError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(SecureStorageError::EmptyToken);
        }
        self.entry()?
            .set_password(key)
            .map_err(|_| SecureStorageError::OperationFailed)
    }

    fn remove(&self) -> Result<(), SecureStorageError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecureStorageError::OperationFailed),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeepSeekStatusView {
    pub status: String,
}

pub struct DeepSeekApiKeyService<S = SystemDeepSeekApiKeyStore> {
    store: S,
}

impl DeepSeekApiKeyService<SystemDeepSeekApiKeyStore> {
    pub fn system() -> Self {
        Self {
            store: SystemDeepSeekApiKeyStore,
        }
    }
}

impl<S: DeepSeekApiKeyStore> DeepSeekApiKeyService<S> {
    #[cfg(test)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
    pub fn status(&self) -> Result<DeepSeekStatusView, SecureStorageError> {
        Ok(Self::status_view(self.store.read()?.is_some()))
    }
    pub fn save(&self, key: &str) -> Result<DeepSeekStatusView, SecureStorageError> {
        if key.trim().is_empty() {
            return Err(SecureStorageError::EmptyToken);
        }
        self.store.save(key.trim())?;
        Ok(Self::status_view(true))
    }
    pub fn remove(&self) -> Result<DeepSeekStatusView, SecureStorageError> {
        self.store.remove()?;
        Ok(Self::status_view(false))
    }
    /// Provider-only access: never expose this through Tauri IPC or logs.
    pub fn read_for_adapter(&self) -> Result<Option<String>, SecureStorageError> {
        self.store.read()
    }
    fn status_view(configured: bool) -> DeepSeekStatusView {
        DeepSeekStatusView {
            status: if configured { "已配置" } else { "未配置" }.into(),
        }
    }
}

pub fn load_deepseek_api_key_for_adapter() -> Result<Option<String>, SecureStorageError> {
    DeepSeekApiKeyService::system().read_for_adapter()
}
pub fn get_deepseek_status() -> Result<DeepSeekStatusView, SecureStorageError> {
    DeepSeekApiKeyService::system().status()
}
pub fn save_deepseek_api_key(key: &str) -> Result<DeepSeekStatusView, SecureStorageError> {
    DeepSeekApiKeyService::system().save(key)
}
pub fn remove_deepseek_api_key() -> Result<DeepSeekStatusView, SecureStorageError> {
    DeepSeekApiKeyService::system().remove()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct MemoryStore(Arc<Mutex<Option<String>>>);

    impl TushareTokenStore for MemoryStore {
        fn read(&self) -> Result<Option<String>, SecureStorageError> {
            Ok(self.0.lock().expect("lock memory store").clone())
        }

        fn save(&self, token: &str) -> Result<(), SecureStorageError> {
            *self.0.lock().expect("lock memory store") = Some(token.into());
            Ok(())
        }

        fn remove(&self) -> Result<(), SecureStorageError> {
            *self.0.lock().expect("lock memory store") = None;
            Ok(())
        }
    }

    impl DeepSeekApiKeyStore for MemoryStore {
        fn read(&self) -> Result<Option<String>, SecureStorageError> {
            Ok(self.0.lock().expect("lock memory store").clone())
        }
        fn save(&self, key: &str) -> Result<(), SecureStorageError> {
            *self.0.lock().expect("lock memory store") = Some(key.into());
            Ok(())
        }
        fn remove(&self) -> Result<(), SecureStorageError> {
            *self.0.lock().expect("lock memory store") = None;
            Ok(())
        }
    }

    #[test]
    fn saves_restores_and_removes_status_without_returning_the_token() {
        let store = MemoryStore::default();
        let service = TushareTokenService::new(store.clone());
        assert_eq!(service.status().unwrap().status, "未配置");

        let saved = service.save("test-token-only-in-memory").unwrap();
        assert_eq!(saved.status, "已配置");
        assert!(!format!("{saved:?}").contains("test-token-only-in-memory"));

        // A new service instance uses the same secure-store boundary, modelling application
        // restart without making the test write any credential to a developer's Keychain.
        let restarted = TushareTokenService::new(store);
        assert_eq!(restarted.status().unwrap().status, "已配置");
        assert_eq!(restarted.remove().unwrap().status, "未配置");
    }

    #[test]
    fn rejects_blank_tokens() {
        let error = TushareTokenService::new(MemoryStore::default())
            .save("  ")
            .expect_err("blank token must be rejected");
        assert_eq!(error, SecureStorageError::EmptyToken);
    }

    #[test]
    fn deepseek_key_survives_restart_and_status_never_reveals_it() {
        let store = MemoryStore::default();
        let service = DeepSeekApiKeyService::new(store.clone());
        assert_eq!(service.status().unwrap().status, "未配置");
        let saved = service.save("deepseek-test-key").unwrap();
        assert_eq!(saved.status, "已配置");
        assert!(!format!("{saved:?}").contains("deepseek-test-key"));
        let restarted = DeepSeekApiKeyService::new(store);
        assert_eq!(restarted.status().unwrap().status, "已配置");
        assert_eq!(restarted.remove().unwrap().status, "未配置");
    }
}
