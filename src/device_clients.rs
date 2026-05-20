use crate::app::DeviceConfig;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "ssr")]
use serde_json::{json, Value};

#[cfg(feature = "ssr")]
use reqwest::Response;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceConnectionStatus {
    pub connected: bool,
    pub title: String,
    pub summary: String,
    pub details: Vec<DeviceConnectionDetail>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceConnectionDetail {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Error)]
pub enum DeviceClientError {
    #[error("missing device host")]
    MissingHost,
    #[error("missing device username")]
    MissingUsername,
    #[error("missing device password")]
    MissingPassword,
    #[error("unsupported device type: {0}")]
    UnsupportedDeviceType(String),
    #[cfg(feature = "ssr")]
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[cfg(feature = "ssr")]
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("device login failed: {0}")]
    DeviceLogin(String),
    #[error("device call failed: {0}")]
    DeviceCall(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevicePlatform {
    OpenWrt,
    Teltonika,
}

impl DevicePlatform {
    pub fn parse(value: Option<&str>) -> Self {
        match value.unwrap_or("openwrt").trim().to_lowercase().as_str() {
            "teltonika" => Self::Teltonika,
            "openwrt" | "opnwrt" | "open-wrt" => Self::OpenWrt,
            _ => Self::OpenWrt,
        }
    }
}

#[cfg(feature = "ssr")]
fn normalized_host(device: &DeviceConfig) -> Result<String, DeviceClientError> {
    let host = device
        .ip_address
        .as_deref()
        .ok_or(DeviceClientError::MissingHost)?
        .trim();

    if host.is_empty() {
        return Err(DeviceClientError::MissingHost);
    }

    Ok(host.trim_start_matches("http://").trim_start_matches("https://").to_string())
}

#[cfg(feature = "ssr")]
fn base_url(device: &DeviceConfig) -> Result<String, DeviceClientError> {
    let raw = device
        .ip_address
        .as_deref()
        .ok_or(DeviceClientError::MissingHost)?
        .trim();

    if raw.is_empty() {
        return Err(DeviceClientError::MissingHost);
    }

    if raw.starts_with("http://") || raw.starts_with("https://") {
        Ok(raw.trim_end_matches('/').to_string())
    } else {
        Ok(format!("http://{}", raw.trim_end_matches('/')))
    }
}

#[cfg(feature = "ssr")]
fn build_http_client(device: &DeviceConfig) -> Result<reqwest::Client, DeviceClientError> {
    let url = base_url(device)?;
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10));

    if url.starts_with("https://") {
        builder = builder.danger_accept_invalid_certs(true);
    }

    Ok(builder.build()?)
}

#[cfg(feature = "ssr")]
async fn parse_json_response(response: Response, context: &str) -> Result<Value, DeviceClientError> {
    let response = response.error_for_status()?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let body = response.text().await?;
    let trimmed = body.trim_start();

    match serde_json::from_str::<Value>(&body) {
        Ok(value) => Ok(value),
        Err(error) => {
            let snippet: String = trimmed.chars().take(120).collect();
            let response_hint = if trimmed.starts_with("<!doctype html") || trimmed.starts_with("<html") {
                "received HTML instead of JSON; /ubus is serving the web UI rather than the JSON-RPC endpoint. On RutOS 07.18+ this usually means the JSON-RPC support package is not installed or the endpoint is disabled"
            } else {
                "received a non-JSON response"
            };

            let content_type_hint = content_type
                .as_deref()
                .map(|value| format!("content-type {value}"))
                .unwrap_or_else(|| "no content-type header".to_string());

            Err(DeviceClientError::DeviceCall(format!(
                "{context}: {response_hint} ({content_type_hint}); response starts with {:?}; json error: {error}",
                snippet
            )))
        }
    }
}

