#![recursion_limit = "256"]

#[cfg(feature = "ssr")]
include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};

    use axum::{
        extract::State,
        http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
        response::IntoResponse,
        Router,
    };
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use wrtctrl::app::{shell, App};
    use tracing::{error, info, warn};

    use tracing_subscriber::prelude::*;

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("wrtctrl=info"));
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_target(true).with_filter(env_filter))
        .with(wrtctrl::log_buffer::LogBufferLayer)
        .try_init();

    #[derive(Clone, Debug)]
    struct ListenerConfig {
        port: u16,
        source: String,
    }

    fn read_listener_config_sync() -> Result<Option<ListenerConfig>, String> {
        let mut candidates = Vec::new();

        if let Ok(path) = std::env::var("WRTCTRL_CONFIG_FILE") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                candidates.push(std::path::PathBuf::from(trimmed));
            }
        }

        candidates.push(std::path::PathBuf::from("/etc/wrtctrl/config.toml"));
        candidates.push(std::path::PathBuf::from("./wrtctrl.toml"));

        for candidate in candidates {
            if !candidate.exists() {
                continue;
            }

            let source = candidate.display().to_string();
            let contents =
                std::fs::read_to_string(&candidate).map_err(|e| format!("Failed reading {source}: {e}"))?;

            let parsed: toml::Value = contents
                .parse()
                .map_err(|e| format!("Invalid TOML in {source}: {e}"))?;

            let server_table = parsed.get("server").and_then(|value| value.as_table());

            let read_port = |keys: &[&str]| -> Result<Option<u16>, String> {
                for key in keys {
                    if let Some(value) = parsed.get(*key) {
                        if let Some(port) = value.as_integer() {
                            if (1..=u16::MAX as i64).contains(&port) {
                                return Ok(Some(port as u16));
                            }

                            return Err(format!("Invalid '{key}' in {source}: must be between 1 and 65535"));
                        }

                        if let Some(port) = value.as_str() {
                            let parsed = port
                                .trim()
                                .parse::<u16>()
                                .map_err(|_| format!("Invalid '{key}' in {source}: must be an integer"))?;
                            return Ok(Some(parsed));
                        }

                        return Err(format!("Invalid '{key}' in {source}: must be an integer"));
                    }

                    if let Some(value) = server_table.and_then(|table| table.get(*key)) {
                        if let Some(port) = value.as_integer() {
                            if (1..=u16::MAX as i64).contains(&port) {
                                return Ok(Some(port as u16));
                            }

                            return Err(format!("Invalid 'server.{key}' in {source}: must be between 1 and 65535"));
                        }

                        if let Some(port) = value.as_str() {
                            let parsed = port
                                .trim()
                                .parse::<u16>()
                                .map_err(|_| format!("Invalid 'server.{key}' in {source}: must be an integer"))?;
                            return Ok(Some(parsed));
                        }

                        return Err(format!("Invalid 'server.{key}' in {source}: must be an integer"));
                    }
                }

                Ok(None)
            };

            if let Some(port) = read_port(&["port", "listen_port"]) ? {
                return Ok(Some(ListenerConfig { port, source }));
            }

            return Ok(None);
        }

        Ok(None)
    }

    fn sanitize_stylesheet(css: String) -> String {
        css.strip_prefix("@import \"tailwindcss\";")
            .unwrap_or(css.as_str())
            .strip_prefix("@source \"../src/**/*.rs\";")
            .unwrap_or_else(|| css.strip_prefix("@import \"tailwindcss\";").unwrap_or(css.as_str()))
            .to_string()
    }

    async fn stylesheet_handler(State(leptos_options): State<LeptosOptions>) -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "text/css; charset=utf-8".parse().unwrap());

        if !EMBEDDED_CSS.is_empty() {
            return (headers, EMBEDDED_CSS).into_response();
        }

        let site_root = PathBuf::from(leptos_options.site_root.as_ref());
        let stylesheet_path = site_root
            .join(leptos_options.site_pkg_dir.as_ref())
            .join(format!("{}.css", leptos_options.output_name.as_ref()));

        match std::fs::read_to_string(stylesheet_path) {
            Ok(css) => (headers, sanitize_stylesheet(css)).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        }
    }

    async fn js_handler(State(leptos_options): State<LeptosOptions>) -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "text/javascript; charset=utf-8".parse().unwrap());

        if !EMBEDDED_JS.is_empty() {
            return (headers, EMBEDDED_JS).into_response();
        }

        let site_root = PathBuf::from(leptos_options.site_root.as_ref());
        let js_path = site_root
            .join(leptos_options.site_pkg_dir.as_ref())
            .join(format!("{}.js", leptos_options.output_name.as_ref()));

        match std::fs::read(js_path) {
            Ok(bytes) => (headers, bytes).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        }
    }

    async fn wasm_bg_handler(State(leptos_options): State<LeptosOptions>) -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/wasm".parse().unwrap());

        if !EMBEDDED_WASM.is_empty() {
            return (headers, EMBEDDED_WASM).into_response();
        }

        let site_root = PathBuf::from(leptos_options.site_root.as_ref());
        let pkg_dir = site_root.join(leptos_options.site_pkg_dir.as_ref());

        let preferred = pkg_dir.join(format!("{}_bg.wasm", leptos_options.output_name.as_ref()));
        let fallback = pkg_dir.join(format!("{}.wasm", leptos_options.output_name.as_ref()));

        let wasm_path = if preferred.exists() { preferred } else { fallback };

        match std::fs::read(wasm_path) {
            Ok(bytes) => (headers, bytes).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        }
    }

    let load_configuration = || {
        let current_dir_manifest = std::env::current_dir().ok().map(|dir| dir.join("Cargo.toml"));
        if let Some(path) = current_dir_manifest.as_ref().filter(|path| path.exists()) {
            if let Some(path_str) = path.to_str() {
                if let Ok(conf) = get_configuration(Some(path_str)) {
                    return Ok(conf);
                }
            }
        }

        let exe_dir_manifest = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("Cargo.toml")));
        if let Some(path) = exe_dir_manifest.as_ref().filter(|path| path.exists()) {
            if let Some(path_str) = path.to_str() {
                if let Ok(conf) = get_configuration(Some(path_str)) {
                    return Ok(conf);
                }
            }
        }

        let embedded_manifest = include_str!("../Cargo.toml");
        let embedded_manifest_path = std::env::temp_dir().join(format!(
            "wrtctrl-{}-Cargo.toml",
            std::process::id()
        ));
        let _ = std::fs::write(&embedded_manifest_path, embedded_manifest);

        if let Some(path_str) = embedded_manifest_path.to_str() {
            return get_configuration(Some(path_str));
        }

        get_configuration(None)
    };

    fn resolve_site_root(raw_site_root: &Path) -> PathBuf {
        if raw_site_root.is_absolute() {
            return raw_site_root.to_path_buf();
        }

        let current_dir = std::env::current_dir().ok();
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));

        let mut candidates = Vec::new();
        if let Some(dir) = exe_dir.as_ref() {
            candidates.push(dir.join(raw_site_root));
            candidates.push(dir.join("site"));
            candidates.push(dir.join("target/site"));

            // Walk upward from the executable directory and try common Leptos output paths.
            for ancestor in dir.ancestors() {
                candidates.push(ancestor.join(raw_site_root));
                candidates.push(ancestor.join("site"));
                candidates.push(ancestor.join("target/site"));
            }
        }
        if let Some(dir) = current_dir.as_ref() {
            candidates.push(dir.join(raw_site_root));
            candidates.push(dir.join("site"));
            candidates.push(dir.join("target/site"));
        }

        candidates
            .into_iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| raw_site_root.to_path_buf())
    }

    let conf = load_configuration().expect("failed to load leptos configuration");
    let mut leptos_options = conf.leptos_options;
    let site_root = resolve_site_root(Path::new(leptos_options.site_root.as_ref()));
    let mut addr: SocketAddr = leptos_options.site_addr;
    let is_dev_env = std::env::var("LEPTOS_ENV")
        .ok()
        .map(|value| value.eq_ignore_ascii_case("dev"))
        .unwrap_or(false);
    if is_dev_env {
        info!(%addr, "DEV mode detected; using leptos site-addr for server functions");
    } else {
        match read_listener_config_sync() {
            Ok(Some(listener_config)) => {
                addr.set_port(listener_config.port);
                info!(
                    port = listener_config.port,
                    source = %listener_config.source,
                    "Using configured listen port"
                );
            }
            Ok(None) => {}
            Err(reason) => {
                warn!(error = %reason, "Ignoring invalid listen port configuration");
            }
        }
    }
    leptos_options.site_root = site_root.to_string_lossy().into_owned().into();
    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/pkg/wrtctrl.css", axum::routing::get(stylesheet_handler))
        .route("/pkg/wrtctrl.js", axum::routing::get(js_handler))
        .route("/pkg/wrtctrl_bg.wasm", axum::routing::get(wasm_bg_handler))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| {
        error!(%addr, error = %e, "Failed to bind listener");
        error!(port = addr.port(), "Another process may already be using this port");
        std::process::exit(1);
    });
    info!(%addr, "Listening");
    axum::serve(listener, app).await.unwrap_or_else(|e| {
        error!(error = %e, "Server exited unexpectedly");
        std::process::exit(1);
    });
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // Client-side hydration is handled in lib.rs via wasm_bindgen.
}
