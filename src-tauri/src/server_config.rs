use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::types::{NativeConfigSource, NativeFrpsConfig, ServerProfile, ServerProfileSummary};

const STORE_FILE: &str = "servers.json";
const STORE_KEY: &str = "servers";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredServerProfile {
    id: String,
    name: String,
    native: NativeFrpsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredServerDocument {
    active_profile_id: String,
    profiles: Vec<StoredServerProfile>,
}

impl From<&StoredServerProfile> for ServerProfile {
    fn from(profile: &StoredServerProfile) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            native: profile.native.clone(),
        }
    }
}

impl StoredServerProfile {
    fn summary(&self, active: bool) -> ServerProfileSummary {
        ServerProfileSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            active,
            configured: !self.native.config_path.trim().is_empty(),
            config_path: self.native.config_path.clone(),
        }
    }
}

fn load_document(app: &AppHandle) -> Result<StoredServerDocument, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|error| format!("无法访问服务端 Profile 存储：{error}"))?;
    let Some(value) = store.get(STORE_KEY) else {
        return Ok(StoredServerDocument::default());
    };
    serde_json::from_value(value).map_err(|error| format!("解析服务端 Profile 失败：{error}"))
}

fn write_document(app: &AppHandle, document: &StoredServerDocument) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|error| format!("无法访问服务端 Profile 存储：{error}"))?;
    let value = serde_json::to_value(document)
        .map_err(|error| format!("序列化服务端 Profile 失败：{error}"))?;
    store.set(STORE_KEY, value);
    store
        .save()
        .map_err(|error| format!("保存服务端 Profile 失败：{error}"))
}

fn generated_profile_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("frps-{millis}")
}

fn normalize_path(path: &str, source: NativeConfigSource) -> Result<String, String> {
    let path = Path::new(path.trim());
    if path.as_os_str().is_empty() {
        return Err("frps 配置文件路径不能为空".into());
    }
    if source == NativeConfigSource::ExternalReadonly && !path.is_file() {
        return Err(format!("找不到 frps 配置文件：{}", path.display()));
    }
    if path.exists() {
        return fs::canonicalize(path)
            .map(|value| value.to_string_lossy().to_string())
            .map_err(|error| format!("无法解析 frps 配置文件路径：{error}"));
    }
    Ok(path.to_string_lossy().to_string())
}

fn set_private_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("设置 frps 配置文件权限失败：{error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_server_profiles(app: AppHandle) -> Result<Vec<ServerProfileSummary>, String> {
    let document = load_document(&app)?;
    Ok(document
        .profiles
        .iter()
        .map(|profile| profile.summary(profile.id == document.active_profile_id))
        .collect())
}

#[tauri::command]
pub fn load_server_profile(
    app: AppHandle,
    profile_id: Option<String>,
) -> Result<Option<ServerProfile>, String> {
    let document = load_document(&app)?;
    let id = profile_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(document.active_profile_id);
    Ok(document
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .map(ServerProfile::from))
}

#[tauri::command]
pub fn select_server_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let mut document = load_document(&app)?;
    if !document
        .profiles
        .iter()
        .any(|profile| profile.id == profile_id)
    {
        return Err(format!("找不到服务端 Profile：{profile_id}"));
    }
    document.active_profile_id = profile_id;
    write_document(&app, &document)
}

#[tauri::command]
pub fn save_server_profile(
    app: AppHandle,
    profile_id: Option<String>,
    name: String,
    config_path: String,
    source: NativeConfigSource,
    auto_start: bool,
    imported_content: Option<String>,
) -> Result<ServerProfile, String> {
    let mut document = load_document(&app)?;
    let id = profile_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(generated_profile_id);
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Server Profile 名称不能为空".into());
    }

    let stored_path = match source {
        NativeConfigSource::Managed => {
            let content = imported_content.filter(|value| !value.trim().is_empty());
            match content {
                Some(content) => {
                    let base = app
                        .path()
                        .app_config_dir()
                        .map_err(|error| format!("无法获取应用配置目录：{error}"))?
                        .join("native-frps")
                        .join(&id);
                    fs::create_dir_all(&base)
                        .map_err(|error| format!("创建 frps Profile 目录失败：{error}"))?;
                    let destination = base.join("frps.toml");
                    fs::write(&destination, content.as_bytes())
                        .map_err(|error| format!("写入 frps.toml 失败：{error}"))?;
                    set_private_permissions(&destination)?;
                    destination.to_string_lossy().to_string()
                }
                None => document
                    .profiles
                    .iter()
                    .find(|profile| profile.id == id)
                    .filter(|profile| profile.native.source == NativeConfigSource::Managed)
                    .map(|profile| profile.native.config_path.clone())
                    .filter(|path| Path::new(path).is_file())
                    .ok_or_else(|| "请先导入或生成一份 frps TOML".to_string())?,
            }
        }
        NativeConfigSource::ExternalReadonly => {
            if imported_content.is_some() {
                return Err("外部只读配置不会复制或改写原文件".into());
            }
            normalize_path(&config_path, source)?
        }
    };

    let native = NativeFrpsConfig {
        config_path: stored_path,
        source,
        auto_start,
    };
    native.validate()?;
    let stored = StoredServerProfile {
        id: id.clone(),
        name,
        native,
    };
    if let Some(existing) = document
        .profiles
        .iter_mut()
        .find(|profile| profile.id == id)
    {
        *existing = stored.clone();
    } else {
        document.profiles.push(stored.clone());
    }
    if document.active_profile_id.is_empty() {
        document.active_profile_id = id;
    }
    write_document(&app, &document)?;
    Ok(ServerProfile::from(&stored))
}

#[tauri::command]
pub fn load_managed_server_config(app: AppHandle, profile_id: String) -> Result<String, String> {
    let document = load_document(&app)?;
    let profile = document
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("找不到服务端 Profile：{profile_id}"))?;
    if profile.native.source != NativeConfigSource::Managed {
        return Err("仅 App 托管的 frps 配置可以在桌面端读取".into());
    }
    let metadata = fs::metadata(&profile.native.config_path)
        .map_err(|error| format!("无法读取托管 frps 配置：{error}"))?;
    if metadata.len() > 1024 * 1024 {
        return Err("托管 frps 配置超过 1 MiB，拒绝加载到编辑器".into());
    }
    fs::read_to_string(&profile.native.config_path)
        .map_err(|error| format!("读取托管 frps 配置失败：{error}"))
}

#[tauri::command]
pub fn delete_server_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let mut document = load_document(&app)?;
    let Some(index) = document
        .profiles
        .iter()
        .position(|profile| profile.id == profile_id)
    else {
        return Err(format!("找不到服务端 Profile：{profile_id}"));
    };
    document.profiles.remove(index);
    if document.active_profile_id == profile_id {
        document.active_profile_id = document
            .profiles
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_default();
    }
    write_document(&app, &document)
}
