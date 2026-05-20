use crate::device_clients::DeviceConnectionStatus;
use crate::modals::{ConfigModal, DeviceDetailsModal};
use leptos_meta::{provide_meta_context, Link, Meta, MetaTags, Stylesheet, Title};
use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::StaticSegment;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct DeviceConfig {
    pub name: String,
    pub comment: Option<String>,
    pub device_type: Option<String>,
    pub ip_address: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct ConfigData {
    pub devices: Option<Vec<DeviceConfig>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GenericConfig {
    pub polling_interval: u64,
}

#[server]
pub async fn read_raw_toml() -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use std::path::PathBuf;

        let mut config_path = None;

        if let Ok(path) = std::env::var("WRTCTRL_CONFIG_FILE") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                config_path = Some(PathBuf::from(trimmed));
            }
        }

        if config_path.is_none() {
            for candidate in ["/etc/wrtctrl/config.toml", "./wrtctrl.toml"] {
                let path = PathBuf::from(candidate);
                if path.exists() {
                    config_path = Some(path);
                    break;
                }
            }
        }

        let config_path = match config_path {
            Some(p) => p,
            None => return Err(ServerFnError::ServerError("Config file not found".to_string())),
        };

        let contents = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => return Err(ServerFnError::ServerError(format!("Failed reading config: {e}"))),
        };

        tracing::info!(path = %config_path.display(), "Reading configuration");
        Ok(contents)
    }

    #[cfg(not(feature = "ssr"))]
    {
        Ok(String::new())
    }
}

#[server]
pub async fn write_raw_toml(raw: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use std::path::PathBuf;

        let mut config_path = None;

        if let Ok(path) = std::env::var("WRTCTRL_CONFIG_FILE") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                config_path = Some(PathBuf::from(trimmed));
            }
        }

        if config_path.is_none() {
            for candidate in ["/etc/wrtctrl/config.toml", "./wrtctrl.toml"] {
                let path = PathBuf::from(candidate);
                if path.exists() {
                    config_path = Some(path);
                    break;
                }
            }
        }

        let config_path = config_path.unwrap_or_else(|| PathBuf::from("./wrtctrl.toml"));

        if let Err(e) = raw.parse::<toml::Value>() {
            return Err(ServerFnError::ServerError(format!("Invalid TOML: {e}")));
        }

        if let Err(e) = std::fs::write(&config_path, raw) {
            return Err(ServerFnError::ServerError(format!("Failed writing config: {e}")));
        }

        Ok(())
    }

    #[cfg(not(feature = "ssr"))]
    {
        Ok(())
    }
}

#[server]
pub async fn read_toml_config() -> Result<ConfigData, ServerFnError> {
    let raw = read_raw_toml().await?;
    let parsed = match raw.parse::<toml::Value>() {
        Ok(value) => value,
        Err(e) => return Err(ServerFnError::ServerError(format!("Invalid TOML: {e}"))),
    };

    let mut devices = Vec::new();
    if let Some(device_values) = parsed.get("devices").and_then(|v| v.as_array()) {
        for device in device_values {
            if let Some(table) = device.as_table() {
                let name = table
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let comment = table
                    .get("comment")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let device_type = table
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let ip_address = table
                    .get("ip_address")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let username = table
                    .get("username")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let password = table
                    .get("password")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                devices.push(DeviceConfig {
                    name,
                    comment,
                    device_type,
                    ip_address,
                    username,
                    password,
                });
            }
        }
    }

    tracing::info!(count = devices.len(), "Configuration loaded");
    Ok(ConfigData {
        devices: Some(devices),
    })
}

#[server]
pub async fn read_generic_config() -> Result<GenericConfig, ServerFnError> {
    let raw = read_raw_toml().await?;
    let parsed = match raw.parse::<toml::Value>() {
        Ok(value) => value,
        Err(e) => return Err(ServerFnError::ServerError(format!("Invalid TOML: {e}"))),
    };

    let polling_interval = parsed
        .get("generic")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("polling_interval"))
        .and_then(|v| v.as_integer())
        .or_else(|| parsed.get("polling_interval").and_then(|v| v.as_integer()))
        .unwrap_or(30);

    Ok(GenericConfig {
        polling_interval: polling_interval.max(1) as u64,
    })
}