#[cfg(feature = "ssr")]
fn extract_openwrt_token(response: &Value) -> Option<String> {
    // LuCI auth login can return token as:
    // 1. {"result": "token"}
    // 2. {"result": [0, "token"]}
    // 3. {"result": {"token": "token"}}
    if let Some(token) = response
        .get("result")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
    {
        return Some(token.to_string());
    }

    if let Some(arr) = response.get("result").and_then(|value| value.as_array()) {
        if let Some(token) = arr
            .iter()
            .find_map(|value| value.as_str())
            .filter(|value| !value.is_empty())
        {
            return Some(token.to_string());
        }
    }

    response
        .get("result")
        .and_then(|value| value.as_object())
        .and_then(|obj| obj.get("token"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(feature = "ssr")]
fn extract_luci_exec_result(response: &Value) -> Result<Option<String>, DeviceClientError> {
    if let Some(error_obj) = response.get("error").and_then(|v| v.as_object()) {
        let code = error_obj.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        let message = error_obj
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        if code != 0 {
            return Err(DeviceClientError::DeviceCall(format!(
                "LuCI exec call failed: {message} (code {code})"
            )));
        }
    }

    Ok(response
        .get("result")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string))
}

#[cfg(feature = "ssr")]
async fn luci_exec_sys(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    command: &str,
) -> Result<Option<String>, DeviceClientError> {
    let payload = json!({
        "id": 1,
        "method": "exec",
        "params": [command],
    });
    let response = client
        .post(format!("{base_url}/cgi-bin/luci/rpc/sys?auth={token}"))
        .json(&payload)
        .send()
        .await?;
    let body = parse_json_response(response, "LuCI sys.exec failed").await?;
    extract_luci_exec_result(&body)
}

#[cfg(feature = "ssr")]
fn format_memory_kb(total_kb: u64, avail_kb: u64) -> String {
    let used_mb = total_kb.saturating_sub(avail_kb) / 1024;
    let total_mb = total_kb / 1024;
    format!("{used_mb} / {total_mb} MB")
}

#[cfg(feature = "ssr")]
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    match (days, hours, minutes) {
        (0, 0, m) => format!("{m}m"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

#[cfg(feature = "ssr")]
fn push_iface_details(iface: &Value, prefix: &str, details: &mut Vec<DeviceConnectionDetail>) {
    let up = iface.get("up").and_then(|v| v.as_bool()).unwrap_or(false);
    details.push(DeviceConnectionDetail {
        label: format!("{prefix} Connected"),
        value: if up { "Yes".to_string() } else { "No".to_string() },
    });

    if let Some(ip) = iface
        .get("ipv4-address")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|addr| addr.get("address"))
        .and_then(|v| v.as_str())
    {
        details.push(DeviceConnectionDetail {
            label: format!("{prefix} IP"),
            value: ip.to_string(),
        });
    }

    if let Some(ip6) = iface
        .get("ipv6-address")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|addr| addr.get("address"))
        .and_then(|v| v.as_str())
    {
        details.push(DeviceConnectionDetail {
            label: format!("{prefix} IPv6"),
            value: ip6.to_string(),
        });
    }

    let dns: Vec<&str> = iface
        .get("dns-server")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    if !dns.is_empty() {
        details.push(DeviceConnectionDetail {
            label: format!("{prefix} DNS"),
            value: dns.join(", "),
        });
    }
}

#[cfg(feature = "ssr")]
fn username(device: &DeviceConfig) -> Result<String, DeviceClientError> {
    device
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or(DeviceClientError::MissingUsername)
}

#[cfg(feature = "ssr")]
fn password(device: &DeviceConfig) -> Result<String, DeviceClientError> {
    device
        .password
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or(DeviceClientError::MissingPassword)
}

#[cfg(feature = "ssr")]
pub async fn probe_device(device: &DeviceConfig) -> Result<DeviceConnectionStatus, DeviceClientError> {
    let platform = DevicePlatform::parse(device.device_type.as_deref());

    match platform {
        DevicePlatform::OpenWrt => probe_openwrt(device).await,
        DevicePlatform::Teltonika => probe_teltonika(device).await,
    }
}

#[cfg(not(feature = "ssr"))]
pub async fn probe_device(_device: &DeviceConfig) -> Result<DeviceConnectionStatus, DeviceClientError> {
    Err(DeviceClientError::UnsupportedDeviceType(
        "device communication is only available on the server".to_string(),
    ))
}

#[cfg(feature = "ssr")]
async fn teltonika_login(
    client: &reqwest::Client,
    ubus_url: &str,
    username: &str,
    password: &str,
) -> Result<String, DeviceClientError> {
    let login_payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "call",
        "params": [
            "00000000000000000000000000000000",
            "session",
            "login",
            {"username": username, "password": password}
        ]
    });

    let response = client
        .post(ubus_url)
        .json(&login_payload)
        .send()
        .await?;
    let response = parse_json_response(response, "Teltonika login failed").await?;

    // result is [error_code, {ubus_rpc_session: "...", ...}]
    let session_id = response
        .get("result")
        .and_then(|r| r.as_array())
        .and_then(|arr| {
            let code = arr.first().and_then(|c| c.as_u64()).unwrap_or(1);
            if code == 0 { arr.get(1) } else { None }
        })
        .and_then(|obj| obj.get("ubus_rpc_session"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| DeviceClientError::DeviceLogin("authentication failed".to_string()))?;

    Ok(session_id)
}

