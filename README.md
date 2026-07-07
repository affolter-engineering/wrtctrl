# wrtctrl

Web application for monitoring OpenWrt-based devices and Teltonika routers through their HTTP JSON-RPC interfaces.

The app reads a TOML configuration file, polls devices from the server side, and shows their current connection and system details in the UI. It supports the LuCI auth and exec flow used by OpenWrt and the ubus session flow used by Teltonika devices.

References:

- https://wiki.teltonika-networks.com/view/RUT240_Monitoring_via_JSON-RPC_linux
- https://documenter.getpostman.com/view/5972215/TVzNHeej#154b5d84-4065-4887-a5a8-c1f3b51fe9a6
- https://techlibrary.doodlelabs.com/json-rpc-api-guide


```bash
opkg update
opkg install luci-mod-rpc luci-lib-ipkg luci-compat
/etc/init.d/uhttpd restart
```

JSON-RPC API must be enabled on the device. Also, it need the be accessible. If you want to use
it on the WAN side then add a firewall rule that allow access to 80/tcp and 443/tcp.

Test the connection to the device:

`curl`:

```bash
$ curl https://10.0.0.3/cgi-bin/luci/rpc/auth --data '
{
  "id": 1,
  "method": "login",
  "params": [
    "root",
    "password"
  ]
}'
{"id":1,"result":"2911aa45e1f9dd2c7f7b8ff2cf9a70b6","error":null}
```

`httpie`:

```bash
$ $  echo '
{
  "id": 1,
  "method": "login",
  "params": [
    "root",
    "password"
  ]
}' | http --json POST 10.0.0.3/cgi-bin/luci/rpc/auth

{
    "error": null,
    "id": 1,
    "result": "babab27ac62d284fe72484a0d6daf3ff"
}
```

## Features

- **Device status monitoring** - Poll OpenWrt and Teltonika devices from the server
- **Config editor** - Edit the TOML configuration from the UI
- **Device details** - Show model, hostname, uptime, interfaces, memory and other status data when available
- **Server-side tracing** - Structured logging with debug output to the frontend
- **Responsive UI** - Built with Leptos and Tailwind CSS v4

## Tech stack

- Rust 2024 edition
- Leptos 0.8 (SSR + hydrate)
- Axum 0.8
- Tailwind CSS v4
- `cargo-leptos` and `cargo-make`

## Prerequisites

Recommended: use the provided Nix shell to get all build tools and dependencies:

```bash
$ nix-shell
```

The shell provides: rustc, cargo, cargo-make, cargo-leptos, tailwindcss, lld, wasm-bindgen-cli and binaryen (wasm-opt)

## Quick start

1. Enter the dev shell:

```bash
$ nix-shell
```

2. Create a configuration file. Example `wrtctrl.toml` (use the *.sample file to get started):

```toml
[generic]
polling_interval = 30

[[devices]]
name = "RUT241"
type = "teltonika"
ip_address = "10.0.0.1"
username = "admin"
password = "password"

[[devices]]
name = "OpenWRT 25.12"
type = "openwrt"
ip_address = "10.0.0.3"
username = "root"
password = "pasword"
```

3. Start the hot-reload development server:

```bash
$ cargo make dev
```

4. Open: http://127.0.0.1:3011

## Build and run

Debug build:

```bash
$ cargo make build-dev
```

Release/musl build (static binary):

```bash
$ cargo make build-musl
```

Run formatting and checks:

```bash
$ cargo make check
```

Full CI pipeline (check → test → build):

```bash
$ cargo make ci
```

## Configuration file

wrtctrl uses TOML configuration. Lookup order:

- `WRTCTRL_CONFIG_FILE` environment variable (if set)
- `/etc/wrtctrl/config.toml`
- `./wrtctrl.toml` (current working directory)

### Key settings

- `[generic].polling_interval` - Polling interval in seconds, minimum 1, default 30
- `polling_interval` - Legacy top-level fallback for the same setting
- `[[devices]]` - Device entries to poll and edit
- `devices[].name` - Display name shown in the UI
- `devices[].type` - Device platform, currently `openwrt` or `teltonika`
- `devices[].ip_address` - Device host or URL
- `devices[].username` - Login user for the JSON-RPC endpoint
- `devices[].password` - Login password for the JSON-RPC endpoint
- `devices[].comment` - Optional free-form note

The UI writes the same TOML layout back to disk when you save changes.

## Docker

Build and run in Docker:

```bash
$ docker build -t wrtctrl .
$ docker run -p 3021:3021 -v /etc/wrtctrl:/etc/wrtctrl wrtctrl
```

## Author(s)

- Fabian Affolter (https://github.com/fabaff)

## License

MIT. See [LICENSE](LICENSE).
