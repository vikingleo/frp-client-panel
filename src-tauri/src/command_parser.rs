use std::ffi::OsStr;

use crate::types::ConnectionConfig;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SafeClientCommand {
    pub client_id: Option<String>,
    pub api_url: Option<String>,
    pub rpc_url: Option<String>,
    pub secret_argument_present: bool,
}

#[tauri::command]
pub fn parse_panel_command(command: String) -> Result<ConnectionConfig, String> {
    parse_panel_command_inner(&command)
}

pub fn parse_panel_command_inner(command: &str) -> Result<ConnectionConfig, String> {
    let tokens = shell_split(command)?;
    if tokens.is_empty() {
        return Err("命令为空".into());
    }

    let start = tokens
        .iter()
        .position(|t| t == "client")
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let mut client_secret = String::new();
    let mut client_id = String::new();
    let mut api_url = String::new();
    let mut rpc_url = String::new();

    let mut i = start;
    while i < tokens.len() {
        let token = &tokens[i];
        match token.as_str() {
            "-s" | "--secret" => {
                i += 1;
                client_secret = tokens.get(i).cloned().unwrap_or_default();
            }
            "-i" | "--id" => {
                i += 1;
                client_id = tokens.get(i).cloned().unwrap_or_default();
            }
            "--api-url" => {
                i += 1;
                api_url = tokens.get(i).cloned().unwrap_or_default();
            }
            "--rpc-url" => {
                i += 1;
                rpc_url = tokens.get(i).cloned().unwrap_or_default();
            }
            _ if token.starts_with("--api-url=") => {
                api_url = token.trim_start_matches("--api-url=").to_string();
            }
            _ if token.starts_with("--rpc-url=") => {
                rpc_url = token.trim_start_matches("--rpc-url=").to_string();
            }
            _ if token.starts_with("--secret=") => {
                client_secret = token.trim_start_matches("--secret=").to_string();
            }
            _ if token.starts_with("--id=") => {
                client_id = token.trim_start_matches("--id=").to_string();
            }
            _ => {}
        }
        i += 1;
    }

    let config = ConnectionConfig {
        client_id,
        client_secret,
        api_url,
        rpc_url,
        auto_connect: false,
        launch_at_login: false,
        allow_insecure_tls: false,
    };
    config.validate()?;
    Ok(config)
}

/// Extracts only non-sensitive client metadata from an already tokenized command.
///
/// This parser intentionally never copies, returns, logs, or persists an external Client Secret.
/// It is used for observing processes and startup items that were not started by this application.
pub(crate) fn parse_safe_client_command_tokens<T: AsRef<OsStr>>(
    tokens: &[T],
) -> Option<SafeClientCommand> {
    let start = tokens
        .iter()
        .position(|token| safe_token_text(token) == Some("client"))?
        + 1;
    let mut command = SafeClientCommand::default();
    let mut index = start;

    while index < tokens.len() {
        let Some(token) = safe_token_text(&tokens[index]) else {
            index += 1;
            continue;
        };
        match token {
            "-s" | "--secret" => {
                command.secret_argument_present = true;
                // Skip the following token without reading or copying its value.
                index += 2;
                continue;
            }
            "-i" | "--id" => {
                index += 1;
                command.client_id = safe_token_at(tokens, index);
            }
            "--api-url" => {
                index += 1;
                command.api_url = safe_token_at(tokens, index);
            }
            "--rpc-url" => {
                index += 1;
                command.rpc_url = safe_token_at(tokens, index);
            }
            _ if token.starts_with("--secret=") => {
                command.secret_argument_present = true;
            }
            _ if token.starts_with("--id=") => {
                command.client_id = Some(token.trim_start_matches("--id=").to_string());
            }
            _ if token.starts_with("--api-url=") => {
                command.api_url = Some(token.trim_start_matches("--api-url=").to_string());
            }
            _ if token.starts_with("--rpc-url=") => {
                command.rpc_url = Some(token.trim_start_matches("--rpc-url=").to_string());
            }
            _ => {}
        }
        index += 1;
    }

    Some(command)
}

fn safe_token_text<T: AsRef<OsStr>>(token: &T) -> Option<&str> {
    token.as_ref().to_str()
}

fn safe_token_at<T: AsRef<OsStr>>(tokens: &[T], index: usize) -> Option<String> {
    tokens
        .get(index)
        .and_then(safe_token_text)
        .map(str::to_string)
}

fn shell_split(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            if quote == Some('\'') {
                current.push(ch);
            } else {
                escaped = true;
            }
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            ' ' | '\n' | '\t' | '\r' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                while matches!(chars.peek(), Some(' ' | '\n' | '\t' | '\r')) {
                    chars.next();
                }
            }
            _ => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }
    if let Some(q) = quote {
        return Err(format!("命令中存在未闭合的引号：{q}"));
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::{parse_panel_command_inner, parse_safe_client_command_tokens};

    #[test]
    fn parses_direct_command() {
        let cfg = parse_panel_command_inner(
            "frp-panel client -s abc -i user.c.mac --api-url https://example.com --rpc-url wss://example.com",
        )
        .unwrap();
        assert_eq!(cfg.client_secret, "abc");
        assert_eq!(cfg.client_id, "user.c.mac");
        assert_eq!(cfg.api_url, "https://example.com");
        assert_eq!(cfg.rpc_url, "wss://example.com");
        assert!(!cfg.allow_insecure_tls);
    }

    #[test]
    fn parses_install_command() {
        let cfg = parse_panel_command_inner(
            "curl -fSL https://raw.githubusercontent.com/VaalaCat/frp-panel/main/install.sh | bash -s -- client -s 'abc def' -i user.c.mac --api-url=https://api.example.com --rpc-url=grpc://rpc.example.com:9001",
        )
        .unwrap();
        assert_eq!(cfg.client_secret, "abc def");
        assert_eq!(cfg.api_url, "https://api.example.com");
        assert_eq!(cfg.rpc_url, "grpc://rpc.example.com:9001");
    }

    #[test]
    fn safe_parser_never_returns_external_secret() {
        let tokens = [
            "frp-panel",
            "client",
            "-s",
            "external-secret-must-not-leak",
            "-i",
            "user.c.macos",
            "--api-url=https://api.example.com",
            "--rpc-url",
            "wss://rpc.example.com",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        let parsed = parse_safe_client_command_tokens(&tokens).unwrap();

        assert!(parsed.secret_argument_present);
        assert_eq!(parsed.client_id.as_deref(), Some("user.c.macos"));
        assert_eq!(parsed.api_url.as_deref(), Some("https://api.example.com"));
        assert_eq!(parsed.rpc_url.as_deref(), Some("wss://rpc.example.com"));
        assert!(!format!("{parsed:?}").contains("external-secret-must-not-leak"));
    }

    #[test]
    fn safe_parser_ignores_non_client_commands() {
        let tokens = [
            "frp-panel",
            "server",
            "--api-url",
            "https://api.example.com",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        assert_eq!(parse_safe_client_command_tokens(&tokens), None);
    }
}