#[cfg(feature = "ssr")]
async fn probe_teltonika(device: &DeviceConfig) -> Result<DeviceConnectionStatus, DeviceClientError> {
    let host = normalized_host(device)?;
    let base_url = base_url(device)?;
    let username = username(device)?;
    let password = password(device)?;
    let ubus_url = format!("{base_url}/ubus");

    let client = build_http_client(device)?;

    let session_id = teltonika_login(&client, &ubus_url, &username, &password).await?;

    let mut details = Vec::new();
    let mut title = "Teltonika".to_string();

    // Firmware version from /etc/version
    let firmware_payload = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "call",
        "params": [session_id.clone(), "file", "read", {"path": "/etc/version"}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&firmware_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika firmware query failed").await {
            if let Some(firmware) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("data"))
                .and_then(|v| v.as_str())
            {
                details.push(DeviceConnectionDetail {
                    label: "Firmware".to_string(),
                    value: firmware.trim().to_string(),
                });
            }
        }
    }

    // Model/product code via mnf_info --name
    let mnf_payload = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "call",
        "params": [session_id.clone(), "file", "exec", {"command": "mnf_info", "params": ["--name"]}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&mnf_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika model query failed").await {
            if let Some(model) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("stdout"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                title = model.to_string();
                details.push(DeviceConnectionDetail {
                    label: "Model".to_string(),
                    value: model.to_string(),
                });
            }
        }
    }

    // Serial number via mnf_info --sn
    let sn_payload = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "call",
        "params": [session_id.clone(), "file", "exec", {"command": "mnf_info", "params": ["--sn"]}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&sn_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika serial query failed").await {
            if let Some(serial) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("stdout"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                details.push(DeviceConnectionDetail {
                    label: "Serial".to_string(),
                    value: serial.to_string(),
                });
            }
        }
    }

    // Hostname from /proc/sys/kernel/hostname
    let hostname_payload = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "call",
        "params": [session_id.clone(), "file", "read", {"path": "/proc/sys/kernel/hostname"}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&hostname_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika hostname query failed").await {
            if let Some(hostname) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("data"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                details.push(DeviceConnectionDetail {
                    label: "Hostname".to_string(),
                    value: hostname.to_string(),
                });
            }
        }
    }

    // LAN + WAN details via network.interface status
    for (iface_name, prefix, id) in [("lan", "LAN", 6_u32), ("wan", "WAN", 11_u32)] {
        let iface_payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "call",
            "params": [session_id.clone(), "network.interface", "status", {"interface": iface_name}]
        });
        if let Ok(resp) = client.post(&ubus_url).json(&iface_payload).send().await {
            if let Ok(body) = parse_json_response(resp, &format!("Teltonika {prefix} query failed")).await {
                let result_obj = body
                    .get("result")
                    .and_then(|r| r.as_array())
                    .and_then(|arr| {
                        let code = arr.first().and_then(|c| c.as_u64()).unwrap_or(1);
                        if code == 0 { arr.get(1) } else { None }
                    });
                if let Some(obj) = result_obj {
                    push_iface_details(obj, prefix, &mut details);
                }
            }
        }
    }

    // Uptime from /proc/uptime ("<seconds_up> <seconds_idle>")
    let uptime_payload = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "call",
        "params": [session_id.clone(), "file", "read", {"path": "/proc/uptime"}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&uptime_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika uptime query failed").await {
            if let Some(uptime_str) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("data"))
                .and_then(|v| v.as_str())
            {
                if let Some(secs) = uptime_str.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                    details.push(DeviceConnectionDetail {
                        label: "Uptime".to_string(),
                        value: format_uptime(secs as u64),
                    });
                }
            }
        }
    }

    // Memory from /proc/meminfo
    let meminfo_payload = json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "call",
        "params": [session_id.clone(), "file", "read", {"path": "/proc/meminfo"}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&meminfo_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika meminfo query failed").await {
            if let Some(data) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("data"))
                .and_then(|v| v.as_str())
            {
                let mut total_kb: Option<u64> = None;
                let mut avail_kb: Option<u64> = None;
                for line in data.lines() {
                    if line.starts_with("MemTotal:") {
                        total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
                    } else if line.starts_with("MemAvailable:") {
                        avail_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
                    }
                }
                if let (Some(total), Some(avail)) = (total_kb, avail_kb) {
                    details.push(DeviceConnectionDetail {
                        label: "Memory".to_string(),
                        value: format_memory_kb(total, avail),
                    });
                }
            }
        }
    }

    // Kernel version from /proc/version
    let version_payload = json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "call",
        "params": [session_id.clone(), "file", "read", {"path": "/proc/version"}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&version_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika kernel query failed").await {
            if let Some(data) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("data"))
                .and_then(|v| v.as_str())
            {
                // "Linux version 5.10.176 (buildbot@buildbot) ..."
                if let Some(kernel) = data.split_whitespace().nth(2) {
                    details.push(DeviceConnectionDetail {
                        label: "Kernel".to_string(),
                        value: kernel.to_string(),
                    });
                }
            }
        }
    }

    // CPU load from /proc/loadavg
    let loadavg_payload = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "call",
        "params": [session_id.clone(), "file", "read", {"path": "/proc/loadavg"}]
    });
    if let Ok(resp) = client.post(&ubus_url).json(&loadavg_payload).send().await {
        if let Ok(body) = parse_json_response(resp, "Teltonika loadavg query failed").await {
            if let Some(data) = body
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.get(1))
                .and_then(|obj| obj.get("data"))
                .and_then(|v| v.as_str())
            {
                let parts: Vec<&str> = data.split_whitespace().take(3).collect();
                if parts.len() == 3 {
                    details.push(DeviceConnectionDetail {
                        label: "CPU Load".to_string(),
                        value: format!("{} {} {}", parts[0], parts[1], parts[2]),
                    });
                }
            }
        }
    }

    Ok(DeviceConnectionStatus {
        connected: true,
        title,
        summary: format!("Connected to {host}"),
        details,
    })
}

