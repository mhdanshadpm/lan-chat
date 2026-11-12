# LAN Chat

A serverless chat app for your local network. Devices find each other with
mDNS and talk directly over WebSockets — no internet, no accounts, no central
server, no message data leaving the LAN.

- **Desktop targets now:** Windows 10/11 (x64) and macOS 10.15+ (Apple Silicon
  and Intel, including universal binaries).
- **Designed to grow:** all OS-specific code lives behind one trait, so Linux,
  Android and iOS can be added without touching the protocol, store, discovery,
  group or transfer logic.
- **Two surfaces only:** the main window owns history and management; a small
  always-on-top bubble handles quick replies without opening the app.

![stack](https://img.shields.io/badge/Tauri-2-24C8DB) ![Rust](https://img.shields.io/badge/Rust-1.79%2B-000000) ![React](https://img.shields.io/badge/React-18-61DAFB) ![SQLite](https://img.shields.io/badge/SQLite-bundled-003B57)

---

## Contents

- [Features](#features)
- [Architecture](#architecture)
- [Workspace layout](#workspace-layout)
- [Prerequisites](#prerequisites)
- [Run in development](#run-in-development)
- [Build for macOS](#build-for-macos)
- [Build for Windows](#build-for-windows)
- [macOS signing and notarization](#macos-signing-and-notarization)
- [Windows code signing](#windows-code-signing)
- [Release workflow](#release-workflow)
- [How to add a new platform](#how-to-add-a-new-platform)
- [Ports, firewall, and macOS Local Network permission](#ports-firewall-and-macos-local-network-permission)
- [Security model](#security-model)
- [Local data](#local-data)
- [Tests](#tests)
- [Protocol documentation](#protocol-documentation)
- [Troubleshooting](#troubleshooting)
- [License](#license)

---

## Features

### Networking

- mDNS/DNS-SD discovery (`_lanchat._tcp.local.`) plus a UDP broadcast fallback.
- Every instance runs a WebSocket server on a random free TCP port and dials
  every discovered peer (full mesh).
- Persistent device UUID, display name, avatar colour, and device name.
- Presence via heartbeats (5 s), timeout detection (20 s), reconnect backoff,
  duplicate-connection tie-breaks, and automatic re-dial after network changes.
- Optional shared LAN passphrase: Argon2id key derivation + XChaCha20-Poly1305
  for every frame after the handshake.
- Wire protocol is versioned (`"v": 1` on every frame and discovery beacon).

### Chat

- One-to-one chats and groups (create, rename, add/remove members, leave),
  plus an automatic **Everyone** group for the whole LAN.
- Text and emoji, mentions (`@name`), replies, typing indicators, unread
  badges, read receipts, and search across the full history.
- Delivery states: sending / sent / delivered / read.
- Offline queue: messages for offline peers are stored and flushed when the
  peer reconnects.
- Everything persists locally in SQLite and is shown only in the main window.

### Files

- Images, videos, audio files (shown as voice notes in the UI), and arbitrary
  files.
- 64 KiB chunked transfers with progress, SHA-256 verification, and resume
  after reconnects or app restarts.
- Locally generated thumbnails (JPEG for images; video frames via `ffmpeg`
  when available) are sent with the offer so previews render before download.
- Auto-accept small files, ask for large ones, configurable size limit.

### Main window

- Responsive layout: sidebar + conversation on desktop, list/chat navigation
  on narrow screens (ready for mobile).
- Sidebar with peers, presence, direct chats, groups, unread counts.
- Settings: theme, bubble options, DND, notification sound, read receipts,
  typing indicators, auto-accept/max file size, discovery port, passphrase,
  launch at login.
- First-run setup: display name and avatar colour.
- Light/dark/system theme.

### Bubble

- Frameless, transparent, always-on-top window near a screen corner.
- Shows sender avatar + name (and group name for group messages), the latest
  message only, and a one-line reply field.
- Attach files, paste images, drag files onto the bubble, emoji picker.
- Messages from the same chat update in place; messages from other chats
  stack with a badge and can be flipped through.
- Sending shows a brief "Sent" tick; the bubble auto-hides after a
  configurable idle delay.
- Does not steal focus when it appears; dragging moves the window and the
  position is remembered.
- Optional collapsed circular avatar with unread count.
- Configurable corner, opacity, auto-hide delay, DND, per-chat mute.
- Windows: topmost + skip taskbar. macOS: floating level, all Spaces, over
  fullscreen, no Dock icon while only the bubble is visible.
- Global hotkey `Ctrl+Shift+Space` / `Cmd+Shift+Space` summons the bubble for
  the last active chat and focuses the reply box.
- Tray (Windows) / menu bar (macOS): Open App, Mute, Quit.
- Native notification sound and launch-at-login option.

### Bubble controls

| Action | Result |
| ------ | ------ |
| Click the header and drag | Moves the bubble; position is saved |
| Enter | Sends the reply |
| Paste an image | Attaches it to the current conversation |
| Drop files | Attaches them to the current conversation |
| `1 / N` chevrons | Flips between stacked conversations |
| Circular avatar | Expands/collapses the bubble |
| Open in app | Opens the main window on that conversation |
| Escape | Hides the bubble |

---

## Architecture

Three Rust crates and one React app. Dependencies point in one direction:

```
src-tauri (lanchat-app)  ──>  platform (lanchat-platform)  ──>  Tauri
        │
        └──────────────>  core (lanchat-core)  ──>  tokio / rusqlite / serde
                                   ^
React UI (src/) ── Tauri commands / events ──┘
```

- **`core`** is OS-agnostic and has no Tauri dependency. It owns the wire
  protocol, discovery, the WebSocket mesh, encryption, SQLite, groups, and
  file transfer. The UI observes `CoreEvent`s and calls `Node` methods.
- **`platform`** is the only crate containing `#[cfg(target_os = "...")]`. It
  defines `PlatformServices` for bubble window behaviour, tray/menu bar,
  notifications, sound, reveal/open file, autostart, and global hotkeys.
  Windows and macOS are implemented; Linux/Android/iOS are marked stubs.
- **`src-tauri`** is glue only: Tauri commands, window management, tray and
  hotkey wiring, event forwarding to the webview.
- **`src/`** is the React + TypeScript + Tailwind UI, split into the main
  window (`index.html`) and the bubble (`bubble.html`).

Event flow:

```
core Node ──broadcast::Receiver<CoreEvent>──> src-tauri/events.rs
      ──emit("core://event")──> React main window + bubble
      ──incoming message──> bubble.rs decides whether to show
      ──emit("bubble://incoming" | "bubble://summon" | "bubble://open")
```

The common `CoreEvent` payloads include `peer_updated`, `peer_offline`,
`message`, `message_state`, `typing`, `group_updated`, `conversation_updated`,
`transfer`, `listening`, `network_changed`, `settings`, and `error`.

### Concurrency model

- One tokio runtime started by Tauri's async runtime.
- Per-connection reader/writer tasks with an `mpsc` outbound queue.
- `broadcast` channel for core events; 2048-frame buffer.
- SQLite access is synchronous behind `rusqlite`; WAL mode is enabled and
  writes are short.
- Transfer tasks are spawned per `(transfer_id, peer)` and guarded by an
  active-transfer set.

---

## Workspace layout

```
.
├── Cargo.toml                     # workspace: core, platform, src-tauri
├── core/
│   ├── src/protocol/
│   │   ├── mod.rs                 # Envelope, limits, validation
│   │   └── messages.rs            # every frame body type
│   ├── src/discovery/mod.rs       # mDNS + UDP beacon + network fingerprint
│   ├── src/net/mod.rs             # WebSocket mesh, handshake, outbox, groups
│   ├── src/crypto.rs              # Argon2id + XChaCha20-Poly1305
│   ├── src/store/                 # SQLite store, models, CoreEvent
│   ├── src/group.rs               # Everyone group, @mention parsing
│   ├── src/transfer/mod.rs        # chunked transfer, thumbnails
│   ├── src/identity.rs            # UUID, name, avatar, device name
│   └── src/config.rs              # Settings / bubble defaults
├── platform/
│   ├── src/lib.rs                 # PlatformServices trait + current()
│   ├── src/desktop.rs             # shared Tauri plugin calls
│   ├── src/macos.rs               # NSWindow shim, Dock icon, afplay
│   ├── src/windows.rs             # Win32 topmost/no-focus shim
│   ├── src/linux.rs               # compile-and-run stub
│   ├── src/android.rs             # marked stub
│   ├── src/ios.rs                 # marked stub
│   └── native/{macos.m,windows.c}
├── src-tauri/
│   ├── src/lib.rs                 # Tauri setup, plugins, command registry
│   ├── src/commands.rs            # all #[tauri::command] handlers
│   ├── src/bubble.rs              # bubble show/hide/position/size/summon
│   ├── src/tray.rs                # tray/menu bar + hotkey
│   ├── src/events.rs              # CoreEvent → webview events
│   ├── src/state.rs               # AppState { node }
│   ├── Info.plist                 # Local Network + Bonjour usage
│   ├── entitlements.plist         # network client/server, audio input, JIT
│   └── tauri.conf.json            # windows, bundle, DMG/NSIS settings
├── src/
│   ├── App.tsx                    # main window state + wiring
│   ├── api.ts                     # typed Tauri command/event bridge
│   ├── mock.ts                    # browser-only demo bridge
│   ├── components/                # Sidebar, ChatView, Settings, modals, ...
│   └── bubble/Bubble.tsx          # quick-reply bubble
├── docs/PROTOCOL.md               # normative wire protocol v1
├── index.html                     # main window entry
└── bubble.html                    # bubble entry
```

---

## Prerequisites

Common:

- **Node.js 20+** and npm
- **Rust 1.79+** (stable) with Cargo
- **Tauri CLI 2** — installed as a dev dependency (`npm install`)

macOS:

- Xcode Command Line Tools (`xcode-select --install`)
- Optional: `ffmpeg` on `PATH` for video thumbnails
- For a universal binary: both Rust targets (see below)

Windows:

- Microsoft C++ Build Tools (MSVC) and the Windows SDK
- WebView2 runtime (preinstalled on Windows 10/11; the installer can bootstrap it
  if missing)
- Optional: `ffmpeg` on `PATH` for video thumbnails
- WiX Toolset v3 for MSI builds; NSIS is fetched automatically by Tauri
- Build on Windows directly — cross-compiling the Tauri shell from macOS is not
  supported

Linux (if you try the stub): see the Tauri Linux prerequisites;
`libayatana-appindicator3-dev`, `librsvg2-dev`, `libwebkit2gtk-4.1-dev`,
`patchelf`, and a C compiler.

Install JS dependencies once:

```bash
npm install
```

---

## Run in development

Desktop app with the real Rust backend:

```bash
npm run tauri dev
```

The first launch creates an identity and opens the first-run setup.

Frontend only, without Tauri:

```bash
npm run dev
```

This runs the UI in a browser against `src/mock.ts`, a demo bridge with fake
peers, groups, and messages. It exists purely for UI iteration; no mock code is
bundled into the desktop app. The bubble is at
`http://localhost:1420/bubble.html`.

Type-check and build the web assets:

```bash
npm run build
```

Backend checks:

```bash
cargo check --workspace
cargo test --workspace
```

Environment variables:

| Variable | Effect |
| -------- | ------ |
| `LANCHAT_LOG` | Rust log filter (default `info`), e.g. `LANCHAT_LOG=debug` |
| `LANCHAT_DISCOVERY_PORT` | Overrides the UDP discovery port at startup (default `47823`) |

---

## Build for macOS

### Universal binary (recommended)

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

Outputs (paths follow the product name and version):

```
target/universal-apple-darwin/release/bundle/macos/LAN Chat.app
target/universal-apple-darwin/release/bundle/dmg/LAN Chat_0.1.0_universal.dmg
```

Because this is a Cargo workspace, Cargo and Tauri write bundles to the
workspace-root `target/` directory, not `src-tauri/target/`.

The DMG layout is configured in `src-tauri/tauri.conf.json` (app icon placement
and Applications folder shortcut). `bundle.targets` is `"all"`; to build a
single artifact explicitly:

```bash
npm run tauri build -- --target universal-apple-darwin --bundles dmg
npm run tauri build -- --target universal-apple-darwin --bundles app
```

### Apple Silicon only / Intel only

```bash
npm run tauri build -- --target aarch64-apple-darwin
npm run tauri build -- --target x86_64-apple-darwin
```

Bundles land under `target/<target>/release/bundle/`. To create a DMG
from an existing `.app` without Tauri, use `hdiutil` or `create-dmg`.

Minimum system version is **10.15** (`bundle.macOS.minimumSystemVersion`).

---

## Build for Windows

Run on Windows with the MSVC toolchain:

```powershell
npm install
npm run tauri build
```

`targets: "all"` produces both an NSIS installer and an MSI. To build one:

```powershell
# NSIS setup .exe (recommended; per-user install)
npm run tauri build -- --bundles nsis

# MSI (requires WiX Toolset v3)
npm run tauri build -- --bundles msi
```

Outputs:

```
target\release\bundle\nsis\LAN Chat_0.1.0_x64-setup.exe
target\release\bundle\msi\LAN Chat_0.1.0_x64_en-US.msi
```

NSIS is configured for a **current-user** install (no admin prompt):

```json
"windows": {
  "nsis": { "installMode": "currentUser", "displayLanguageSelector": false }
}
```

The app is a normal GUI executable; closing the main window hides it to the
tray/menu bar. Use **Quit LAN Chat** to exit.

---

## macOS signing and notarization

Signing is required for distribution outside the App Store on modern macOS.
The bundle already enables the hardened runtime and uses
`src-tauri/entitlements.plist`:

```xml
<key>com.apple.security.network.client</key><true/>
<key>com.apple.security.network.server</key><true/>
<key>com.apple.security.device.audio-input</key><true/>
<key>com.apple.security.cs.allow-jit</key><true/>
<key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/>
```

`network.client` and `network.server` are required for WebSockets both ways.
The JIT / unsigned-memory entitlements are needed by the WebView.

### Automatic (Tauri, recommended)

Install a **Developer ID Application** certificate in the login keychain and
export these variables before building:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_CERTIFICATE="$(base64 -i /path/to/developer-id.p12)"
export APPLE_CERTIFICATE_PASSWORD="p12-password"

# Notarization with an Apple ID and an app-specific password:
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="abcd-efgh-ijkl-mnop"
export APPLE_TEAM_ID="TEAMID"

npm run tauri build -- --target universal-apple-darwin
```

Tauri signs the `.app`/`.dmg`, submits it to the notary service, and staples
the ticket when the credentials are present. For CI, you can instead use an
App Store Connect API key:

```bash
export APPLE_API_KEY="/path/to/AuthKey_ABC123XYZ.p8"
export APPLE_API_ISSUER="00000000-0000-0000-0000-000000000000"
export APPLE_API_KEY_PATH="/path/to/AuthKey_ABC123XYZ.p8"
```

### Manual notarization

If you built an unsigned or signed-but-unnotarized app:

```bash
# Store notary credentials once
xcrun notarytool store-credentials "lanchat-notary" \
  --apple-id "you@example.com" \
  --team-id "TEAMID" \
  --password "app-specific-password"

# Submit and wait
ditto -c -k --keepParent "target/universal-apple-darwin/release/bundle/macos/LAN Chat.app" LANChat.zip
xcrun notarytool submit LANChat.zip --keychain-profile "lanchat-notary" --wait

# Staple the ticket to the app, then rebuild the DMG
xcrun stapler staple "target/universal-apple-darwin/release/bundle/macos/LAN Chat.app"
hdiutil create -volname "LAN Chat" -srcfolder "LAN Chat.app" -ov -format UDZO LANChat.dmg
```

Verify:

```bash
codesign --verify --deep --strict --verbose=2 "LAN Chat.app"
spctl --assess --type execute --verbose=4 "LAN Chat.app"
xcrun stapler validate "LAN Chat.app"
```

Notes:

- The bundle identifier is `com.lanchat.desktop`; change it in `tauri.conf.json`
  before signing if you distribute under a different name.
- The hardened runtime is enabled; do not disable it for distribution.
- If you later add microphone recording, add `NSMicrophoneUsageDescription` to
  `src-tauri/Info.plist` (the audio-input entitlement alone is not enough).
- Ad-hoc signatures (`codesign -s -`) are fine for local testing but will not
  notarize.

---

## Windows code signing

Authenticode signing avoids SmartScreen warnings for distributed installers.
Add signing settings to `tauri.conf.json` (or pass them via environment/CI):

```json
"bundle": {
  "windows": {
    "certificateThumbprint": "A1B2C3...",
    "digestAlgorithm": "sha256",
    "timestampUrl": "http://timestamp.digicert.com"
  }
}
```

An EV certificate or a cloud HSM signing service (Azure Trusted Signing,
DigiCert KeyLocker) is recommended. Sign both the installer and the app
executable if you use a custom pipeline. Unsigned local builds are fine for
development; Windows SmartScreen will warn until the binary builds reputation.

---

## Release workflow

[`.github/workflows/release.yml`](.github/workflows/release.yml) builds the
desktop installers and publishes a GitHub release. It runs on `ubuntu-latest`
for the release job, `macos-latest` for a universal macOS build, and
`windows-latest` for Windows.

Cut a release by pushing a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

You can also run **Actions → Release → Run workflow** and enter a tag. The
workflow creates that tag at the selected commit if it does not exist.

The workflow:

1. Builds a universal `.dmg` (`--target universal-apple-darwin`) on macOS.
2. Builds an NSIS `.exe` and an MSI on Windows (`--bundles nsis,msi`).
3. Publishes a GitHub release for the tag with generated release notes and
   uploads every installer as a release asset.

The tag name is used as the release name (for example `LAN Chat v0.1.0`).
Prereleases are not detected automatically; mark the release as a prerelease
in the GitHub UI, or add a `prerelease` rule to the workflow.

### Signed macOS releases

The macOS build is unsigned unless these repository secrets are all set
(Settings → Secrets and variables → Actions):

| Secret | Value |
| ------ | ----- |
| `APPLE_CERTIFICATE` | Base64 of the Developer ID Application `.p12` (`base64 -i cert.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Apple account email |
| `APPLE_PASSWORD` | App-specific password for notarization |
| `APPLE_TEAM_ID` | Apple Developer Team ID |

When any of them is missing, the workflow skips signing and produces an
unsigned DMG, which requires a right-click → Open on macOS. Windows releases
are unsigned unless you configure Authenticode in `tauri.conf.json` as shown
above.

The workflow needs `contents: write` (already set) to create the release and
upload assets using the automatic `GITHUB_TOKEN`.

---

## How to add a new platform

The goal is that adding a target never touches `core`.

### 1. Implement `PlatformServices`

Create `platform/src/<platform>.rs` and implement the trait from
`platform/src/lib.rs`:

```rust
pub struct MyPlatform;

impl PlatformServices for MyPlatform {
    fn name(&self) -> &'static str { "myplatform" }
    fn capabilities(&self) -> Capabilities { /* ... */ }
    fn configure_bubble(&self, window: &tauri::WebviewWindow, options: BubbleWindowOptions) -> Result<()> { /* ... */ }
    fn show_bubble_without_focus(&self, window: &tauri::WebviewWindow) -> Result<()> { /* ... */ }
    fn set_dock_icon_visible(&self, app: &AppHandle, visible: bool) -> Result<()> { /* ... */ }
    fn show_notification(&self, app: &AppHandle, spec: &NotificationSpec) -> Result<()> { /* ... */ }
    fn play_notification_sound(&self) -> Result<()> { /* ... */ }
    fn reveal_file(&self, path: &Path) -> Result<()> { /* ... */ }
    fn open_path(&self, path: &Path) -> Result<()> { /* ... */ }
    fn set_autostart(&self, app: &AppHandle, enabled: bool) -> Result<()> { /* ... */ }
    fn autostart_enabled(&self, app: &AppHandle) -> Result<bool> { /* ... */ }
    fn register_hotkey(&self, app: &AppHandle, accelerator: &str) -> Result<()> { /* ... */ }
    fn unregister_all_hotkeys(&self, app: &AppHandle) -> Result<()> { /* ... */ }
}
```

For unsupported features return
`PlatformError::Unsupported("reason")` — callers already log and degrade
gracefully. Reuse `desktop.rs` for plugin-based pieces (notifications,
autostart, hotkeys).

### 2. Register it

In `platform/src/lib.rs`:

```rust
#[cfg(target_os = "myplatform")]
mod myplatform;

pub fn current() -> &'static dyn PlatformServices {
    #[cfg(target_os = "myplatform")]
    { return &myplatform::MyPlatform; }
    // existing branches ...
}
```

Add `#[cfg(target_os = "myplatform")]` arms in `platform/build.rs` only if you
need native helpers. This is the only crate where target `cfg`s are allowed.

### 3. Make Tauri support the target

- Add the target's triple to the build/CI matrix.
- Provide icons and bundle settings in `tauri.conf.json`
  (`bundle.targets`, platform-specific sections, identifier).
- Add required capabilities in `src-tauri/capabilities/`.
- Gate anything the target cannot do at runtime through
  `PlatformServices::capabilities()` instead of spreading `cfg`s.

For **mobile** (Tauri 2):

```bash
npm run tauri android init
npm run tauri ios init
npm run tauri android build
npm run tauri ios build
```

Android specifics:

- Add `android.permission.INTERNET`,
  `android.permission.CHANGE_WIFI_MULTICAST_STATE`, and
  `android.permission.ACCESS_WIFI_STATE`.
- Acquire a `WifiManager.MulticastLock` before mDNS/UDP discovery, otherwise
  multicast packets are dropped when the screen is off.
- A foreground service is needed to keep the socket alive in the background.
- The floating bubble requires `SYSTEM_ALERT_WINDOW`; if not granted, fall back
  to notifications with quick reply. Tauri windows cannot float over other
  apps without an overlay permission.
- Notifications need `POST_NOTIFICATIONS` on Android 13+.

iOS specifics:

- Add `NSLocalNetworkUsageDescription` and `NSBonjourServices`
  (`_lanchat._tcp`) to `Info.plist` — the desktop file is a good template.
- Multicast/broadcast requires the
  `com.apple.developer.networking.multicast` entitlement (request it from
  Apple). Without it, use Bonjour only (`NWBrowser`/`NetService`).
- Third-party apps cannot draw a floating bubble over other apps. Use local
  notifications with a text reply action instead; the main UI and the core
  networking still work unchanged.
- Background execution is time-limited; reconnect when the app returns to the
  foreground. The core's reconnect logic handles this.

Linux:

- `platform/src/linux.rs` is a working starting point. Floating over fullscreen
  Wayland clients depends on the compositor; `bubble_over_fullscreen` is
  reported as `false`.
- For tray support install `libayatana-appindicator3`; for sound use
  `canberra-gtk-play` or `paplay`.

### 4. Update docs and tests

- Add a row to the capability matrix in this README.
- Document firewall/permission requirements for the new OS.
- Run `cargo check --workspace --target <triple>` and the core test suite.
- Do not add OS `cfg`s outside `platform/`. If a core change seems necessary,
  add an abstraction to `core` (e.g. a discovery adapter) and implement it in
  `platform`.

---

## Ports, firewall, and macOS Local Network permission

### Ports used

| Port | Protocol | Direction | Purpose |
| ---- | -------- | --------- | ------- |
| Random ephemeral | TCP | inbound + outbound | WebSocket server per instance (`0.0.0.0:0`, HTTP path `/lanchat`). Chosen by the OS at startup; changes every launch. |
| **47823** (default) | UDP | inbound + outbound | Discovery beacon; broadcast every 5 s to `255.255.255.255` and per-interface broadcast addresses. Configurable in Settings (restart required) or via `LANCHAT_DISCOVERY_PORT`. |
| 5353 | UDP | inbound + outbound | mDNS/DNS-SD multicast (224.0.0.251 / ff02::fb). |
| Ephemeral fallback | UDP | inbound + outbound | Used by a second instance on the same machine when 47823 is taken; advertised in the beacon's `disc_port`. |
| 443/80 | TCP | outbound | Not used by LAN Chat. No internet access is required. |

There is no fixed server port. Because the WebSocket port is random, firewall
rules should be **per application**, not per port, so they survive relaunches.

### Windows Firewall

On first launch Windows shows a Defender Firewall prompt. Allow access on
**Private networks**; do not enable Public unless you trust the network.

If the prompt was dismissed, add rules manually (adjust the program path for
MSI installs, which default to `C:\Program Files\LAN Chat\LAN Chat.exe`):

```powershell
# Allow the executable (covers the random TCP port)
New-NetFirewallRule -DisplayName "LAN Chat" `
  -Direction Inbound -Program "$env:LOCALAPPDATA\LAN Chat\LAN Chat.exe" `
  -Profile Private -Action Allow

# Allow discovery broadcasts if you use a custom port
New-NetFirewallRule -DisplayName "LAN Chat discovery" `
  -Direction Inbound -Protocol UDP -LocalPort 47823 `
  -Profile Private -Action Allow
```

`netsh` equivalent:

```bat
netsh advfirewall firewall add rule name="LAN Chat" dir=in action=allow program="%LOCALAPPDATA%\LAN Chat\LAN Chat.exe" profile=private
```

Notes:

- Multiple devices on the same subnet can talk directly; routed/VLAN traffic
  needs the firewall to allow the app on those profiles too.
- Guest Wi-Fi with **client/AP isolation** blocks all peer-to-peer traffic —
  no app setting can work around that.
- Corporate networks may block multicast and UDP broadcast; mDNS may also be
  filtered. LAN Chat falls back to UDP, but both can be blocked. A flat,
  trusted LAN is the intended environment.

### macOS Local Network permission

Since macOS 15 (Sequoia), apps must get user consent to access the local
network. `src-tauri/Info.plist` already provides the required keys:

```xml
<key>NSLocalNetworkUsageDescription</key>
<string>LAN Chat finds other people on your Wi-Fi or wired network ...</string>
<key>NSBonjourServices</key>
<array><string>_lanchat._tcp</string></array>
```

- The prompt appears the first time discovery starts.
- Allow it in **System Settings → Privacy & Security → Local Network**.
- If denied, mDNS and UDP broadcast are blocked and the app cannot find peers
  (direct connections by IP are not exposed in the UI).
- After a rebuild with a changed signature, macOS may treat the app as new and
  ask again. To reset during development:

```bash
tccutil reset LocalNetwork com.lanchat.desktop
```

- Unsigned/ad-hoc builds still get the prompt, but a stable Developer ID
  signature is recommended so the permission persists across updates.
- macOS 10.15–14 do not require this prompt, but mDNS should still be allowed
  by any third-party firewall.

### Quick network checklist

1. Both devices on the same subnet (same Wi-Fi SSID or wired LAN).
2. Firewall allows the app on Private networks.
3. macOS Local Network permission granted.
4. Client isolation disabled on the router/guest network.
5. Optional passphrase identical on both devices (a mismatch shows as an
   offline peer; check logs with `LANCHAT_LOG=debug`).

---

## Security model

- **Transport encryption is optional.** With a LAN passphrase configured on
  both sides, the handshake proves knowledge of the passphrase without sending
  it:
  - Argon2id (`m=19456 KiB`, `t=2`, `p=1`) derives a 32-byte key from the
    passphrase and a per-connection 16-byte salt.
  - The dialer sends a proof in `hello`; the acceptor sends a proof in
    `hello_ack`. Each proof is an XChaCha20-Poly1305 ciphertext of a string
    bound to both peer UUIDs.
  - Every frame after the handshake is sealed as a `sealed` envelope around a
    complete inner envelope (24-byte random nonce, AAD `lanchat/frame-v1`).
- **Handshake metadata is visible** on the LAN before the tunnel exists
  (display name, avatar colour, device name, app version). Message contents,
  typing state, groups, and files are encrypted.
- **No forward secrecy.** The passphrase-derived key is static for a session
  and reused across connections (with a fresh salt per connection). Rotate the
  passphrase to invalidate old captures.
- **No identity authentication beyond the passphrase.** Peer UUIDs are stable
  but self-asserted; use the shared passphrase on untrusted LANs.
- **Local data is not encrypted at rest.** SQLite, thumbnails, and downloaded
  files are protected by the OS account and full-disk encryption
  (FileVault / BitLocker). The passphrase itself is stored in the local
  settings table in plain text.
- **No telemetry, no accounts, no internet calls.** The app only opens local
  sockets.
- Files are sanitised on receive (directory components stripped, path
  traversal rejected) and writes go through a `.part` file with SHA-256
  verification before the final rename.

---

## Local data

Tauri's app data directory is used for identity and history:

| Platform | Location |
| -------- | -------- |
| macOS | `~/Library/Application Support/com.lanchat.desktop/` |
| Windows | `%APPDATA%\com.lanchat.desktop\` |

Contents:

```
identity.json        # peer id, display name, avatar color, device name
lanchat.sqlite3      # peers, conversations, groups, messages, files, outbox, settings
lanchat.sqlite3-wal  # WAL (present while running)
thumbs/              # received/generated JPEG thumbnails
outbox/             # bytes written for paste/drag attachments before sending
```

Downloads are saved to the user's Downloads folder under `LAN Chat/`
(`~/Downloads/LAN Chat` on macOS, `%USERPROFILE%\Downloads\LAN Chat` on
Windows). Incoming files are written as `<name>.part` until complete.

SQLite tables: `peers`, `conversations`, `groups`, `group_members`, `messages`,
`files`, `outbox`, `settings`. Full history is available only in the main
window; the bubble intentionally shows only the latest message.

To start fresh, quit the app and remove the app data directory. This deletes
the identity, so other peers will see a new device.

---

## Tests

Core tests cover the protocol, discovery, crypto, transfer helpers, and
connection bookkeeping:

```bash
cargo test -p lanchat-core
```

Whole workspace:

```bash
cargo test --workspace
cargo check --workspace
```

Frontend type-check and production build:

```bash
npm run build
```

A few things worth knowing when testing by hand:

- Two instances on one Mac share UDP port 47823; the second one falls back to
  an ephemeral `disc_port` and unicast replies handle discovery. Use a second
  machine for the real path.
- The bubble ignores the auto-hide timer while the reply field is focused or
  contains text, and while the document is hidden. It also respects DND and
  per-chat mute.
- The global hotkey is `CmdOrCtrl+Shift+Space`. If another app has already
  registered it, registration fails silently and only the tray/menu bar remains
  available; check the log for `global hotkey unavailable`.
- On macOS, the Dock icon disappears when only the bubble is visible and
  returns when the main window opens.

---

## Protocol documentation

The normative wire protocol is in [docs/PROTOCOL.md](docs/PROTOCOL.md):

- envelope and frame validation rules
- JSON schemas and examples for `hello`, `hello_ack`, `presence`, `message`,
  `ack`, `typing`, `group_create`, `group_update`, `file_offer`, `file_chunk`,
  `file_ack`, `sealed`, `error`
- encryption handshake and proof derivation
- group fan-out and convergence
- chunking, resume, and offline queue semantics
- discovery announcement and connection policy
- limits and versioning

The Rust types in `core/src/protocol/` are the source of truth. If you change a
frame, update this document and any example in it; keep changes additive within
protocol version 1.

---

## Troubleshooting

**Peers never appear**

- Confirm both devices are on the same subnet and that client isolation is off.
- Allow the app in the OS firewall (Windows: Private profile).
- On macOS, check Local Network permission and
  `tccutil reset LocalNetwork com.lanchat.desktop` after changing signatures.
- Try `LANCHAT_LOG=debug npm run tauri dev` and look for `mDNS unavailable` or
  `discovery failed`.
- If mDNS is filtered, the UDP beacon should still work. A custom discovery
  port must match on both ends, or be left at `47823`.

**Connected but messages fail**

- A passphrase mismatch makes the handshake fail; one side must configure the
  same passphrase, or both must leave it empty.
- The standard text limit is 16 000 characters.

**Files won't transfer**

- The receiver's max file size setting may be lower than the file.
- Video thumbnails require `ffmpeg` on `PATH`; transfers still work without
  thumbnails.
- If a transfer was interrupted, leave the app running or restart it: `.part`
  files and transfer rows persist, and the sender re-offers on reconnect.

**Bubble doesn't appear**

- Enable it in Settings → Bubble.
- DND and per-chat mute suppress it.
- It hides while the main window is focused if
  `pause_bubble_when_app_focused` is on.
- On macOS, check that the app is allowed to show windows over fullscreen and
  that the hotkey isn't captured by another app.

**App won't quit**

- Closing the main window hides it instead of exiting. Use the tray/menu bar →
  **Quit LAN Chat** (or the platform's Quit menu item). `Exit` shuts the node
  down cleanly.

---

## License

MIT. See [LICENSE](LICENSE) for the full text and the `license` field in
`Cargo.toml`.
