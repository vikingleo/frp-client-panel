use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::types::{
    ClientMode, ConnectionConfig, NativeConfigSource, NativeFrpcConfig, Profile, ProfileSummary,
};

const STORE_FILE: &str = "connections.json";
const KEY_CONFIG: &str = "connection";
const KEYRING_SERVICE: &str = "app.frppanel.client";
const LEGACY_KEYRING_SERVICE: &str = "app.frppanel.macclient";
const KEYRING_ACCOUNT: &str = "client-secret";
const PROFILE_STORE_FILE: &str = "profiles.json";
const KEY_PROFILES: &str = "profiles";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredProfile {
    id: String,
    name: String,
    #[serde(default)]
    mode: ClientMode,
    #[serde(default)]
    panel: Option<StoredConnectionConfig>,
    #[serde(default)]
    native: Option<NativeFrpcConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredProfileDocument {
    active_profile_id: String,
    profiles: Vec<StoredProfile>,
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

impl StoredProfile {
    fn from_panel(config: &ConnectionConfig) -> Self {
        Self {
            id: "panel-default".into(),
            name: "frp-panel Client".into(),
            mode: ClientMode::PanelManaged,
            panel: Some(StoredConnectionConfig::from(config)),
            native: None,
        }
    }

    fn summary(&self, active: bool) -> ProfileSummary {
        ProfileSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            mode: self.mode,
            active,
            configured: match self.mode {
                ClientMode::PanelManaged => self.panel.is_some(),
                ClientMode::NativeFrpc => self
                    .native
                    .as_ref()
                    .map(|native| !native.config_path.trim().is_empty())
                    .unwrap_or(false),
            },
            config_path: self
                .native
                .as_ref()
                .map(|native| native.config_path.clone()),
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
    if let Err(error) = upsert_panel_profile(app, config) {
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

fn write_profile_document(app: &AppHandle, document: &StoredProfileDocument) -> Result<(), String> {
    let store = app
        .store(PROFILE_STORE_FILE)
        .map_err(|error| format!("无法访问 Profile 存储：{error}"))?;
    let value =
        serde_json::to_value(document).map_err(|error| format!("序列化 Profile 失败：{error}"))?;
    store.set(KEY_PROFILES, value);
    store
        .save()
        .map_err(|error| format!("保存 Profile 失败：{error}"))
}

fn load_profile_document(app: &AppHandle) -> Result<StoredProfileDocument, String> {
    let store = app
        .store(PROFILE_STORE_FILE)
        .map_err(|error| format!("无法访问 Profile 存储：{error}"))?;
    if let Some(value) = store.get(KEY_PROFILES) {
        return serde_json::from_value(value)
            .map_err(|error| format!("解析 Profile 失败：{error}"));
    }

    // Migrate the original single-connection store without moving or duplicating the secret.
    let legacy_connection = load_connection_inner(app)?;
    let document = profile_document_from_legacy_connection(legacy_connection.as_ref());
    write_profile_document(app, &document)?;
    Ok(document)
}

fn profile_document_from_legacy_connection(
    config: Option<&ConnectionConfig>,
) -> StoredProfileDocument {
    match config {
        Some(config) => StoredProfileDocument {
            active_profile_id: "panel-default".into(),
            profiles: vec![StoredProfile::from_panel(config)],
        },
        None => StoredProfileDocument {
            active_profile_id: String::new(),
            profiles: Vec::new(),
        },
    }
}

fn upsert_panel_profile(app: &AppHandle, config: &ConnectionConfig) -> Result<(), String> {
    let mut document = load_profile_document(app)?;
    if let Some(profile) = document
        .profiles
        .iter_mut()
        .find(|profile| profile.id == "panel-default")
    {
        profile.name = "frp-panel Client".into();
        profile.mode = ClientMode::PanelManaged;
        profile.panel = Some(StoredConnectionConfig::from(config));
        profile.native = None;
    } else {
        document.profiles.push(StoredProfile::from_panel(config));
    }
    if document.active_profile_id.is_empty() {
        document.active_profile_id = "panel-default".into();
    }
    write_profile_document(app, &document)
}

fn hydrate_profile(_app: &AppHandle, stored: &StoredProfile) -> Result<Profile, String> {
    let panel = match &stored.panel {
        Some(panel) => Some(hydrate_connection_config(panel, &SystemCredentialStore)?.0),
        None => None,
    };
    Ok(Profile {
        id: stored.id.clone(),
        name: stored.name.clone(),
        mode: stored.mode,
        panel,
        native: stored.native.clone(),
    })
}

fn normalize_native_path(path: &str, source: NativeConfigSource) -> Result<String, String> {
    let path = Path::new(path.trim());
    if path.as_os_str().is_empty() {
        return Err("原生 frpc 配置文件路径不能为空".into());
    }
    if source == NativeConfigSource::ExternalReadonly && !path.is_file() {
        return Err(format!("找不到原生 frpc 配置文件：{}", path.display()));
    }
    if path.exists() {
        return fs::canonicalize(path)
            .map(|value| value.to_string_lossy().to_string())
            .map_err(|error| format!("无法解析配置文件路径：{error}"));
    }
    Ok(path.to_string_lossy().to_string())
}

fn generated_profile_id(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{prefix}-{millis}")
}

fn profile_from_stored_id<'a>(
    document: &'a StoredProfileDocument,
    id: &str,
) -> Result<&'a StoredProfile, String> {
    document
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| format!("找不到 Profile：{id}"))
}

#[tauri::command]
pub fn list_profiles(app: AppHandle) -> Result<Vec<ProfileSummary>, String> {
    let document = load_profile_document(&app)?;
    Ok(document
        .profiles
        .iter()
        .map(|profile| profile.summary(profile.id == document.active_profile_id))
        .collect())
}

#[tauri::command]
pub fn load_profile(app: AppHandle, profile_id: Option<String>) -> Result<Option<Profile>, String> {
    let document = load_profile_document(&app)?;
    let id = profile_id.unwrap_or_else(|| document.active_profile_id.clone());
    if id.is_empty() {
        return Ok(None);
    }
    Ok(Some(hydrate_profile(
        &app,
        profile_from_stored_id(&document, &id)?,
    )?))
}

#[tauri::command]
pub fn select_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let mut document = load_profile_document(&app)?;
    profile_from_stored_id(&document, &profile_id)?;
    document.active_profile_id = profile_id;
    write_profile_document(&app, &document)
}

#[tauri::command]
pub fn save_native_profile(
    app: AppHandle,
    profile_id: Option<String>,
    name: String,
    config_path: String,
    source: NativeConfigSource,
    auto_connect: bool,
    launch_at_login: bool,
    imported_content: Option<String>,
) -> Result<Profile, String> {
    let mut document = load_profile_document(&app)?;
    let id = profile_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| generated_profile_id("native"));
    let id = id.trim().to_string();
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_".contains(ch))
    {
        return Err("Profile ID 只能包含字母、数字、短横线和下划线".into());
    }
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("原生 Profile 名称不能为空".into());
    }

    let normalized_path = match source {
        NativeConfigSource::Managed => {
            match imported_content.filter(|content| !content.trim().is_empty()) {
                Some(content) => {
                    let base = app
                        .path()
                        .app_config_dir()
                        .map_err(|error| format!("无法获取应用配置目录：{error}"))?
                        .join("native-profiles")
                        .join(&id);
                    fs::create_dir_all(&base)
                        .map_err(|error| format!("创建原生 Profile 目录失败：{error}"))?;
                    let destination = base.join("frpc.toml");
                    fs::write(&destination, content.as_bytes())
                        .map_err(|error| format!("写入 frpc.toml 失败：{error}"))?;
                    set_private_permissions(&destination)?;
                    destination.to_string_lossy().to_string()
                }
                None => document
                    .profiles
                    .iter()
                    .find(|profile| profile.id == id && profile.mode == ClientMode::NativeFrpc)
                    .and_then(|profile| profile.native.as_ref())
                    .filter(|native| native.source == NativeConfigSource::Managed)
                    .map(|native| native.config_path.clone())
                    .filter(|path| Path::new(path).is_file())
                    .ok_or_else(|| "请先导入或粘贴非空 TOML 配置".to_string())?,
            }
        }
        NativeConfigSource::ExternalReadonly => {
            if imported_content.is_some() {
                return Err("外部只读配置不会复制或改写原文件".into());
            }
            normalize_native_path(&config_path, source)?
        }
    };

    let native = NativeFrpcConfig {
        config_path: normalized_path,
        source,
        auto_connect,
        launch_at_login,
    };
    native.validate()?;
    let stored = StoredProfile {
        id: id.clone(),
        name,
        mode: ClientMode::NativeFrpc,
        panel: None,
        native: Some(native),
    };
    if let Some(existing) = document
        .profiles
        .iter_mut()
        .find(|profile| profile.id == id)
    {
        *existing = stored;
    } else {
        document.profiles.push(stored);
    }
    document.active_profile_id = id.clone();
    write_profile_document(&app, &document)?;
    let saved = profile_from_stored_id(&document, &id)?.clone();
    Ok(Profile {
        id: saved.id,
        name: saved.name,
        mode: saved.mode,
        panel: None,
        native: saved.native,
    })
}

