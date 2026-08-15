use std::fmt::Display;

use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::types::ConnectionConfig;

const STORE_FILE: &str = "connections.json";
const KEY_CONFIG: &str = "connection";
const KEYRING_SERVICE: &str = "app.frppanel.client";
const LEGACY_KEYRING_SERVICE: &str = "app.frppanel.macclient";
const KEYRING_ACCOUNT: &str = "client-secret";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredConnectionConfig {
    client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    api_url: String,
    rpc_url: String,
    #[serde(default)]
    auto_connect: bool,
    #[serde(default)]
    launch_at_login: bool,
    #[serde(default)]
    allow_insecure_tls: bool,
}

impl From<&ConnectionConfig> for StoredConnectionConfig {
    fn from(config: &ConnectionConfig) -> Self {
        Self {
            client_id: config.client_id.clone(),
            client_secret: None,
            api_url: config.api_url.clone(),
            rpc_url: config.rpc_url.clone(),
            auto_connect: config.auto_connect,
            launch_at_login: config.launch_at_login,
            allow_insecure_tls: config.allow_insecure_tls,
        }
    }
}

impl StoredConnectionConfig {
    fn without_legacy_secret(&self) -> Self {
        let mut cleaned = self.clone();
        cleaned.client_secret = None;
        cleaned
    }

    fn to_connection_config(&self, client_secret: String) -> ConnectionConfig {
        ConnectionConfig {
            client_id: self.client_id.clone(),
            client_secret,
            api_url: self.api_url.clone(),
            rpc_url: self.rpc_url.clone(),
            auto_connect: self.auto_connect,
            launch_at_login: self.launch_at_login,
            allow_insecure_tls: self.allow_insecure_tls,
        }
    }
}

trait SecretStore {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, secret: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

struct SystemCredentialStore;

impl SystemCredentialStore {
    fn entry_for(&self, service: &str) -> Result<Entry, String> {
        Entry::new(service, KEYRING_ACCOUNT).map_err(credential_store_error)
    }

    fn entry(&self) -> Result<Entry, String> {
        self.entry_for(KEYRING_SERVICE)
    }

    fn legacy_entry(&self) -> Result<Entry, String> {
        self.entry_for(LEGACY_KEYRING_SERVICE)
    }
}

impl SecretStore for SystemCredentialStore {
    fn get(&self) -> Result<Option<String>, String> {
        match self.entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => match self.legacy_entry()?.get_password() {
                Ok(secret) => {
                    self.entry()?
                        .set_password(&secret)
                        .map_err(credential_store_error)?;
                    let _ = self.legacy_entry()?.delete_credential();
                    Ok(Some(secret))
                }
                Err(KeyringError::NoEntry) => Ok(None),
                Err(error) => Err(credential_store_error(error)),
            },
            Err(error) => Err(credential_store_error(error)),
        }
    }

    fn set(&self, secret: &str) -> Result<(), String> {
        self.entry()?
            .set_password(secret)
            .map_err(credential_store_error)
    }

    fn delete(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(credential_store_error(error)),
        }
    }
}

#[tauri::command]
pub fn save_connection(app: AppHandle, config: ConnectionConfig) -> Result<(), String> {
    config.validate()?;
    let normalized = normalize_config(&config);
    save_connection_inner(&app, &normalized)
}

pub fn save_connection_inner(app: &AppHandle, config: &ConnectionConfig) -> Result<(), String> {
    config.validate()?;
    save_connection_with_secret_store(app, config, &SystemCredentialStore)
}

fn save_connection_with_secret_store(
    app: &AppHandle,
    config: &ConnectionConfig,
    secret_store: &impl SecretStore,
) -> Result<(), String> {
    let previous_secret = secret_store.get()?;
    secret_store.set(&config.client_secret)?;

    let stored = StoredConnectionConfig::from(config);
    if let Err(error) = write_stored_connection(app, &stored) {
        restore_secret(secret_store, previous_secret);
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn load_connection(app: AppHandle) -> Result<Option<ConnectionConfig>, String> {
    load_connection_inner(&app)
}

pub fn load_connection_inner(app: &AppHandle) -> Result<Option<ConnectionConfig>, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|error| format!("无法访问配置存储：{error}"))?;
    let Some(value) = store.get(KEY_CONFIG) else {
        return Ok(None);
    };

    let stored: StoredConnectionConfig =
        serde_json::from_value(value).map_err(|error| format!("解析配置失败：{error}"))?;
    let (config, migrated_legacy_secret) =
        hydrate_connection_config(&stored, &SystemCredentialStore)?;

    if migrated_legacy_secret {
        write_stored_connection(app, &stored.without_legacy_secret())?;
    }

    Ok(Some(config))
}

fn write_stored_connection(app: &AppHandle, config: &StoredConnectionConfig) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|error| format!("无法访问配置存储：{error}"))?;
    let value = serde_json::to_value(config).map_err(|error| format!("序列化配置失败：{error}"))?;
    store.set(KEY_CONFIG, value);
    store
        .save()
        .map_err(|error| format!("保存配置失败：{error}"))
}