#[server]
pub async fn write_generic_config(config: GenericConfig) -> Result<(), ServerFnError> {
    let mut raw = read_raw_toml().await.unwrap_or_default();
    if raw.trim().is_empty() {
        raw = String::from("");
    }

    let mut toml_value = if raw.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        match raw.parse::<toml::Value>() {
            Ok(value) => value,
            Err(e) => return Err(ServerFnError::ServerError(format!("Invalid TOML: {e}"))),
        }
    };

    let table = match toml_value.as_table_mut() {
        Some(table) => table,
        None => return Err(ServerFnError::ServerError("Invalid TOML structure".to_string())),
    };

    let generic_entry = table
        .entry("generic".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    let generic_table = match generic_entry.as_table_mut() {
        Some(t) => t,
        None => return Err(ServerFnError::ServerError("Invalid [generic] section".to_string())),
    };

    generic_table.insert(
        "polling_interval".to_string(),
        toml::Value::Integer(config.polling_interval.max(1) as i64),
    );

    let new_contents = match toml::to_string_pretty(&toml_value) {
        Ok(contents) => contents,
        Err(e) => return Err(ServerFnError::ServerError(format!("Failed serializing TOML: {e}"))),
    };

    write_raw_toml(new_contents).await
}

#[server]
pub async fn read_device_status(device: DeviceConfig) -> Result<DeviceConnectionStatus, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let name = device.name.clone();
        let ip = device.ip_address.clone().unwrap_or_default();
        tracing::info!(device = %name, ip = %ip, "Polling device");
        let result = crate::device_clients::probe_device(&device).await;
        match &result {
            Ok(status) => tracing::info!(device = %name, summary = %status.summary, "Device reachable"),
            Err(error) => tracing::warn!(device = %name, error = %error, "Device unreachable"),
        }
        return result.map_err(|error| ServerFnError::ServerError(error.to_string()));
    }

    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::ServerError(
            "device communication is only available on the server".to_string(),
        ))
    }
}

#[server]
pub async fn reboot_device_action(device: DeviceConfig) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        return crate::device_clients::reboot_device(&device)
            .await
            .map_err(|error| ServerFnError::ServerError(error.to_string()));
    }

    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::ServerError(
            "device communication is only available on the server".to_string(),
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppStats {
    pub device_statuses: std::collections::HashMap<String, Option<bool>>,
}

#[derive(Clone, Debug, Default)]
pub struct DebugOutputEntry {
    pub source: String,
    pub message: String,
}

#[derive(Clone, Copy)]
pub struct DebugOutputBus {
    pub entries: RwSignal<Vec<DebugOutputEntry>>,
}

#[derive(Clone, Copy)]
pub struct AppRefreshBus {
    pub tick: RwSignal<u64>,
}

#[derive(Clone, Copy)]
pub struct ConfigRefreshBus {
    pub tick: RwSignal<u64>,
}

#[derive(Clone, Copy)]
pub struct DeviceModalBus {
    pub signal: RwSignal<Option<String>>,
}

#[derive(Clone, Copy)]
pub struct AppStatsBus {
    pub stats: RwSignal<AppStats>,
}

#[allow(dead_code)]
fn append_debug_output(bus: &DebugOutputBus, source: &str, messages: Vec<String>) {
    bus.entries.update(|entries| {
        for message in messages {
            entries.push(DebugOutputEntry {
                source: source.to_string(),
                message,
            });
        }
    });
}

#[server]
pub async fn write_toml_config(config: ConfigData) -> Result<(), ServerFnError> {
    let mut raw = read_raw_toml().await.unwrap_or_default();
    if raw.trim().is_empty() {
        raw = String::from("");
    }

    let mut toml_value = if raw.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        match raw.parse::<toml::Value>() {
            Ok(value) => value,
            Err(e) => return Err(ServerFnError::ServerError(format!("Invalid TOML: {e}"))),
        }
    };

    let table = match toml_value.as_table_mut() {
        Some(table) => table,
        None => return Err(ServerFnError::ServerError("Invalid TOML structure".to_string())),
    };

    let mut devices_array = Vec::new();
    if let Some(devices) = config.devices {
        for device in devices {
            let mut item = toml::map::Map::new();
            item.insert("name".to_string(), toml::Value::String(device.name));
            if let Some(comment) = device.comment {
                if !comment.trim().is_empty() {
                    item.insert("comment".to_string(), toml::Value::String(comment));
                }
            }
            if let Some(device_type) = device.device_type {
                if !device_type.trim().is_empty() {
                    item.insert("type".to_string(), toml::Value::String(device_type));
                }
            }
            if let Some(ip_address) = device.ip_address {
                if !ip_address.trim().is_empty() {
                    item.insert("ip_address".to_string(), toml::Value::String(ip_address));
                }
            }
            if let Some(username) = device.username {
                if !username.trim().is_empty() {
                    item.insert("username".to_string(), toml::Value::String(username));
                }
            }
            if let Some(password) = device.password {
                if !password.trim().is_empty() {
                    item.insert("password".to_string(), toml::Value::String(password));
                }
            }
            devices_array.push(toml::Value::Table(item));
        }
    }
    table.insert("devices".to_string(), toml::Value::Array(devices_array));

    let new_contents = match toml::to_string_pretty(&toml_value) {
        Ok(contents) => contents,
        Err(e) => return Err(ServerFnError::ServerError(format!("Failed serializing TOML: {e}"))),
    };

    write_raw_toml(new_contents).await
}


