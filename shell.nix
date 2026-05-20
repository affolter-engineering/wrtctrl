{ pkgs ? import <nixpkgs> {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  }
}:

pkgs.mkShell {
  name = "wrtctrl";

  nativeBuildInputs = with pkgs; [
    (rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" "rust-analyzer" ];
      targets = [ "wasm32-unknown-unknown" "x86_64-unknown-linux-musl" ];
    })
    zig
    cargo-zigbuild
    ripgrep
    cargo-make
    cargo-leptos

    tailwindcss_4

    pkg-config
    openssl
    lld
    wasm-bindgen-cli
    binaryen   # wasm-opt (used by cargo-leptos in release mode)
    sass       # optional: if you add .scss files later

    ripgrep
  ];

  buildInputs = with pkgs; [ openssl ];

  OPENSSL_NO_VENDOR = "1";
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
  CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";

  shellHook = ''
    echo "✓ cargo-leptos $(cargo-leptos --version 2>/dev/null | head -1)"

    echo ""
    echo "╔══════════════════════════════════════════╗"
    echo "║  wrtctrl dev shell                       ║"
    echo "╠══════════════════════════════════════════╣"
    echo "║  cargo make dev    -> hot-reload server  ║"
    echo "║  cargo make build  -> release build      ║"
    echo "║  cargo make test   -> run tests          ║"
    echo "║  cargo make check  -> fmt & clippy       ║"
    echo "╚══════════════════════════════════════════╝"
  '';
}