fn hydrate_connection_config(
    stored: &StoredConnectionConfig,
    secret_store: &impl SecretStore,
) -> Result<(ConnectionConfig, bool), String> {
    let legacy_secret = stored
        .client_secret
        .as_deref()
        .filter(|secret| !secret.trim().is_empty());
    let keychain_secret = secret_store.get()?;

    let client_secret = match keychain_secret {
        Some(secret) if !secret.trim().is_empty() => secret,
        _ => {
            let Some(secret) = legacy_secret else {
                return Err(
                    "系统凭据库中未找到 Client Secret。请在配置页重新粘贴命令并保存。".into(),
                );
            };
            secret_store.set(secret)?;
            secret.to_string()
        }
    };

    let config = stored.to_connection_config(client_secret);
    config.validate()?;
    Ok((config, stored.client_secret.is_some()))
}

fn normalize_config(config: &ConnectionConfig) -> ConnectionConfig {
    ConnectionConfig {
        client_id: config.client_id.trim().to_string(),
        client_secret: config.client_secret.trim().to_string(),
        api_url: config.api_url.trim().to_string(),
        rpc_url: config.rpc_url.trim().to_string(),
        auto_connect: config.auto_connect,
        launch_at_login: config.launch_at_login,
        allow_insecure_tls: config.allow_insecure_tls,
    }
}

fn restore_secret(secret_store: &impl SecretStore, previous_secret: Option<String>) {
    match previous_secret {
        Some(secret) => {
            let _ = secret_store.set(&secret);
        }
        None => {
            let _ = secret_store.delete();
        }
    }
}

fn credential_store_error(error: impl Display) -> String {
    format!("无法访问系统凭据库：{error}")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{
        hydrate_connection_config, SecretStore, StoredConnectionConfig, KEYRING_ACCOUNT,
        KEYRING_SERVICE,
    };
    use crate::types::ConnectionConfig;

    #[derive(Default)]
    struct MemorySecretStore {
        secret: RefCell<Option<String>>,
    }

    impl super::SecretStore for MemorySecretStore {
        fn get(&self) -> Result<Option<String>, String> {
            Ok(self.secret.borrow().clone())
        }

        fn set(&self, secret: &str) -> Result<(), String> {
            *self.secret.borrow_mut() = Some(secret.to_string());
            Ok(())
        }

        fn delete(&self) -> Result<(), String> {
            *self.secret.borrow_mut() = None;
            Ok(())
        }
    }

    fn stored_config(legacy_secret: Option<&str>) -> StoredConnectionConfig {
        StoredConnectionConfig {
            client_id: "user.c.mac".into(),
            client_secret: legacy_secret.map(str::to_string),
            api_url: "https://panel.example.com".into(),
            rpc_url: "wss://panel.example.com/rpc".into(),
            auto_connect: true,
            launch_at_login: true,
            allow_insecure_tls: false,
        }
    }

    #[test]
    fn new_store_metadata_never_serializes_client_secret() {
        let config = ConnectionConfig {
            client_id: "user.c.mac".into(),
            client_secret: "new-secret".into(),
            api_url: "https://panel.example.com".into(),
            rpc_url: "wss://panel.example.com/rpc".into(),
            auto_connect: true,
            launch_at_login: true,
            allow_insecure_tls: false,
        };

        let value = serde_json::to_value(StoredConnectionConfig::from(&config)).unwrap();

        assert!(value.get("client_secret").is_none());
        assert_eq!(
            value
                .get("allow_insecure_tls")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(KEYRING_SERVICE, "app.frppanel.client");
        assert_eq!(KEYRING_ACCOUNT, "client-secret");
    }

    #[test]
    fn migrates_legacy_plaintext_secret_to_system_credential_store() {
        let stored = stored_config(Some("legacy-secret"));
        let secrets = MemorySecretStore::default();

        let (config, migrated) = hydrate_connection_config(&stored, &secrets).unwrap();

        assert_eq!(config.client_secret, "legacy-secret");
        assert!(config.launch_at_login);
        assert!(migrated);
        assert_eq!(secrets.get().unwrap().as_deref(), Some("legacy-secret"));
        assert!(!serde_json::to_string(&stored.without_legacy_secret())
            .unwrap()
            .contains("legacy-secret"));
    }

    #[test]
    fn prefers_existing_credential_and_cleans_legacy_field() {
        let stored = stored_config(Some("old-secret"));
        let secrets = MemorySecretStore {
            secret: RefCell::new(Some("keychain-secret".into())),
        };

        let (config, migrated) = hydrate_connection_config(&stored, &secrets).unwrap();

        assert_eq!(config.client_secret, "keychain-secret");
        assert!(migrated);
    }

    #[test]
    fn rejects_metadata_without_credential_or_legacy_secret() {
        let stored = stored_config(None);
        let error = hydrate_connection_config(&stored, &MemorySecretStore::default()).unwrap_err();

        assert!(error.contains("系统凭据库"));
    }
}
