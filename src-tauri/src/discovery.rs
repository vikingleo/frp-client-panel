use std::collections::{BTreeMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::State;

use crate::command_parser::{parse_safe_client_command_tokens, SafeClientCommand};
use crate::runtime::AppRuntime;
use crate::types::{
    ExternalBinaryInfo, ExternalClientDiscovery, ObservedClientInfo, StartupItemInfo,
};

const CLIENT_BINARY_NAMES: [&str; 2] = ["frp-panel", "frp-panel-client"];

#[tauri::command]
pub fn get_external_client_discovery(runtime: State<'_, AppRuntime>) -> ExternalClientDiscovery {
    discover_external_clients(runtime.managed_child_pid())
}

pub fn discover_external_clients(excluded_pid: Option<u32>) -> ExternalClientDiscovery {
    ExternalClientDiscovery {
        installed_binaries: discover_installed_binaries(),
        running_clients: discover_running_clients(excluded_pid),
        startup_items: discover_startup_items(),
    }
}

pub fn find_external_client_conflict(
    client_id: &str,
    excluded_pid: Option<u32>,
) -> Option<ObservedClientInfo> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return None;
    }

    let clients = discover_running_clients(excluded_pid);
    find_client_id_conflict(&clients, client_id).cloned()
}

fn discover_installed_binaries() -> Vec<ExternalBinaryInfo> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            candidates.extend(binary_candidates_in_directory(&directory, "PATH"));
        }
    }

    for directory in common_binary_directories() {
        candidates.extend(binary_candidates_in_directory(&directory, "常用目录"));
    }

    dedupe_binaries(candidates)
}

fn common_binary_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    directories.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/local/sbin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);

    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = env::var_os("ProgramFiles") {
            directories.push(PathBuf::from(program_files));
        }
    }

    directories
}

fn binary_candidates_in_directory(directory: &Path, source: &str) -> Vec<ExternalBinaryInfo> {
    client_binary_filenames()
        .into_iter()
        .map(|name| directory.join(name))
        .filter(|path| is_executable_file(path))
        .map(|path| ExternalBinaryInfo {
            name: binary_name_from_path(&path).unwrap_or_else(|| "frp-panel".to_string()),
            path: canonical_or_original_path(&path),
            source: source.to_string(),
        })
        .collect()
}

fn client_binary_filenames() -> Vec<&'static str> {
    #[cfg(target_os = "windows")]
    {
        vec!["frp-panel.exe", "frp-panel-client.exe"]
    }

    #[cfg(not(target_os = "windows"))]
    {
        CLIENT_BINARY_NAMES.to_vec()
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        return metadata.permissions().mode() & 0o111 != 0;
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn canonical_or_original_path(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn dedupe_binaries(binaries: Vec<ExternalBinaryInfo>) -> Vec<ExternalBinaryInfo> {
    let mut unique = BTreeMap::new();
    for binary in binaries {
        unique.entry(binary.path.clone()).or_insert(binary);
    }
    unique.into_values().collect()
}

fn discover_running_clients(excluded_pid: Option<u32>) -> Vec<ObservedClientInfo> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );

    let mut clients = system
        .processes()
        .values()
        .filter_map(|process| {
            let pid = process.pid().as_u32();
            if Some(pid) == excluded_pid {
                return None;
            }

            let command = process.cmd();
            let binary_name = process_binary_name(process.exe(), process.name(), command)?;
            observed_client_from_tokens(
                pid,
                binary_name,
                process.exe().map(canonical_or_original_path),
                command,
                process.start_time(),
                process.run_time(),
            )
        })
        .collect::<Vec<_>>();
    clients.sort_by_key(|client| client.pid);
    clients
}

fn process_binary_name(
    executable: Option<&Path>,
    process_name: &OsStr,
    command: &[OsString],
) -> Option<String> {
    executable
        .and_then(binary_name_from_path)
        .filter(|name| is_known_client_binary_name(name))
        .or_else(|| {
            process_name
                .to_str()
                .filter(|name| is_known_client_binary_name(name))
                .map(str::to_string)
        })
        .or_else(|| {
            command
                .first()
                .and_then(|command| command.to_str())
                .and_then(binary_name_from_text)
                .filter(|name| is_known_client_binary_name(name))
        })
}

