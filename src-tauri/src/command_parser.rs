use crate::types::ConnectionConfig;

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
    use super::parse_panel_command_inner;

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
}
