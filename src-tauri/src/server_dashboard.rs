use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{redirect::Policy, Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::server_config::{load_managed_server_config, load_server_profile};
use crate::types::{
    NativeConfigSource, ServerDashboardClient, ServerDashboardProxy, ServerDashboardStatus,
};

const DASHBOARD_PAGE_SIZE: i64 = 200;
const DASHBOARD_MAX_PAGES: i64 = 50;
const DASHBOARD_REQUEST_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Deserialize, Default)]
struct FrpsConfig {
    #[serde(rename = "webServer", default)]
    web_server: WebServerConfig,
}

#[derive(Debug, Deserialize, Default)]
struct WebServerConfig {
    #[serde(default)]
    addr: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    user: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    tls: Option<WebServerTlsConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct WebServerTlsConfig {}

#[derive(Debug, Deserialize, Default)]
struct V2SystemInfo {
    #[serde(default)]
    version: String,
    #[serde(default)]
    status: V2SystemStatus,
}

#[derive(Debug, Deserialize, Default)]
struct V2SystemStatus {
    #[serde(default, rename = "totalTrafficIn")]
    total_traffic_in: i64,
    #[serde(default, rename = "totalTrafficOut")]
    total_traffic_out: i64,
    #[serde(default, rename = "curConns")]
    current_connections: i64,
    #[serde(default, rename = "clientCounts")]
    client_counts: i64,
}

#[derive(Debug, Deserialize, Default)]
struct V2Page<T> {
    #[serde(default)]
    total: i64,
    #[serde(default)]
    items: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct V2Envelope<T> {
    #[serde(default)]
    code: u16,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    data: Option<T>,
}

#[derive(Debug, Deserialize, Default)]
struct V2Client {
    #[serde(default)]
    key: String,
    #[serde(default)]
    user: String,
    #[serde(default, rename = "clientID")]
    client_id: String,
    #[serde(default, rename = "runID")]
    run_id: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    hostname: String,
    #[serde(default, rename = "clientIP")]
    client_ip: String,
    #[serde(default, rename = "firstConnectedAt")]
    first_connected_at: i64,
    #[serde(default, rename = "lastConnectedAt")]
    last_connected_at: i64,
    #[serde(default, rename = "disconnectedAt")]
    disconnected_at: i64,
    #[serde(default)]
    online: bool,
}

#[derive(Debug, Deserialize, Default)]
struct V2Proxy {
    #[serde(default)]
    name: String,
    #[serde(default)]
    user: String,
    #[serde(default, rename = "clientID")]
    client_id: String,
    #[serde(default)]
    spec: V2ProxySpec,
    #[serde(default)]
    status: V2ProxyStatus,
}

#[derive(Debug, Deserialize, Default)]
struct V2ProxySpec {
    #[serde(default, rename = "type")]
    proxy_type: String,
    #[serde(flatten)]
    fields: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Deserialize, Default)]
struct V2ProxyStatus {
    #[serde(default, rename = "phase")]
    state: String,
    #[serde(default, rename = "todayTrafficIn")]
    today_traffic_in: i64,
    #[serde(default, rename = "todayTrafficOut")]
    today_traffic_out: i64,
    #[serde(default, rename = "curConns")]
    current_connections: i64,
    #[serde(default, rename = "lastStartAt")]
    last_start_at: i64,
    #[serde(default, rename = "lastCloseAt")]
    last_close_at: i64,
}

#[tauri::command]
pub async fn get_server_dashboard_status(app: AppHandle) -> ServerDashboardStatus {
    let (endpoint, user, password) = match load_dashboard_context(&app) {
        Ok(value) => value,
        Err(error) => return unavailable(None, error),
    };

    let response = fetch_dashboard(&endpoint, &user, &password).await;
    match response {
        Ok(mut status) => {
            status.endpoint = Some(endpoint);
            status
        }
        Err(error) => unavailable(Some(endpoint), error),
    }
}

fn load_dashboard_context(app: &AppHandle) -> Result<(String, String, String), String> {
    let profile = load_server_profile(app.clone(), None)?
        .ok_or_else(|| "尚未选择本机 frps Server Profile".to_string())?;
    if profile.native.source != NativeConfigSource::Managed {
        return Err("外部只读 frps 配置不会被 App 读取；请使用 App 托管配置查看 Dashboard".into());
    }

    let content = load_managed_server_config(app.clone(), profile.id)?;
    let config: FrpsConfig =
        toml::from_str(&content).map_err(|_| "frps TOML 无法解析 Dashboard 配置".to_string())?;
    dashboard_endpoint(&config.web_server)
}

fn dashboard_endpoint(config: &WebServerConfig) -> Result<(String, String, String), String> {
    if config.port == 0 {
        return Err("frps 未配置 webServer.port，无法读取 Dashboard".into());
    }

    let mut host = config.addr.trim().to_string();
    if host.is_empty() || host == "0.0.0.0" {
        host = "127.0.0.1".into();
    } else if host == "::" {
        host = "::1".into();
    }
    if host.contains('/') || host.contains('@') {
        return Err("frps Dashboard 地址格式不受支持".into());
    }

    let url_host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    let scheme = if config.tls.is_some() {
        "https"
    } else {
        "http"
    };
    let endpoint = format!("{scheme}://{url_host}:{}", config.port);
    reqwest::Url::parse(&endpoint).map_err(|_| "frps Dashboard 地址格式无效".to_string())?;
    Ok((endpoint, config.user.clone(), config.password.clone()))
}

async fn fetch_dashboard(
    endpoint: &str,
    user: &str,
    password: &str,
) -> Result<ServerDashboardStatus, String> {
    let client = Client::builder()
        .timeout(DASHBOARD_REQUEST_TIMEOUT)
        .redirect(Policy::none())
        .build()
        .map_err(|_| "无法创建 Dashboard HTTP 客户端".to_string())?;

    let system: V2SystemInfo =
        get_json(&client, endpoint, "/api/v2/system/info", user, password).await?;
    let clients: Vec<V2Client> =
        fetch_all_pages(&client, endpoint, "/api/v2/clients", user, password).await?;
    let proxies: Vec<V2Proxy> =
        fetch_all_pages(&client, endpoint, "/api/v2/proxies", user, password).await?;

    let online_clients = clients.iter().filter(|client| client.online).count() as i64;
    let online_proxies = proxies
        .iter()
        .filter(|proxy| proxy.status.state.eq_ignore_ascii_case("online"))
        .count() as i64;

    Ok(ServerDashboardStatus {
        available: true,
        endpoint: None,
        version: Some(system.version),
        total_traffic_in: system.status.total_traffic_in,
        total_traffic_out: system.status.total_traffic_out,
        current_connections: system.status.current_connections,
        client_counts: system.status.client_counts,
        online_clients,
        proxy_counts: proxies.len() as i64,
        online_proxies,
        clients: clients.into_iter().map(map_client).collect(),
        proxies: proxies.into_iter().map(map_proxy).collect(),
        refreshed_at_ms: now_ms(),
        error: None,
    })
}

async fn fetch_all_pages<T: DeserializeOwned + Default>(
    client: &Client,
    endpoint: &str,
    path: &str,
    user: &str,
    password: &str,
) -> Result<Vec<T>, String> {
    let mut page_number = 1_i64;
    let mut items = Vec::new();
    loop {
        let page: V2Page<T> = get_json_with_query(
            client,
            endpoint,
            path,
            &[
                ("page", page_number.to_string()),
                ("pageSize", DASHBOARD_PAGE_SIZE.to_string()),
            ],
            user,
            password,
        )
        .await?;
        let page_len = page.items.len();
        items.extend(page.items);
        if page_len == 0
            || items.len() as i64 >= page.total
            || page_len < DASHBOARD_PAGE_SIZE as usize
        {
            break;
        }
        page_number += 1;
        if page_number > DASHBOARD_MAX_PAGES {
            return Err("frps Dashboard 返回的记录过多，已停止分页读取".into());
        }
    }
    Ok(items)
}

async fn get_json<T: DeserializeOwned + Default>(
    client: &Client,
    endpoint: &str,
    path: &str,
    user: &str,
    password: &str,
) -> Result<T, String> {
    get_json_with_query(client, endpoint, path, &[], user, password).await
}

async fn get_json_with_query<T: DeserializeOwned + Default>(
    client: &Client,
    endpoint: &str,
    path: &str,
    query: &[(&str, String)],
    user: &str,
    password: &str,
) -> Result<T, String> {
    let url = format!("{endpoint}{path}");
    let mut request = client.get(url);
    if !query.is_empty() {
        request = request.query(query);
    }
    if !user.is_empty() || !password.is_empty() {
        request = request.basic_auth(user, Some(password));
    }
    let response = request.send().await.map_err(|_| {
        "无法连接 frps Dashboard，请确认 frps 正在运行且 Dashboard 地址可达".to_string()
    })?;
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err("frps Dashboard 认证失败，请检查 webServer.user/password".into());
    }
    if !status.is_success() {
        return Err(format!(
            "frps Dashboard 返回 HTTP {}，可能不支持当前 API",
            status.as_u16()
        ));
    }
    let envelope = response
        .json::<V2Envelope<T>>()
        .await
        .map_err(|_| "frps Dashboard 返回了无法识别的数据".to_string())?;
    if envelope.code != 200 {
        return Err(if envelope.msg.is_empty() {
            "frps Dashboard API 返回错误".into()
        } else {
            format!("frps Dashboard API 返回错误：{}", envelope.msg)
        });
    }
    envelope
        .data
        .ok_or_else(|| "frps Dashboard API 没有返回数据".to_string())
}