fn observed_client_from_tokens<T: AsRef<OsStr>>(
    pid: u32,
    binary_name: String,
    binary_path: Option<String>,
    tokens: &[T],
    started_at_epoch_seconds: u64,
    run_time_seconds: u64,
) -> Option<ObservedClientInfo> {
    let safe_command = parse_safe_client_command_tokens(tokens)?;
    Some(observed_client_from_safe_command(
        pid,
        binary_name,
        binary_path,
        safe_command,
        started_at_epoch_seconds,
        run_time_seconds,
    ))
}

fn observed_client_from_safe_command(
    pid: u32,
    binary_name: String,
    binary_path: Option<String>,
    command: SafeClientCommand,
    started_at_epoch_seconds: u64,
    run_time_seconds: u64,
) -> ObservedClientInfo {
    ObservedClientInfo {
        pid,
        binary_name,
        binary_path,
        client_id: command.client_id,
        api_url: command.api_url,
        rpc_url: command.rpc_url,
        started_at_epoch_seconds: Some(started_at_epoch_seconds),
        run_time_seconds: Some(run_time_seconds),
        secret_argument_present: command.secret_argument_present,
    }
}

fn binary_name_from_path(path: &Path) -> Option<String> {
    path.file_name().and_then(OsStr::to_str).map(str::to_string)
}

fn binary_name_from_text(value: &str) -> Option<String> {
    binary_name_from_path(Path::new(value))
}

fn is_known_client_binary_name(value: &str) -> bool {
    let name = Path::new(value)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(value)
        .to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);

    CLIENT_BINARY_NAMES.iter().any(|known| name == *known) || name.starts_with("frp-panel-client-")
}