#[cfg(feature = "ssr")]
async fn probe_openwrt(device: &DeviceConfig) -> Result<DeviceConnectionStatus, DeviceClientError> {
    let host = normalized_host(device)?;
    let base_url = base_url(device)?;
    let username = username(device)?;
    let password = password(device)?;

    let client = build_http_client(device)?;
    let mut details = Vec::new();
    let mut title = "OpenWrt".to_string();

    // Prefer ubus JSON-RPC on OpenWrt: this is stable across 24.x devices
    // and returns board/info data with consistent keys.
    let ubus_url = format!("{base_url}/ubus");
    let session_id = teltonika_login(&client, &ubus_url, &username, &password).await;
    if let Ok(session_id) = session_id {
        let board_payload = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "call",
            "params": [session_id.clone(), "system", "board", {}]
        });
        let board_response = client.post(&ubus_url).json(&board_payload).send().await?;
        let board_body = parse_json_response(board_response, "OpenWrt board query failed").await?;
        if let Some(obj) = board_body
            .get("result")
            .and_then(|r| r.as_array())
            .and_then(|arr| {
                let code = arr.first().and_then(|c| c.as_u64()).unwrap_or(1);
                if code == 0 { arr.get(1) } else { None }
            })
            .and_then(|v| v.as_object())
        {
            if let Some(model) = obj.get("model").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                details.push(DeviceConnectionDetail {
                    label: "Model".to_string(),
                    value: model.to_string(),
                });
                title = model.to_string();
            }

            if let Some(hostname) = obj.get("hostname").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                details.push(DeviceConnectionDetail {
                    label: "Hostname".to_string(),
                    value: hostname.to_string(),
                });
            }

            if let Some(system) = obj.get("system").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                details.push(DeviceConnectionDetail {
                    label: "System".to_string(),
                    value: system.to_string(),
                });
            }

            if let Some(kernel) = obj.get("kernel").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                details.push(DeviceConnectionDetail {
                    label: "Kernel".to_string(),
                    value: kernel.to_string(),
                });
            }

            if let Some(firmware) = obj
                .get("release")
                .and_then(|v| v.as_object())
                .and_then(|r| r.get("description"))
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
            {
                details.push(DeviceConnectionDetail {
                    label: "Firmware".to_string(),
                    value: firmware.to_string(),
                });
            }
        }

        let info_payload = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "call",
            "params": [session_id.clone(), "system", "info", {}]
        });
        let info_response = client.post(&ubus_url).json(&info_payload).send().await?;
        let info_body = parse_json_response(info_response, "OpenWrt info query failed").await?;
        if let Some(obj) = info_body
            .get("result")
            .and_then(|r| r.as_array())
            .and_then(|arr| {
                let code = arr.first().and_then(|c| c.as_u64()).unwrap_or(1);
                if code == 0 { arr.get(1) } else { None }
            })
            .and_then(|v| v.as_object())
        {
            if let Some(uptime) = obj.get("uptime").and_then(|v| v.as_u64()) {
                details.push(DeviceConnectionDetail {
                    label: "Uptime".to_string(),
                    value: format_uptime(uptime),
                });
            }

            if let Some(memory) = obj.get("memory").and_then(|v| v.as_object()) {
                if let Some(total) = memory.get("total").and_then(|v| v.as_u64()) {
                    let available = memory
                        .get("available")
                        .and_then(|v| v.as_u64())
                        .or_else(|| memory.get("free").and_then(|v| v.as_u64()));
                    if let Some(avail) = available {
                        details.push(DeviceConnectionDetail {
                            label: "Memory".to_string(),
                            value: format_memory_kb(total / 1024, avail / 1024),
                        });
                    }
                }
            }

            if let Some(load_arr) = obj.get("load").and_then(|v| v.as_array()) {
                let loads: Vec<String> = load_arr
                    .iter()
                    .take(3)
                    .filter_map(|v| v.as_u64())
                    .map(|v| format!("{:.2}", v as f64 / 65536.0))
                    .collect();
                if loads.len() == 3 {
                    details.push(DeviceConnectionDetail {
                        label: "CPU Load".to_string(),
                        value: loads.join(" "),
                    });
                }
            }
        }

        // On OpenWrt 24.x direct network.interface.<ifname>.status may be ACL-blocked,
        // but network.interface.dump is typically allowed.
        let net_payload = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "call",
            "params": [session_id, "network.interface", "dump", {}]
        });
        if let Ok(resp) = client.post(&ubus_url).json(&net_payload).send().await {
            if let Ok(body) = parse_json_response(resp, "OpenWrt interface dump query failed").await {
                if let Some(interfaces) = body
                    .get("result")
                    .and_then(|r| r.as_array())
                    .and_then(|arr| {
                        let code = arr.first().and_then(|c| c.as_u64()).unwrap_or(1);
                        if code == 0 { arr.get(1) } else { None }
                    })
                    .and_then(|v| v.get("interface"))
                    .and_then(|v| v.as_array())
                {
                    for (iface_name, prefix) in [("lan", "LAN"), ("wan", "WAN")] {
                        if let Some(iface) = interfaces.iter().find(|iface| {
                            iface.get("interface").and_then(|v| v.as_str()) == Some(iface_name)
                        }) {
                            push_iface_details(iface, prefix, &mut details);
                        }
                    }
                }
            }
        }

        // Some firmwares allow session login but block the queried ubus objects,
        // which would otherwise show as "authenticated" with an empty data table.
        // Only keep the ubus path if we actually collected any device details.
        if !details.is_empty() {
            return Ok(DeviceConnectionStatus {
                connected: true,
                title,
                summary: format!("Authenticated at {host} via ubus"),
                details,
            });
        }
    }

    // Turris fallback: use LuCI auth + sys.exec commands when /ubus endpoint is not available.
    let login_payload = json!({
        "id": 1,
        "method": "login",
        "params": [username, password],
    });
    let login_response = client
        .post(format!("{base_url}/cgi-bin/luci/rpc/auth"))
        .json(&login_payload)
        .send()
        .await?;
    let login_json = parse_json_response(login_response, "OpenWrt/Turris LuCI login failed").await?;
    let token = extract_openwrt_token(&login_json)
        .ok_or_else(|| DeviceClientError::DeviceLogin("missing auth token from LuCI RPC".to_string()))?;

    if let Some(board_text) = luci_exec_sys(&client, &base_url, &token, "ubus call system board").await? {
        if let Ok(board) = serde_json::from_str::<Value>(&board_text) {
            if let Some(obj) = board.as_object() {
                if let Some(model) = obj.get("model").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                    details.push(DeviceConnectionDetail {
                        label: "Model".to_string(),
                        value: model.to_string(),
                    });
                    title = model.to_string();
                }

                if let Some(hostname) = obj.get("hostname").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                    details.push(DeviceConnectionDetail {
                        label: "Hostname".to_string(),
                        value: hostname.to_string(),
                    });
                }

                if let Some(system) = obj.get("system").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                    details.push(DeviceConnectionDetail {
                        label: "System".to_string(),
                        value: system.to_string(),
                    });
                }

                if let Some(kernel) = obj.get("kernel").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                    details.push(DeviceConnectionDetail {
                        label: "Kernel".to_string(),
                        value: kernel.to_string(),
                    });
                }

                if let Some(firmware) = obj
                    .get("release")
                    .and_then(|v| v.as_object())
                    .and_then(|r| r.get("description"))
                    .and_then(|v| v.as_str())
                    .filter(|v| !v.is_empty())
                {
                    details.push(DeviceConnectionDetail {
                        label: "Firmware".to_string(),
                        value: firmware.to_string(),
                    });
                }
            }
        }
    }

    if details.iter().all(|d| d.label != "Hostname") {
        if let Some(hostname) = luci_exec_sys(&client, &base_url, &token, "cat /proc/sys/kernel/hostname").await? {
            details.push(DeviceConnectionDetail {
                label: "Hostname".to_string(),
                value: hostname,
            });
        }
    }

    if details.iter().all(|d| d.label != "Firmware") {
        if let Some(release_file) = luci_exec_sys(&client, &base_url, &token, "cat /etc/openwrt_release").await? {
            if let Some(desc_line) = release_file
                .lines()
                .find(|line| line.starts_with("DISTRIB_DESCRIPTION="))
            {
                let value = desc_line
                    .split_once('=')
                    .map(|(_, v)| v.trim_matches('\''))
                    .unwrap_or("")
                    .trim();
                if !value.is_empty() {
                    details.push(DeviceConnectionDetail {
                        label: "Firmware".to_string(),
                        value: value.to_string(),
                    });
                }
            }
        }
    }

    if let Some(uptime_str) = luci_exec_sys(&client, &base_url, &token, "cat /proc/uptime").await? {
        if let Some(secs) = uptime_str
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
        {
            details.push(DeviceConnectionDetail {
                label: "Uptime".to_string(),
                value: format_uptime(secs as u64),
            });
        }
    }

    if let Some(meminfo) = luci_exec_sys(&client, &base_url, &token, "cat /proc/meminfo").await? {
        let mut total_kb: Option<u64> = None;
        let mut avail_kb: Option<u64> = None;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
            } else if line.starts_with("MemAvailable:") {
                avail_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
            } else if line.starts_with("MemFree:") && avail_kb.is_none() {
                avail_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
            }
        }
        if let (Some(total), Some(avail)) = (total_kb, avail_kb) {
            details.push(DeviceConnectionDetail {
                label: "Memory".to_string(),
                value: format_memory_kb(total, avail),
            });
        }
    }

    if let Some(loadavg) = luci_exec_sys(&client, &base_url, &token, "cat /proc/loadavg").await? {
        let parts: Vec<&str> = loadavg.split_whitespace().take(3).collect();
        if parts.len() == 3 {
            details.push(DeviceConnectionDetail {
                label: "CPU Load".to_string(),
                value: format!("{} {} {}", parts[0], parts[1], parts[2]),
            });
        }
    }

    for (cmd, prefix) in [("ifstatus lan", "LAN"), ("ifstatus wan", "WAN")] {
        if let Some(text) = luci_exec_sys(&client, &base_url, &token, cmd).await? {
            if let Ok(iface) = serde_json::from_str::<Value>(&text) {
                push_iface_details(&iface, prefix, &mut details);
            }
        }
    }

    Ok(DeviceConnectionStatus {
        connected: true,
        title,
        summary: format!("Authenticated at {host} via LuCI RPC"),
        details,
    })
}

