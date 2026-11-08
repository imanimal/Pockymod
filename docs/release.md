# Releasing

## Build the installer

On Windows 10/11 x64 with Rust (stable), Node 20+ and the MSVC build tools:

```
npm ci
npx tauri build
```

The NSIS installer lands in `target\release\bundle\nsis\`. It installs per user (no admin rights) and embeds the WebView2 bootstrapper, so machines without the WebView2 runtime get it during setup. CI builds the same installer on every pull request (artifact `pocket-mods-windows-installer`).

## Before a public release

- **Code signing.** The installer and `pocket-mods.exe` are unsigned, so Windows SmartScreen warns ("Windows protected your PC") until the publisher has a reputation. Sign both with an OV or EV code-signing certificate (`bundle.windows.certificateThumbprint` or a `signCommand` in `tauri.conf.json`).
- **Nexus app registration.** A personal API key is fine for testing and personal use. A public app must be registered with Nexus support, with a testing build, to get a client ID for Sign in with Nexus (`CLIENT_ID` in `crates/pocket-core/src/nexus/oauth.rs`), and Nexus may refuse. Never ship a personal key in the app. Until registered, each user pastes their own key (Settings, Nexus account).
- **Self-updates.** Push a tag like `v0.2.0`: `.github/workflows/release.yml` builds the installer and publishes it with
  `<installer>.sha256`. Running copies find it within six hours, download it quietly and install it silently when the
  user is away (the window closed, or on quit), then start again in the tray (Settings, Background: install when I'm
  away, download and I'll restart, or off). Bump `version` in `Cargo.toml` and `tauri.conf.json` first. Sign the
  installer before a public release, so Windows vouches for it.
- **mod.io.** The read-only mod.io browser is an experiment that needs a mod.io API key. Ask mod.io (through its developer contact) whether Cyberpunk 2077's catalogue may be used by a third-party app before turning it on by default.