fn discover_startup_items() -> Vec<StartupItemInfo> {
    #[cfg(target_os = "macos")]
    {
        return discover_macos_launch_agents();
    }

    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
fn discover_macos_launch_agents() -> Vec<StartupItemInfo> {
    let mut paths = HashSet::new();
    for directory in macos_launch_agent_directories() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(OsStr::to_str) == Some("plist") {
                paths.insert(path);
            }
        }
    }

    let mut items = paths
        .into_iter()
        .filter_map(|path| {
            let value = plist::Value::from_file(&path).ok()?;
            launch_agent_item_from_value(&path, &value)
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.path.cmp(&right.path));
    items
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_directories() -> Vec<PathBuf> {
    let mut directories = vec![PathBuf::from("/Library/LaunchAgents")];
    if let Some(home) = env::var_os("HOME") {
        directories.insert(0, PathBuf::from(home).join("Library/LaunchAgents"));
    }
    directories
}

#[cfg(target_os = "macos")]
fn launch_agent_item_from_value(path: &Path, value: &plist::Value) -> Option<StartupItemInfo> {
    let dictionary = value.as_dictionary()?;
    let program = dictionary.get("Program").and_then(plist::Value::as_string);
    let mut arguments = dictionary
        .get("ProgramArguments")
        .and_then(plist::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(plist::Value::as_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(program) = program {
        if arguments.first().copied() != Some(program) {
            arguments.insert(0, program);
        }
    }

    arguments
        .first()
        .and_then(|argument| binary_name_from_text(argument))
        .filter(|name| is_known_client_binary_name(name))?;
    let safe_command = parse_safe_client_command_tokens(&arguments)?;

    Some(StartupItemInfo {
        label: dictionary
            .get("Label")
            .and_then(plist::Value::as_string)
            .map(str::to_string)
            .unwrap_or_else(|| path.display().to_string()),
        path: path.display().to_string(),
        kind: "macos-launch-agent".to_string(),
        client_id: safe_command.client_id,
        api_url: safe_command.api_url,
        rpc_url: safe_command.rpc_url,
        secret_argument_present: safe_command.secret_argument_present,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        dedupe_binaries, find_client_id_conflict, is_known_client_binary_name,
        observed_client_from_tokens, ExternalBinaryInfo, ObservedClientInfo,
    };

    #[test]
    fn recognizes_supported_client_binary_names() {
        assert!(is_known_client_binary_name("frp-panel"));
        assert!(is_known_client_binary_name("frp-panel-client"));
        assert!(is_known_client_binary_name(
            "frp-panel-client-aarch64-apple-darwin"
        ));
        assert!(is_known_client_binary_name("frp-panel-client.exe"));
        assert!(!is_known_client_binary_name("frp-panel-mac-client"));
    }

    #[test]
    fn observes_client_commands_without_retaining_secret() {
        let tokens = [
            "frp-panel",
            "client",
            "--secret=external-secret-must-not-leak",
            "--id=user.c.macos",
            "--api-url",
            "https://server.example.com",
            "--rpc-url=wss://server.example.com",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        let observed = observed_client_from_tokens(
            42,
            "frp-panel".to_string(),
            Some("/usr/local/bin/frp-panel".to_string()),
            &tokens,
            1_700_000_000,
            12,
        )
        .unwrap();

        assert_eq!(observed.client_id.as_deref(), Some("user.c.macos"));
        assert_eq!(
            observed.api_url.as_deref(),
            Some("https://server.example.com")
        );
        assert_eq!(
            observed.rpc_url.as_deref(),
            Some("wss://server.example.com")
        );
        assert!(observed.secret_argument_present);
        assert!(!format!("{observed:?}").contains("external-secret-must-not-leak"));
    }

    #[test]
    fn ignores_non_client_commands() {
        let tokens = [
            "frp-panel",
            "server",
            "--api-url",
            "https://server.example.com",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        assert!(observed_client_from_tokens(
            42,
            "frp-panel".to_string(),
            None,
            &tokens,
            1_700_000_000,
            12,
        )
        .is_none());
    }

    #[test]
    fn detects_matching_client_id_conflicts() {
        let clients = vec![ObservedClientInfo {
            pid: 42,
            binary_name: "frp-panel".to_string(),
            binary_path: Some("/usr/local/bin/frp-panel".to_string()),
            client_id: Some("user.c.macos".to_string()),
            api_url: None,
            rpc_url: None,
            started_at_epoch_seconds: None,
            run_time_seconds: None,
            secret_argument_present: true,
        }];

        assert_eq!(
            find_client_id_conflict(&clients, "user.c.macos")
                .unwrap()
                .pid,
            42
        );
        assert!(find_client_id_conflict(&clients, "another.client").is_none());
    }

    #[test]
    fn deduplicates_binary_paths() {
        let binaries = vec![
            ExternalBinaryInfo {
                name: "frp-panel".to_string(),
                path: "/usr/local/bin/frp-panel".to_string(),
                source: "PATH".to_string(),
            },
            ExternalBinaryInfo {
                name: "frp-panel".to_string(),
                path: "/usr/local/bin/frp-panel".to_string(),
                source: "常用目录".to_string(),
            },
        ];

        assert_eq!(dedupe_binaries(binaries).len(), 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn parses_macos_launch_agent_without_exposing_secret() {
        use std::path::Path;

        use plist::{Dictionary, Value};

        let mut dictionary = Dictionary::new();
        dictionary.insert(
            "Label".to_string(),
            Value::String("frp-panel.client".to_string()),
        );
        dictionary.insert(
            "ProgramArguments".to_string(),
            Value::Array(vec![
                Value::String("/usr/local/bin/frp-panel".to_string()),
                Value::String("client".to_string()),
                Value::String("-s".to_string()),
                Value::String("external-secret-must-not-leak".to_string()),
                Value::String("-i".to_string()),
                Value::String("user.c.macos".to_string()),
            ]),
        );

        let item = super::launch_agent_item_from_value(
            Path::new("/Users/example/Library/LaunchAgents/frp-panel.client.plist"),
            &Value::Dictionary(dictionary),
        )
        .unwrap();

        assert_eq!(item.client_id.as_deref(), Some("user.c.macos"));
        assert!(item.secret_argument_present);
        assert!(!format!("{item:?}").contains("external-secret-must-not-leak"));
    }
}

fn find_client_id_conflict<'a>(
    clients: &'a [ObservedClientInfo],
    client_id: &str,
) -> Option<&'a ObservedClientInfo> {
    clients
        .iter()
        .find(|client| client.client_id.as_deref() == Some(client_id))
}