fn map_client(client: V2Client) -> ServerDashboardClient {
    ServerDashboardClient {
        key: client.key,
        user: client.user,
        client_id: client.client_id,
        run_id: client.run_id,
        version: client.version,
        hostname: client.hostname,
        client_ip: client.client_ip,
        first_connected_at: client.first_connected_at,
        last_connected_at: client.last_connected_at,
        disconnected_at: client.disconnected_at,
        online: client.online,
    }
}

fn map_proxy(proxy: V2Proxy) -> ServerDashboardProxy {
    let remote_port = ["tcp", "udp"]
        .iter()
        .find_map(|key| proxy.spec.fields.get(*key))
        .and_then(|value| value.get("remotePort"))
        .and_then(Value::as_i64);

    let mut domains = Vec::new();
    for key in ["http", "https", "tcpmux"] {
        if let Some(value) = proxy.spec.fields.get(key) {
            if let Some(items) = value.get("customDomains").and_then(Value::as_array) {
                domains.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
            }
            if let Some(subdomain) = value.get("subdomain").and_then(Value::as_str) {
                if !subdomain.is_empty() {
                    domains.push(subdomain.to_string());
                }
            }
        }
    }
    domains.sort();
    domains.dedup();

    ServerDashboardProxy {
        name: proxy.name,
        user: proxy.user,
        client_id: proxy.client_id,
        proxy_type: proxy.spec.proxy_type,
        state: proxy.status.state,
        today_traffic_in: proxy.status.today_traffic_in,
        today_traffic_out: proxy.status.today_traffic_out,
        current_connections: proxy.status.current_connections,
        last_start_at: proxy.status.last_start_at,
        last_close_at: proxy.status.last_close_at,
        remote_port,
        domains,
    }
}