#[cfg(feature = "ssr")]
fn read_instance_name_sync() -> String {
    "WRTCTRL".to_string()
}

#[server]
pub async fn read_instance_name() -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        return Ok(read_instance_name_sync());
    }

    #[cfg(not(feature = "ssr"))]
    {
        Ok("wrtctrl".to_string())
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ServerLogEntry {
    pub index: u64,
    pub level: String,
    pub message: String,
}

#[server]
pub async fn read_server_logs(since_index: u64) -> Result<Vec<ServerLogEntry>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let entries = crate::log_buffer::read_since(since_index);
        return Ok(entries
            .into_iter()
            .map(|e| ServerLogEntry {
                index: e.index,
                level: e.level,
                message: e.message,
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    {
        Ok(vec![])
    }
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    let options_for_reload = options.clone();

    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                {move || {
                    if cfg!(debug_assertions) {
                        Some(view! { <AutoReload options=options_for_reload.clone() /> })
                    } else {
                        None
                    }
                }}
                <HydrationScripts options=options.clone() />
                <MetaTags />
                <Stylesheet id="leptos" href="/pkg/wrtctrl.css" />
            </head>
            <body class="bg-gray-900 text-gray-100 min-h-screen">
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    let debug_entries = RwSignal::new(vec![DebugOutputEntry {
        source: "system".to_string(),
        message: "log initialized".to_string(),
    }]);
    provide_context(DebugOutputBus {
        entries: debug_entries,
    });
    let app_refresh_tick = RwSignal::new(0_u64);
    provide_context(AppRefreshBus { tick: app_refresh_tick });
    let config_refresh_tick = RwSignal::new(0_u64);
    provide_context(ConfigRefreshBus { tick: config_refresh_tick });
    let device_modal_signal = RwSignal::new(None::<String>);
    provide_context(DeviceModalBus { signal: device_modal_signal });

    #[cfg(feature = "hydrate")]
    {
        use gloo_timers::callback::Interval;
        use std::mem;

        let polling_interval_seconds = RwSignal::new(20_u64);

        let polling_interval_resource = LocalResource::new(
            || async move {
                read_generic_config()
                    .await
                    .map(|cfg| cfg.polling_interval.max(1))
                    .unwrap_or(20)
            },
        );

        Effect::new(move |_| {
            if let Some(interval) = polling_interval_resource.get() {
                polling_interval_seconds.set(interval.max(1));
            }
        });

        let elapsed_seconds = RwSignal::new(0_u64);

        let refresh_interval = Interval::new(1_000, move || {
            let interval = polling_interval_seconds.get().max(1);
            let next_elapsed = elapsed_seconds.get().saturating_add(1);

            if next_elapsed >= interval {
                elapsed_seconds.set(0);
                app_refresh_tick.update(|tick| *tick += 1);
            } else {
                elapsed_seconds.set(next_elapsed);
            }
        });
        mem::forget(refresh_interval);
    }

    let page_brand = LocalResource::new(
        || async move { read_instance_name().await.unwrap_or_else(|_| "wrtctrl".to_string()) },
    );

    view! {
        <Title text=move || format!("{} Welcome", page_brand.get().unwrap_or_else(|| "wrtctrl".to_string())) />
        <Meta name="description" content="wrtctrl Welcome" />
        <Link rel="shortcut icon" type_="image/ico" href="/favicon.ico" />

        <Router>
            <div class="flex flex-col min-h-screen">
                <Navbar />
                <main class="flex-1">
                    <Routes fallback=|| view! { <NotFound /> }>
                        <Route path=StaticSegment("") view=HomePage />
                    </Routes>
                </main>
                <Footer />
            </div>
        </Router>
    }
}

#[component]
fn Navbar() -> impl IntoView {
    view! {
        <nav class="sticky top-0 z-50 bg-gray-900 border-b border-gray-800 shadow-sm">
            <div class="mx-auto max-w-6xl px-4 sm:px-6 lg:px-8">
                <div class="flex h-14 items-center justify-between">
                    // Brand
                    <a href="/" class="flex items-center gap-2 font-bold text-gray-100 tracking-tight text-lg hover:text-white transition">
                            <img
                                src="/logo.svg"
                            alt="affolter engineering logo"
                            class="h-7 w-auto"
                        />
                        <InstanceName />
                    </a>
                    // Connection status
                    <div class="flex items-center">
                        <StatusBadge />
                    </div>
                </div>
            </div>
        </nav>
    }
}


#[component]
fn Footer() -> impl IntoView {
    let show_dialog = RwSignal::new(false);
    let show_config = RwSignal::new(false);
    let device_modal_bus = use_context::<DeviceModalBus>();

    view! {
        <footer class="border-t border-gray-800 bg-gray-900 py-4 text-center text-sm text-gray-100">
            <div class="mx-auto flex max-w-6xl items-center justify-center gap-2 px-4">
                <span>"wrtctrl · © Schiek IT Consulting & affolter engineering · v" {env!("CARGO_PKG_VERSION")}</span>
                <button
                    class="rounded px-2 py-1 text-base leading-none text-gray-100 transition hover:bg-gray-800 hover:text-white"
                    on:click=move |_| {
                        if let Some(bus) = device_modal_bus {
                            bus.signal.set(Some(String::new()));
                        }
                    }
                    type="button"
                    aria-label="Add Device"
                    title="Add Device"
                >
                    "+"
                </button>
                <button
                    class="rounded px-2 py-1 text-base leading-none text-gray-100 transition hover:bg-gray-800 hover:text-white"
                    on:click=move |_| show_config.set(true)
                    type="button"
                    aria-label="Open Configuration"
                    title="Configuration"
                >
                    "⚙️"
                </button>
                <button
                    class="rounded px-2 py-1 text-base leading-none text-gray-100 transition hover:bg-gray-800 hover:text-white"
                    on:click=move |_| show_dialog.set(true)
                    type="button"
                    aria-label="Open About"
                    title="About"
                >
                    "🦉"
                </button>
            </div>

            {move || {
                show_config
                    .get()
                    .then(|| {
                        view! {
                            <ConfigModal show_config=show_config />
                        }
                    })
            }}

            {move || {
                show_dialog
                    .get()
                    .then(|| {
                        view! {
                            <div class="fixed inset-0 z-50 flex items-center justify-center bg-gray-900/80 px-4">
                                <div class="w-full max-w-md rounded-lg border border-gray-700 bg-gray-800 p-6 text-left shadow-2xl">
                                    <div class="flex items-start justify-between gap-4">
                                        <div>
                                            <h2 class="text-lg font-semibold text-gray-100">"About"</h2>
                                            <p class="mt-1 text-sm text-gray-300">
                                                "wrtctrl is a project by Schiek IT Consulting and affolter engineering."
                                            </p>
                                        </div>
                                    </div>

                                    <ul class="mt-4 list-disc space-y-2 pl-5 text-sm text-gray-200">
                                        <li>
                                            <span class="font-medium text-gray-100">"Repository: "</span>
                                            <a
                                                class="text-gray-200 transition hover:text-white hover:underline"
                                                href="https://github.com/affolter-engineering/wrtctrl"
                                                target="_blank"
                                                rel="noreferrer"
                                            >
                                                "github.com/affolter-engineering/wrtctrl"
                                            </a>
                                        </li>
                                        <li>
                                            <span class="font-medium text-gray-100">"Issues: "</span>
                                            <a
                                                class="text-gray-200 transition hover:text-white hover:underline"
                                                href="https://github.com/affolter-engineering/wrtctrl/issues"
                                                target="_blank"
                                                rel="noreferrer"
                                            >
                                                "github.com/affolter-engineering/wrtctrl/issues"
                                            </a>
                                        </li>
                                        <li>
                                            <span class="font-medium text-gray-100">"Authors: "</span>
                                            "Peter Schiek and Fabian Affolter"
                                        </li>
                                    </ul>

                                    <div class="mt-6 flex justify-end">
                                        <button
                                            class="rounded-md bg-indigo-700 px-3 py-2 text-sm font-medium text-gray-100 transition hover:bg-indigo-600"
                                            on:click=move |_| show_dialog.set(false)
                                            type="button"
                                        >
                                            "Close"
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }
                    })
            }}

        </footer>
    }
}


#[component]
fn NotificationArea() -> impl IntoView {
    let app_stats_bus = use_context::<AppStatsBus>();

    view! {
        {move || {
            let Some(app_stats_bus) = app_stats_bus else {
                return ().into_any();
            };

            let stats = app_stats_bus.stats.get();
            let statuses = &stats.device_statuses;

            let any_polled = statuses.values().any(|s| s.is_some());
            let any_reachable = statuses.values().any(|s| *s == Some(true));

            if statuses.is_empty() || !any_polled || any_reachable {
                return ().into_any();
            }

            view! {
                <div class="app-notification app-notification-danger" role="alert" aria-live="polite">
                    <div class="app-notification-title">"No device can be reached"</div>
                </div>
            }
                .into_any()
        }}
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let app_stats = RwSignal::new(AppStats::default());
    provide_context(AppStatsBus { stats: app_stats });

    view! {
        <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-16 flex flex-col gap-6">
            // <header class="text-center">
            //     <h1 class="text-5xl font-extrabold tracking-tight text-black">
            //         "Welcome to " <InstanceName />
            //     </h1>
            //     <p class="mt-4 text-lg text-black">
            //         ""
            //     </p>
            // </header>

            <NotificationArea />
        
            <DevicesCard />

            <div class="flex flex-col gap-4">
                <LogOutputCard />
            </div>
        </div>
    }
}

#[component]
fn DevicesCard() -> impl IntoView {
    let config_refresh_bus = use_context::<ConfigRefreshBus>();
    let config_refresh_tick = config_refresh_bus
        .map(|bus| bus.tick)
        .unwrap_or_else(|| RwSignal::new(0_u64));

    let config_data = Resource::new_blocking(
        move || config_refresh_tick.get(),
        |_| async move { read_toml_config().await },
    );
    let device_modal_bus = use_context::<DeviceModalBus>();
    let show_details = device_modal_bus
        .map(|bus| bus.signal)
        .unwrap_or_else(|| RwSignal::new(None::<String>));

    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
            <Transition
                fallback=move || {
                    view! {
                        <div class="rounded border border-gray-700 bg-gray-800 p-4 text-sm text-gray-200">
                            <div class="h-4 w-32 animate-pulse rounded bg-gray-700/60"></div>
                        </div>
                    }
                }
            >
                {move || {
                    match config_data.get() {
                        Some(Err(e)) => view! {
                            <div class="rounded border border-red-700 bg-red-950/40 p-4 text-sm text-red-200">
                                {format!("Failed to load devices: {e}")}
                            </div>
                        }
                            .into_any(),
                        Some(Ok(config)) => {
                            let devices = config.devices.unwrap_or_default();
                            if devices.is_empty() {
                                return view! {
                                    <div class="rounded border border-gray-700 bg-gray-800 p-4 text-sm text-gray-200">
                                        "No devices configured."
                                    </div>
                                }
                                    .into_any();
                            }

                            view! {
                                {devices
                                    .into_iter()
                                    .map(|device| {
                                        view! {
                                            <DeviceCard device=device show_details=show_details />
                                        }
                                    })
                                    .collect_view()}
                            }
                                .into_any()
                        }
                        None => view! { <></> }.into_any(),
                    }
                }}
            </Transition>
        </div>

        {move || match show_details.get() {
            None => None,
            Some(ref name) if name.is_empty() => Some(view! {
                <DeviceDetailsModal device_name=None show_details=show_details />
            }),
            Some(device_name) => Some(view! {
                <DeviceDetailsModal device_name=Some(device_name) show_details=show_details />
            }),
        }}
    }
}

#[component]
fn DeviceRebootConfirmation(
    device: DeviceConfig,
    show: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    reboot_action: Action<DeviceConfig, Result<(), ServerFnError>>,
) -> impl IntoView {
    let device_name_display = device.name.clone();
    let device_for_dispatch = device.clone();

    view! {
        {move || {
            if show.get() {
                let is_pending = reboot_action.pending().get();
                let device_clone = device_for_dispatch.clone();
                view! {
                    <div class="mt-4 rounded border border-orange-700 bg-orange-950/20 p-3 text-xs">
                        <p class="mb-3 text-gray-100 font-medium">
                            "Are you sure you want to reboot " {device_name_display.clone()} "?"
                        </p>
                        {error.get().map(|err| {
                            view! {
                                <p class="mb-3 text-red-300">{format!("Error: {err}")}</p>
                            }
                        })}
                        <div class="flex gap-2">
                            <button
                                class="rounded bg-orange-700 px-2 py-1 text-[11px] text-gray-100 transition hover:bg-orange-600 disabled:opacity-50"
                                type="button"
                                on:click=move |_| {
                                    error.set(None);
                                    reboot_action.dispatch(device_clone.clone());
                                }
                                disabled=is_pending
                            >
                                {if is_pending { "Rebooting..." } else { "Confirm Reboot" }}
                            </button>
                            <button
                                class="rounded bg-gray-700 px-2 py-1 text-[11px] text-gray-100 transition hover:bg-gray-600 disabled:opacity-50"
                                type="button"
                                on:click=move |_| {
                                    show.set(false);
                                    error.set(None);
                                }
                                disabled=is_pending
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                }
                    .into_any()
            } else {
                view! { <></> }.into_any()
            }
        }}
    }
}

#[component]
fn DeviceCard(device: DeviceConfig, show_details: RwSignal<Option<String>>) -> impl IntoView {
    let app_refresh_bus = use_context::<AppRefreshBus>();
    let global_refresh_tick = app_refresh_bus
        .map(|bus| bus.tick)
        .unwrap_or_else(|| RwSignal::new(0_u64));
    let refresh_tick = RwSignal::new(0_u64);
    let show_reboot_confirm = RwSignal::new(false);
    let reboot_error = RwSignal::new(None);
    let device_for_resource = device.clone();
    let device_status = Resource::new(move || (refresh_tick.get(), global_refresh_tick.get()), move |_| {
        let device = device_for_resource.clone();
        async move { read_device_status(device).await }
    });

    let reboot_action = create_action(move |device: &DeviceConfig| {
        let device = device.clone();
        async move {
            reboot_device_action(device).await
        }
    });

    create_effect(move |_| {
        if let Some(result) = reboot_action.value().get() {
            match result {
                Ok(_) => {
                    show_reboot_confirm.set(false);
                    leptos::logging::log!("Device reboot initiated");
                }
                Err(e) => {
                    reboot_error.set(Some(e.to_string()));
                }
            }
        }
    });

    let device_for_edit = device.clone();
    let device_for_reboot = device.clone();

    let app_stats_bus = use_context::<AppStatsBus>();
    let device_name_for_stats = device.name.clone();
    let connected: RwSignal<Option<bool>> = RwSignal::new(None);
    let card_footer: RwSignal<Option<(String, String)>> = RwSignal::new(None);
    Effect::new(move |_| {
        if let Some(result) = device_status.get() {
            match result {
                Ok(ref status) => {
                    connected.set(Some(status.connected));
                    card_footer.set(Some((status.title.clone(), status.summary.clone())));
                    if let Some(bus) = app_stats_bus {
                        bus.stats.update(|stats| {
                            stats.device_statuses.insert(device_name_for_stats.clone(), Some(status.connected));
                        });
                    }
                }
                Err(_) => {
                    connected.set(Some(false));
                    card_footer.set(None);
                    if let Some(bus) = app_stats_bus {
                        bus.stats.update(|stats| {
                            stats.device_statuses.insert(device_name_for_stats.clone(), Some(false));
                        });
                    }
                }
            }
        }
    });
    let card_class = move || match connected.get() {
        Some(true) => "rounded border border-green-700 bg-green-950/30 p-4 shadow-sm",
        Some(false) => "rounded border border-red-700 bg-red-950/30 p-4 shadow-sm",
        None => "rounded border border-gray-700 bg-gray-800 p-4 shadow-sm",
    };

    view! {
        <div class=card_class>
            <div class="flex items-start justify-between gap-3">
                <div>
                    <h3 class="text-base font-semibold text-gray-100">{device.name.clone()}</h3>
                    <p class="mt-1 text-xs text-gray-300">
                        {device.comment.clone().unwrap_or_else(|| "No comment".to_string())}
                    </p>
                </div>
                <div class="inline-flex rounded overflow-hidden border border-gray-600">
                    <button
                        class="px-2 py-1 text-[11px] text-gray-100 bg-gray-700 transition hover:bg-gray-600"
                        type="button"
                        on:click=move |_| show_details.set(Some(device_for_edit.name.clone()))
                        aria-label="Edit device"
                        title="Edit device"
                    >
                        "Edit"
                    </button>
                    <button
                        class="px-2 py-1 text-[11px] text-gray-100 bg-orange-700 border-l border-gray-600 transition hover:bg-orange-600"
                        type="button"
                        on:click=move |_| {
                            show_reboot_confirm.set(true);
                            reboot_error.set(None);
                        }
                        aria-label="Reboot device"
                        title="Reboot device"
                    >
                        "Reboot"
                    </button>
                </div>
            </div>

            <div class="mt-4 rounded border border-gray-700 bg-gray-900/60 p-3 text-xs text-gray-200">
                <Transition fallback=move || view! { <p>"Checking connection..."</p> }>
                    {move || {
                        match device_status.get() {
                            Some(Err(error)) => view! {
                                <p class="text-red-300">{format!("Connection failed: {error}")}</p>
                            }
                                .into_any(),
                            Some(Ok(status)) => view! {
                                <div class="space-y-2">
                                    {(!status.details.is_empty()).then(|| view! {
                                        <table class="w-full border-collapse text-[11px]">
                                            <tbody>
                                                {status
                                                    .details
                                                    .into_iter()
                                                    .map(|detail| {
                                                        view! {
                                                            <tr class="border-t border-gray-700/50 first:border-t-0">
                                                                <td class="py-1 pr-3 font-medium text-gray-400 whitespace-nowrap w-1/3">
                                                                    {detail.label}
                                                                </td>
                                                                <td class="py-1 text-gray-100 break-all">
                                                                    {detail.value}
                                                                </td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </tbody>
                                        </table>
                                    })}
                                </div>
                            }
                                .into_any(),
                            None => view! { <></> }.into_any(),
                        }
                    }}
                </Transition>
            </div>

            {move || card_footer.get().map(|(title, summary)| view! {
                <div class="mt-3 border-t border-gray-700/50 pt-2 flex items-center justify-between gap-2 text-[11px]">
                    <span class="text-gray-400">{summary}</span>
                </div>
            })}

            <DeviceRebootConfirmation device=device_for_reboot show=show_reboot_confirm error=reboot_error reboot_action=reboot_action />
        </div>
    }
}

#[component]
fn LogOutputCard() -> impl IntoView {
    let debug_bus = use_context::<DebugOutputBus>();
    let app_refresh_bus = use_context::<AppRefreshBus>();
    let refresh_tick = app_refresh_bus
        .map(|b| b.tick)
        .unwrap_or_else(|| RwSignal::new(0_u64));
    let last_log_index = RwSignal::new(0_u64);

    let server_logs = LocalResource::new(move || {
        refresh_tick.get();
        let since = last_log_index.get_untracked();
        async move { read_server_logs(since).await }
    });

    Effect::new(move |_| {
        let Some(debug_bus) = debug_bus else { return; };
        if let Some(Ok(entries)) = server_logs.get() {
            if entries.is_empty() {
                return;
            }
            let new_max = entries.iter().map(|e| e.index).max().unwrap_or(0) + 1;
            last_log_index.set(new_max);
            let messages: Vec<String> = entries
                .into_iter()
                .map(|e| format!("[{}] {}", e.level.to_lowercase(), e.message))
                .collect();
            append_debug_output(&debug_bus, "config", messages);
        }
    });

    view! {
        <div class="rounded border border-gray-700 bg-gray-800 p-6 shadow-sm flex flex-col gap-3">
            <h2 class="text-bg font-semibold text-gray-100">"Log Output"</h2>
            {move || {
                let Some(debug_bus) = debug_bus else {
                    return view! {
                        <p class="text-xs text-gray-500">"Debug output is unavailable."</p>
                    }
                        .into_any();
                };

                let entries = debug_bus.entries.get();
                if entries.is_empty() {
                    return view! {
                        <p class="text-xs text-gray-500">"No log output yet."</p>
                    }
                        .into_any();
                }

                view! {
                    <div class="max-h-48 overflow-auto rounded-lg border border-gray-700 bg-gray-900/60 p-3">
                        <ul class="space-y-1 text-[10px] text-gray-200 font-mono">
                            {entries
                                .into_iter()
                                .rev()
                                .map(|entry| {
                                    view! {
                                        <li>
                                            <span class="font-semibold text-gray-400">"[" {entry.source} "] "</span>
                                            <span class="break-words whitespace-pre-wrap">{entry.message}</span>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn InstanceName() -> impl IntoView {
    let brand_name = LocalResource::new(
        || async move { read_instance_name().await.unwrap_or_else(|_| "wrtctrl".to_string()) },
    );

    view! {
        {move || brand_name.get().unwrap_or_else(|| "wrtctrl".to_string())}
    }
}


#[component]
fn StatusBadge() -> impl IntoView {
    let app_refresh_bus = use_context::<AppRefreshBus>();
    let refresh_tick = app_refresh_bus
        .map(|bus| bus.tick)
        .unwrap_or_else(|| RwSignal::new(0_u64));
    let elapsed_since_update = RwSignal::new(0_u64);

    let health_status = LocalResource::new(move || {
        refresh_tick.get();
        async move {
            // consume_headscale_endpoint("GET".to_string(), "health".to_string(), None)
            //     .await
            //     .map(|_| true)
            //     .unwrap_or(false)
            Ok::<Option<bool>, ServerFnError>(Some(true)) // Placeholder: always healthy
        }
    });

    Effect::new(move |_| {
        if health_status.get().is_some() {
            elapsed_since_update.set(0);
        }
    });

    #[cfg(feature = "hydrate")]
    {
        use gloo_timers::callback::Interval;
        use std::mem;

        let elapsed_since_update = elapsed_since_update;
        let elapsed_interval = Interval::new(1_000, move || {
            elapsed_since_update.update(|seconds| *seconds = seconds.saturating_add(1));
        });
        mem::forget(elapsed_interval);
    }

    view! {
        <div class="inline-flex items-center gap-2">
            <span class="text-[8px] text-white/70">
                {move || format!("Last update was {} s ago", elapsed_since_update.get())}
            </span>
        </div>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        let resp = expect_context::<leptos_axum::ResponseOptions>();
        resp.set_status(http::StatusCode::NOT_FOUND);
    }

    view! {
        <div class="flex flex-col items-center justify-center flex-1 gap-4 py-24">
            <h1 class="text-4xl font-bold text-black">"404 - Not Found"</h1>
            <StatusBadge />
        </div>
    }
}
