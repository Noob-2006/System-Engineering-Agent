use std::path::Path;

/// Commands the app's own pages may call. Declaring them makes Tauri check every app command
/// against the capabilities in `capabilities/` (local pages: `main.json`; the agent's console and
/// setup wizard served from 127.0.0.1: `console.json`, autostart plus `console_access_token`).
const COMMANDS: &[&str] = &["app_info", "boot_start", "boot_state", "set_mode", "autostart_get", "autostart_set", "discover_hosts", "parse_setup_code", "probe_host", "pair_host", "hosts_list", "host_open", "host_forget", "show_local", "console_access_token"];

fn main() {
    // The splash pages use the project's shared theme (ui/theme.css + the star mask); copy it in
    // so `frontendDist` stays self-contained. src/vendor/ is git-ignored.
    let ui = Path::new("../../../ui");
    let vendor = Path::new("../src/vendor");
    let _ = std::fs::create_dir_all(vendor);
    for f in ["theme.css", "star.svg", "icon.svg"] {
        let from = ui.join(f);
        println!("cargo:rerun-if-changed={}", from.display());
        if from.is_file() {
            std::fs::copy(&from, vendor.join(f)).unwrap_or_else(|e| panic!("copy {}: {e}", from.display()));
        }
    }
    // iOS bundles the WASI tools (`tauri.ios.conf.json` resources → share/wasi). They are built
    // by scripts/prepare-ios.sh; the directory must exist for tauri-build even before that.
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("apple-ios") {
        let _ = std::fs::create_dir_all("wasi-tools");
        // Only a real app build (on a Mac) needs them; a cross type-check elsewhere skips quietly.
        if cfg!(target_os = "macos") && !Path::new("wasi-tools/tools.json").is_file() {
            println!("cargo:warning=src-tauri/wasi-tools has no tools.json: the app would have no shell commands; run scripts/prepare-ios.sh");
        }
    }
    // Desktop installers ship the built-in VM (`tauri.{windows,macos,linux}.conf.json` resources
    // `vm/`), staged by packaging/vm/bundle.sh. The directory must exist for tauri-build; an
    // installer built without it would fall back to the OS sandbox, so say so loudly.
    if !target.contains("android") && !target.contains("apple-ios") {
        let _ = std::fs::create_dir_all("vm");
        println!("cargo:rerun-if-changed=vm/bundle.json");
        println!("cargo:rerun-if-env-changed=SEAGENT_NO_VM_BUNDLE");
        if !Path::new("vm/bundle.json").is_file() {
            // A release build (`cargo tauri build`) makes the installers people get: without the VM
            // it must fail, not warn into a scrolled-away log (#15). Dev builds keep working.
            let release = std::env::var("PROFILE").as_deref() == Ok("release");
            let opted_out = std::env::var("SEAGENT_NO_VM_BUNDLE").as_deref() == Ok("1");
            if release && !opted_out {
                panic!("src-tauri/vm has no bundle.json: this app would ship WITHOUT its built-in VM. Run apps/desktop/scripts/prepare-vm.sh <triple> (then prepare-sidecar.sh), or set SEAGENT_NO_VM_BUNDLE=1 to build without the VM on purpose.");
            }
            println!("cargo:warning=src-tauri/vm has no bundle.json: this build has NO built-in VM (tools fall back to the OS sandbox); stage it with apps/desktop/scripts/prepare-vm.sh");
        }
    }
    println!("cargo:rustc-env=SEAGENT_TARGET_TRIPLE={target}");
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS))).expect("tauri build script");
}
