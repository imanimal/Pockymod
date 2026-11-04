# Pocket Mods

A Windows mod manager for Cyberpunk 2077 that sources mods from Nexus Mods. Browsing, installing, configuring and sharing mods all happen in one window, and the Nexus website opens only when Nexus requires it.

It uses the Pocket design language (handheld plastic, program keys, label bands, LCD inlays).

## Status

Built milestone by milestone from [docs/build-prompt.md](docs/build-prompt.md). Each milestone has a note in [docs/milestones](docs/milestones) with what to test and what is not verified yet. Assumptions are in [docs/assumptions.md](docs/assumptions.md).

## What it does

- **Browse** Nexus Mods in the app: search, categories and tags with counts, sort, and Cyberpunk-aware filters Nexus does not have (required framework, mod type, updated since your patch), plus hide, ignore and downloaded.
- **Install** with one key: requirements come along, the cost in clicks is shown first, free accounts click "Mod Manager Download" in an in-app Nexus window and the queue moves on by itself, Premium accounts need no clicks.
- **Deploy** with hard links from a mod library on the game's drive; every change is reversible and no mod's file is ever deleted by another.
- **Health**: missing or outdated frameworks after a game patch, broken deploys, REDmod state, each with one fix key. **Profiles** and the **REDmod load order**, with the REDmod deploy step.
- **Collections**: browse Nexus collections with pre-install checks, and share your own as a file that pins mod IDs, file IDs, hashes and dependencies.

## Run

```
npm install
npm run tauri dev      # the desktop app
npm run dev            # the UI alone in a browser, with demo data
```

Windows 10/11 x64 is the target. Rust (stable) and Node 20+ are needed to build; `docs/release.md` covers the installer. On Linux, Tauri also needs the WebKitGTK development packages.

```
cargo test -p pocket-core                              # core tests
cargo test -p pocket-core --test live -- --ignored     # live checks against the Nexus API (no key needed)
cargo run -p pocket-core --example nexus_probe         # log the live mods() response shape
```

## Repo layout

- `docs/build-prompt.md`: the product, design, tech and Nexus rules for the build.
- `design/reference/pocket.css`: the Pocket stylesheet from Collegiapp, the reference for tokens and component names.
- `src/styles/pocket-os.css`: the standalone Pocket stylesheet this app uses, with bundled fonts.
- `src/`: the React UI. It holds no Nexus or file logic.
- `crates/pocket-core`: Nexus client, cache, install and Cyberpunk logic in Rust.
- `src-tauri`: the Tauri 2 shell that exposes the core to the UI.
- `docs/milestones`: what each milestone built, how to test it, and what is not verified.
- `docs/nexus`: the live probe of the Nexus API.
