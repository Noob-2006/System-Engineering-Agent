# SE Agent app (Tauri 2)

Version **0.2.1-alpha** (`src-tauri/tauri.conf.json`; the MSI version `0.2.1` and the Android
`versionCode` 2001 are numeric-only and live in `tauri.windows.conf.json` /
`tauri.android.conf.json`; CI derives all three from the `v*` tag via `scripts/ci-config.py`).
User-facing install steps per platform: [docs/install.md §0](../../docs/install.md#0-選擇平台與安裝檔).

"Download, double-click, installed." The app is a thin shell around the headless agent:

- **Desktop** (Windows / macOS / Linux): bundles `seagent` as a Tauri sidecar, makes sure the
  background daemon runs for the current user, then shows the agent's own **web setup wizard**
  (first run) or **console** in its window. A splash page shows progress ("Installing background
  service…") and errors with **Copy details**. Tray: Open console · Pause agent · Restart service ·
  Start at login · Check for updates · Quit (agent keeps running) · Stop agent and quit.
- **Android**: the same, with the agent running under a Foreground Service — or "connect to
  another computer".
- **iOS**: the whole agent runs **inside the app process** (no child processes on iOS): the
  `seagent` library is linked through the seagent-mobile plugin (`seagent::embed`), its App Server
  listens on 127.0.0.1 and the WebView shows its wizard / console like on a desktop. Only VMs and
  native shell programs are unavailable (platform policy); commands run in the in-process
  WebAssembly sandbox. Pairing with a computer is optional. See [MOBILE.md](MOBILE.md).

It is its **own Cargo workspace** (`apps/desktop/Cargo.toml`, own `Cargo.lock`): Tauri, tokio,
wry and the WebView bindings never enter the agent's dependency tree, `cargo deny` or SBOM. See
"App shell budget" in [packaging/release/DEPENDENCY-REVIEW.md](../../packaging/release/DEPENDENCY-REVIEW.md).
(The reverse edge exists for iOS only: the app links the agent library, so the app's lockfile
lists the agent's crates too; desktop and Android builds do not compile them.)

## Layout

| Path | What |
|---|---|
| `src/` | The app's own pages (vanilla HTML/JS, shared `ui/theme.css` copied to `src/vendor/` by `build.rs`): `index.html` splash, `connect.html` pairing (phones) |
| `src-tauri/src/boot.rs` | Launch sequence: locate agent → `setup bootstrap` → `daemon status`/`start` → wait for `GET /health` → wizard (`setup link --json`) or console (`app token`) |
| `src-tauri/src/agent.rs` | Running the sidecar (no console window on Windows, timeouts, output parsing) |
| `src-tauri/src/desktop.rs` | Tray, login item (tauri-plugin-autostart, `--background`), updater (GitHub `latest.json`) |
| `src-tauri/src/pair.rs`, `tls.rs` | Setup codes, paired hosts, pinned-TLS loopback tunnel (phones / remote hosts) |
| `src-tauri/capabilities/` | `main` (own pages), `console` (127.0.0.1 pages: `autostart_get/set` only), `mobile` (QR scanner) |
| `plugins/seagent-mobile/` | Kotlin (foreground service, NSD discovery) + Swift (Bonjour discovery) plugin |
| `scripts/` | `prepare-sidecar.sh`, `prepare-android.sh`, `icons.sh`/`icons.py`, CI helpers |

## Build

Prerequisites: Rust ≥ 1.80 (the agent itself needs 1.89), the Tauri CLI
(`cargo install tauri-cli --version '^2' --locked`), and per platform:

| Platform | Install |
|---|---|
| Debian/Ubuntu | `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev pkg-config build-essential patchelf file` |
| Fedora | `sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel` |
| Windows | Visual Studio Build Tools (MSVC + Windows SDK); WebView2 is preinstalled on Windows 10/11 (the installer bootstraps it otherwise). WiX/NSIS are downloaded by the Tauri CLI. |
| macOS | Xcode command line tools; both Rust targets for a universal build |

```bash
apps/desktop/scripts/prepare-vm.sh                 # the built-in VM → src-tauri/vm/ (see below)
apps/desktop/scripts/prepare-sidecar.sh            # builds seagent → src-tauri/binaries/seagent-<host triple>
cd apps/desktop
cargo tauri dev                                    # run with the sidecar from binaries/
cargo tauri build                                  # installers in target/release/bundle/
cargo tauri build --target universal-apple-darwin  # after prepare-vm.sh + prepare-sidecar.sh universal-apple-darwin
```

**The built-in VM.** Every desktop installer carries the microVM (resources `vm/`: the guest
kernel + initramfs and the OS's hypervisor front-end — vfkit on macOS, QEMU on Windows,
Firecracker on Linux — each pinned by hash), and the agent pins the image's manifest sha256
(`SEAGENT_VM_IMAGE_MANIFEST_SHA256`). CI stages it in the `vm-image` job; locally
`scripts/prepare-vm.sh [triple]` does the same:

| Host | What prepare-vm.sh does |
|---|---|
| Linux (same CPU as the guest) | builds the image with `packaging/vm/build-image.sh --build-kernel` (packages: `build-essential flex bison bc libelf-dev cpio xz-utils musl-tools`, Rust target `<arch>-unknown-linux-musl`) |
| macOS, Windows (or the other CPU) | builds it **in a Linux container** — Docker or Podman (`CONTAINER=podman`); arm64 on Apple Silicon for the macOS app, amd64 for Windows. The image is a Linux kernel built from source and a static musl guest agent, so it cannot be built natively there |
| any | `IMAGE=<dir>` reuses a built image instead (e.g. CI's `vm-bundle-image-<arch>` artifact) |

It then runs `packaging/vm/bundle.sh --os <os> --arch <arch> --lock packaging/vm/image.lock`
into `src-tauri/vm/`. `prepare-sidecar.sh` reads the pin from `src-tauri/vm/bundle.json`, and
both it and a release `cargo tauri build` **fail** when a desktop build has no `vm/bundle.json`
or one staged for another OS — an app without it would silently fall back to the OS sandbox.
To build without the VM on purpose, set `SEAGENT_NO_VM_BUNDLE=1` for both steps.

**macOS dmg.** `cargo tauri build` lays the dmg out on an interstitial image mounted at
`/Volumes/dmg.XXXXXX`; an app started from there exits at once (no window, daemon or VM), so it can
no longer keep that image busy (`hdiutil: couldn't unmount … Resource busy`). Opening the app from
inside a read-only `.dmg` shows "drag SE Agent to Applications first" and starts nothing, and a
daemon whose executable disappears (an ejected image, a deleted app) stops itself within seconds
instead of holding port 8640.

Installers (`bundle.targets`): Windows NSIS `.exe` (per-user, `installMode: currentUser`, English /
繁體中文 / 简体中文, stops a running daemon before upgrading via `windows/hooks.nsh`) and MSI (WiX;
per-machine, for admins/GPO); macOS `.app` + `.dmg`; Linux `.AppImage`, `.deb`, `.rpm`. The agent's
`share/` is bundled as a resource and passed to the sidecar as `SEAGENT_SHARE_DIR`.

**Chinese fonts on Linux.** The UI is largely Chinese; `ui/theme.css` falls back to the native CJK
face of each OS (Microsoft JhengHei, PingFang, Noto Sans CJK, …). The `.deb` recommends
`fonts-noto-cjk` and the `.rpm` `google-noto-sans-cjk-ttc-fonts`. The AppImage, which has no package
manager behind it, **bundles Noto Sans TC** (variable TTF, 11.4 MiB, SIL OFL 1.1 — `OFL.txt` ships next
to it) under `usr/share/seagent-desktop/fonts/`. `scripts/fetch-cjk-font.sh` downloads it from a pinned
commit of googlefonts/noto-cjk and checks its SHA-256 (`prepare-sidecar.sh` runs it for Linux
triples; it is not committed to git). Only when the system has no CJK font (`fc-list :lang=zh` is
empty) does the app copy it to `~/.local/share/seagent-desktop/fonts` and set `FONTCONFIG_FILE` to the
bundled `fonts.conf` (system configuration + that font), so system fonts always win. If neither is
available (e.g. a .deb install without the recommended package), the app shows a one-time warning
with the install command.

Icons are committed (`src-tauri/icons/`), rendered from `ui/icon.svg` by `scripts/icons.sh`
(pure Python). `scripts/icons.sh --tauri` uses `cargo tauri icon` instead and also fills the
Android/iOS projects.

## Signing and updates

Nothing is required: without secrets CI builds unsigned installers (SmartScreen / Gatekeeper
warn until signing is configured). The optional secrets are listed at the top of
[.github/workflows/desktop.yml](../../.github/workflows/desktop.yml); `scripts/ci-config.py` turns
them into `tauri.ci.conf.json`.

- Updater: generate a key once with `cargo tauri signer generate -w seagent-updater.key`; store the
  private key as `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`) and the public key as
  `TAURI_UPDATER_PUBKEY`. On a `v*` tag CI uploads the installers, their `.sig` files and
  `latest.json` to the release; the app checks
  `https://github.com/Noob-2006/System-Engineering-Agent/releases/latest/download/latest.json` at
  start and from the tray. Builds without a key skip the check. Before installing an update the
  app stops the daemon (Windows locks a running `seagent.exe`); the relaunched app starts it again.
- Windows: `WINDOWS_CERTIFICATE` (pfx) → `certificateThumbprint`, or Azure Trusted Signing
  (`AZURE_*` → `signCommand` with `trusted-signing-cli`).
- macOS: `APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY`; notarization with `APPLE_ID`,
  `APPLE_PASSWORD`, `APPLE_TEAM_ID`. Unsigned builds are ad-hoc signed (`signingIdentity: "-"`).

## Agent CLI the app relies on

Exists today: `setup bootstrap` (stderr `console: http://…`), `daemon status|start|stop`,
`setup link --json`, `app token`, `app status`, `pause [reason]` / `resume` / `pause status`,
`service restart`, `health`. Not yet in the agent (the app has fallbacks; see the module docs in
`src-tauri/src/agent.rs` and [MOBILE.md](MOBILE.md#agent-side-changes-needed)):

- `seagent setup status --json` → `{done, listen, console_url}` (today `setup status` falls into the
  interactive wizard, so the app infers "done" from `setup link --json` failing);
- `seagent app token --json` → `{token, url}`, non-zero exit when no token is set (today it prints
  a how-to with exit 0, which the app has to recognise);
- Windows/macOS ports of `daemon start` (detached, no console window) and of the wizard's
  autostart step (Windows service / launchd); the app's own "Start at login" item is independent.
