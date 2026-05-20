use std::{env, fs, path::PathBuf};

fn format_bytes_const(name: &str, bytes: &[u8]) -> String {
    let body = bytes
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("pub static {name}: &[u8] = &[{body}];\n")
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));

    let generated_tailwind_css_path = manifest_dir.join("target/tmp/tailwind.css");
    let default_css_path = manifest_dir.join("target/site/pkg/wrtctrl.css");
    let alternate_css_path = manifest_dir.join("site/pkg/wrtctrl.css");
    let configured_css_path = env::var_os("WRTCTRL_EMBEDDED_CSS").map(PathBuf::from);

    let js_primary = manifest_dir.join("target/site/pkg/wrtctrl.js");
    let js_fallback = manifest_dir.join("site/pkg/wrtctrl.js");
    let configured_js_path = env::var_os("WRTCTRL_EMBEDDED_JS").map(PathBuf::from);

    let wasm_bg_primary = manifest_dir.join("target/site/pkg/wrtctrl_bg.wasm");
    let wasm_primary = manifest_dir.join("target/site/pkg/wrtctrl.wasm");
    let wasm_bg_fallback = manifest_dir.join("site/pkg/wrtctrl_bg.wasm");
    let wasm_fallback = manifest_dir.join("site/pkg/wrtctrl.wasm");
    let configured_wasm_path = env::var_os("WRTCTRL_EMBEDDED_WASM").map(PathBuf::from);

    let css_path = configured_css_path
        .into_iter()
        .chain([
            default_css_path.clone(),
            alternate_css_path.clone(),
            generated_tailwind_css_path.clone(),
        ])
        .find(|path| path.exists());

    let js_path = configured_js_path
        .into_iter()
        .chain([js_primary.clone(), js_fallback.clone()])
        .find(|path| path.exists());

    let wasm_path = configured_wasm_path
        .into_iter()
        .chain([
            wasm_bg_primary.clone(),
            wasm_primary.clone(),
            wasm_bg_fallback.clone(),
            wasm_fallback.clone(),
        ])
        .find(|path| path.exists());

    let mut css = css_path
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();

    // Some build flows can leave pkg CSS as raw Tailwind source directives.
    // If that happens, prefer the compiled tailwind output.
    if css.contains("@import \"tailwindcss\"") {
        if let Ok(compiled_css) = fs::read_to_string(&generated_tailwind_css_path) {
            if !compiled_css.trim().is_empty() && !compiled_css.contains("@import \"tailwindcss\"") {
                css = compiled_css;
            }
        }
    }

    let js_bytes = js_path
        .as_ref()
        .and_then(|path| fs::read(path).ok())
        .unwrap_or_default();

    let wasm_bytes = wasm_path
        .as_ref()
        .and_then(|path| fs::read(path).ok())
        .unwrap_or_default();

    let mut embedded = String::new();
    embedded.push_str(&format!("pub const EMBEDDED_CSS: &str = {css:?};\n"));
    embedded.push_str(&format_bytes_const("EMBEDDED_JS", &js_bytes));
    embedded.push_str(&format_bytes_const("EMBEDDED_WASM", &wasm_bytes));

    fs::write(out_dir.join("embedded_assets.rs"), embedded)
        .expect("failed to write embedded_assets.rs");

    println!("cargo:rerun-if-env-changed=WRTCTRL_EMBEDDED_CSS");
    println!("cargo:rerun-if-env-changed=WRTCTRL_EMBEDDED_JS");
    println!("cargo:rerun-if-env-changed=WRTCTRL_EMBEDDED_WASM");
    println!("cargo:rerun-if-changed={}", generated_tailwind_css_path.display());
    println!("cargo:rerun-if-changed={}", default_css_path.display());
    println!("cargo:rerun-if-changed={}", alternate_css_path.display());
    println!("cargo:rerun-if-changed={}", js_primary.display());
    println!("cargo:rerun-if-changed={}", js_fallback.display());
    println!("cargo:rerun-if-changed={}", wasm_bg_primary.display());
    println!("cargo:rerun-if-changed={}", wasm_primary.display());
    println!("cargo:rerun-if-changed={}", wasm_bg_fallback.display());
    println!("cargo:rerun-if-changed={}", wasm_fallback.display());
    println!("cargo:rerun-if-changed=style/tailwind.css");
    println!("cargo:rerun-if-changed=build.rs");

    if css.is_empty() {
        println!(
            "cargo:warning=No generated CSS found for embedding; /pkg/wrtctrl.css will fall back to the site directory if present"
        );
    }
    if js_bytes.is_empty() {
        println!(
            "cargo:warning=No generated JS found for embedding; /pkg/wrtctrl.js will fall back to the site directory if present"
        );
    }
    if wasm_bytes.is_empty() {
        println!(
            "cargo:warning=No generated WASM found for embedding; /pkg/wrtctrl_bg.wasm will fall back to the site directory if present"
        );
    }
}