#[cfg(feature = "ssr")]
pub async fn reboot_device(device: &DeviceConfig) -> Result<(), DeviceClientError> {
    let platform = DevicePlatform::parse(device.device_type.as_deref());

    match platform {
        DevicePlatform::OpenWrt => reboot_openwrt(device).await,
        DevicePlatform::Teltonika => reboot_teltonika(device).await,
    }
}

#[cfg(not(feature = "ssr"))]
pub async fn reboot_device(_device: &DeviceConfig) -> Result<(), DeviceClientError> {
    Err(DeviceClientError::UnsupportedDeviceType(
        "device communication is only available on the server".to_string(),
    ))
}

#[cfg(feature = "ssr")]
async fn reboot_teltonika(device: &DeviceConfig) -> Result<(), DeviceClientError> {
    let base_url = base_url(device)?;
    let username = username(device)?;
    let password = password(device)?;
    let ubus_url = format!("{base_url}/ubus");

    let client = build_http_client(device)?;

    let session_id = teltonika_login(&client, &ubus_url, &username, &password).await?;

    // Reboot via JSON-RPC: file exec reboot config
    let reboot_payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "call",
        "params": [session_id, "file", "exec", {"command": "reboot", "params": ["config"]}]
    });

    client
        .post(&ubus_url)
        .json(&reboot_payload)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

#[cfg(feature = "ssr")]
async fn reboot_openwrt(device: &DeviceConfig) -> Result<(), DeviceClientError> {
    let base_url = base_url(device)?;
    let username = username(device)?;
    let password = password(device)?;

    let client = build_http_client(device)?;

    let login_payload = json!({
        "id": 1,
        "method": "login",
        "params": [username, password],
    });

    let login_response: Value = client
        .post(format!("{base_url}/cgi-bin/luci/rpc/auth"))
        .json(&login_payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let token = extract_openwrt_token(&login_response)
        .ok_or_else(|| DeviceClientError::DeviceLogin("missing auth token".to_string()))?;

    let reboot_payload = json!({
        "id": 1,
        "method": "reboot",
        "params": [],
    });

    client
        .post(format!("{base_url}/cgi-bin/luci/rpc/sys?auth={token}"))
        .json(&reboot_payload)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}