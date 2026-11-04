# Pocket Mods

A Windows mod manager for Cyberpunk 2077 that sources mods from Nexus Mods. Browsing, installing, configuring and sharing mods all happen in one window, and the Nexus website opens only when Nexus requires it.

It uses the Pocket design language (handheld plastic, program keys, label bands, LCD inlays).

## Status

Built milestone by milestone from [docs/build-prompt.md](docs/build-prompt.md). Each milestone has a note in [docs/milestones](docs/milestones) with what to test and what is not verified yet. Assumptions are in [docs/assumptions.md](docs/assumptions.md).

## Run

```
npm install
npm run tauri dev      # the desktop app
npm run dev            # the UI alone in a browser, with demo data
```

Windows 10/11 x64 is the target. Rust (stable) and Node 20+ are needed to build.

## Repo layout

- `docs/build-prompt.md`: the product, design, tech and Nexus rules for the build.
- `design/reference/pocket.css`: the Pocket stylesheet from Collegiapp, the reference for tokens and component names.
- `src/styles/pocket-os.css`: the standalone Pocket stylesheet this app uses, with bundled fonts.
- `src/`: the React UI. It holds no Nexus or file logic.
- `crates/pocket-core`: Nexus client, cache, install and Cyberpunk logic in Rust.
- `src-tauri`: the Tauri 2 shell that exposes the core to the UI.