#[tauri::command]
pub fn load_managed_native_config(app: AppHandle, profile_id: String) -> Result<String, String> {
    let document = load_profile_document(&app)?;
    let profile = profile_from_stored_id(&document, &profile_id)?;
    let native = profile
        .native
        .as_ref()
        .filter(|native| native.source == NativeConfigSource::Managed)
        .ok_or_else(|| "仅 App 托管的原生配置可以在桌面端读取".to_string())?;
    let metadata = fs::metadata(&native.config_path)
        .map_err(|error| format!("无法读取托管 frpc 配置：{error}"))?;
    if metadata.len() > 1024 * 1024 {
        return Err("托管 frpc 配置超过 1 MiB，拒绝加载到编辑器".into());
    }
    fs::read_to_string(&native.config_path)
        .map_err(|error| format!("读取托管 frpc 配置失败：{error}"))
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let mut document = load_profile_document(&app)?;
    if document.profiles.len() <= 1 {
        return Err("至少需要保留一个 Profile".into());
    }
    let index = document
        .profiles
        .iter()
        .position(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("找不到 Profile：{profile_id}"))?;
    document.profiles.remove(index);
    if document.active_profile_id == profile_id {
        document.active_profile_id = document
            .profiles
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_default();
    }
    write_profile_document(&app, &document)
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("设置配置文件权限失败：{error}"))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{
        hydrate_connection_config, profile_document_from_legacy_connection, SecretStore,
        StoredConnectionConfig, StoredProfile, KEYRING_ACCOUNT, KEYRING_SERVICE,
    };
    use crate::types::{ClientMode, ConnectionConfig};

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

    #[test]
    fn stored_panel_profile_does_not_serialize_secret() {
        let profile = StoredProfile {
            id: "panel-default".into(),
            name: "frp-panel Client".into(),
            mode: ClientMode::PanelManaged,
            panel: Some(StoredConnectionConfig::from(&ConnectionConfig {
                client_id: "user.c.mac".into(),
                client_secret: "must-not-write-profile-json".into(),
                api_url: "https://panel.example.com".into(),
                rpc_url: "wss://panel.example.com/rpc".into(),
                auto_connect: false,
                launch_at_login: false,
                allow_insecure_tls: false,
            })),
            native: None,
        };
        let serialized = serde_json::to_string(&profile).unwrap();
        assert!(!serialized.contains("must-not-write-profile-json"));
    }

    #[test]
    fn migrates_legacy_connection_to_the_default_panel_profile_without_serializing_secret() {
        let legacy = ConnectionConfig {
            client_id: "user.c.mac".into(),
            client_secret: "must-stay-in-keychain".into(),
            api_url: "https://panel.example.com".into(),
            rpc_url: "wss://panel.example.com/rpc".into(),
            auto_connect: true,
            launch_at_login: false,
            allow_insecure_tls: false,
        };

        let document = profile_document_from_legacy_connection(Some(&legacy));

        assert_eq!(document.active_profile_id, "panel-default");
        assert_eq!(document.profiles.len(), 1);
        let profile = &document.profiles[0];
        assert_eq!(profile.id, "panel-default");
        assert_eq!(profile.mode, ClientMode::PanelManaged);
        assert_eq!(
            profile.panel.as_ref().map(|panel| panel.client_id.as_str()),
            Some("user.c.mac")
        );
        assert!(!serde_json::to_string(&document)
            .unwrap()
            .contains("must-stay-in-keychain"));
    }
}