fn unavailable(endpoint: Option<String>, error: String) -> ServerDashboardStatus {
    ServerDashboardStatus {
        available: false,
        endpoint,
        version: None,
        total_traffic_in: 0,
        total_traffic_out: 0,
        current_connections: 0,
        client_counts: 0,
        online_clients: 0,
        proxy_counts: 0,
        online_proxies: 0,
        clients: Vec::new(),
        proxies: Vec::new(),
        refreshed_at_ms: now_ms(),
        error: Some(error),
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::{dashboard_endpoint, fetch_dashboard, FrpsConfig};

    #[test]
    fn dashboard_endpoint_maps_wildcard_to_loopback() {
        let config: FrpsConfig = toml::from_str(
            r#"
            [webServer]
            addr = "0.0.0.0"
            port = 7500
            user = "admin"
            password = "never-returned"
            "#,
        )
        .expect("valid TOML");
        let (endpoint, user, password) = dashboard_endpoint(&config.web_server).expect("endpoint");
        assert_eq!(endpoint, "http://127.0.0.1:7500");
        assert_eq!(user, "admin");
        assert_eq!(password, "never-returned");
    }

    #[test]
    fn dashboard_endpoint_requires_a_port() {
        let config = FrpsConfig::default();
        assert!(dashboard_endpoint(&config.web_server).is_err());
    }

    #[tokio::test]
    async fn fetch_dashboard_maps_wrapped_v2_responses() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().expect("request");
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut request_line = String::new();
                reader.read_line(&mut request_line).expect("request line");
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).expect("header line");
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }

                let body = if request_line.contains("/api/v2/system/info") {
                    r#"{"code":200,"msg":"success","data":{"version":"0.71.0","status":{"totalTrafficIn":128,"totalTrafficOut":256,"curConns":3,"clientCounts":1}}}"#
                } else if request_line.contains("/api/v2/clients") {
                    r#"{"code":200,"msg":"success","data":{"total":1,"page":1,"pageSize":200,"items":[{"key":"client-key","clientID":"client-a","hostname":"mac-mini","clientIP":"10.0.0.8","online":true,"lastConnectedAt":123}]}}"#
                } else {
                    r#"{"code":200,"msg":"success","data":{"total":1,"page":1,"pageSize":200,"items":[{"name":"ssh","clientID":"client-a","spec":{"type":"tcp","tcp":{"remotePort":60022}},"status":{"phase":"online","todayTrafficIn":5,"todayTrafficOut":7,"curConns":1}}]}}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).expect("response");
            }
        });

        let status = fetch_dashboard(&format!("http://{address}"), "admin", "password")
            .await
            .expect("dashboard status");
        assert!(status.available);
        assert_eq!(status.version.as_deref(), Some("0.71.0"));
        assert_eq!(status.total_traffic_in, 128);
        assert_eq!(status.total_traffic_out, 256);
        assert_eq!(status.online_clients, 1);
        assert_eq!(status.online_proxies, 1);
        assert_eq!(status.clients[0].client_id, "client-a");
        assert_eq!(status.proxies[0].remote_port, Some(60022));
        assert_eq!(status.proxies[0].today_traffic_out, 7);
        server.join().expect("server thread");
    }
}
