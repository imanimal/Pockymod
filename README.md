# Pockymod

A Windows mod manager for Cyberpunk 2077 that sources mods from Nexus Mods. Browsing, installing, configuring and sharing mods happen in one window, and every mod and collection links back to its Nexus page. How it follows Nexus's API policy is in [docs/nexus/compliance.md](docs/nexus/compliance.md).

It uses the Pocket design language (handheld plastic, program keys, label bands, LCD inlays).

## Status

Built milestone by milestone from [docs/build-prompt.md](docs/build-prompt.md). Each milestone has a note in [docs/milestones](docs/milestones) with what to test and what is not verified yet. Assumptions are in [docs/assumptions.md](docs/assumptions.md).

## What it does

- **Browse** Nexus Mods in the app: search, categories and tags with counts, sort, and Cyberpunk-aware filters Nexus does not have (required framework, mod type, updated since your patch), plus hide, ignore and downloaded. Mods Nexus tags as made with AI carry a label, and Settings can leave them out. A mod opens in a drawer or as a full page like its Nexus page; pictures open full window with zoom.
- **Install** with one key, or Quick install from any card: requirements come along, Premium accounts need no clicks, and free accounts click "Mod Manager Download" on the Nexus page (in their own browser by default, or in the app's browser pane) and the queue moves on by itself. A card for a mod you have opens everything you can do to it.
- **Nexus pages** open in your own browser (your sign-in, saved passwords and extensions) or, if you choose, in a pane docked beside the app that can be resized, put over the whole view, hidden or popped out.
- **Deploy** with hard links from a mod library on the game's drive; every change is reversible and no mod's file is ever deleted by another.
- **Health**: missing or outdated frameworks after a game patch, broken deploys, REDmod state, each with a fix key, and Fix all. **Profiles** and the **REDmod load order**, with the REDmod deploy step.
- **Collections**: browse Nexus collections, open one as a page (picture, description, every mod against what you have) or check it quickly before installing, and share your own as a file that pins mod IDs, file IDs, hashes and dependencies.
- **Your own mods first**: mods Vortex installed or you copied in by hand are tracked as ordinary mods (Vortex's stay in Vortex's format), Vortex's profiles come along with them, and a requirement you already have is never installed over yours.
- **Undo** every change to your mods (Ctrl+Z, Ctrl+Y), and put back removed mods for 14 days. **Play** with or **without mods** from the launch deck.
- **For authors**: your uploaded mods with their numbers and files, and new versions published from the app. [docs/creators.md](docs/creators.md).
- **Themes**: Redline (the default, styled after the game's HUD), Standard (dark or light) and Pocket, an optional CRT layer, and themes you make in the app or import as a file. [docs/themes.md](docs/themes.md) explains how to make one.

## Add-ons

Themes and extensions from the [marketplace](marketplace) (a folder in this repository) or from a file, in the app's **Add-ons** screen. Extensions add Browse shelves, Library group rules, tools and launch options in the Play menu, console commands, links and sandboxed panels, and each shows what it can do before it is installed.

- [docs/marketplace.md](docs/marketplace.md): install, update, remove, and share your own.
- [docs/extensions.md](docs/extensions.md): write an extension, step by step.

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
- `src/styles/pocket-os.css`: the standalone Pocket stylesheet this app uses, with bundled fonts. `src/styles/themes/` holds the Standard and Redline themes, `fx.css` the CRT layer.
- `src/`: the React UI. It holds no Nexus or file logic.
- `crates/pocket-core`: Nexus client, cache, install and Cyberpunk logic in Rust.
- `src-tauri`: the Tauri 2 shell that exposes the core to the UI.
- `docs/milestones`: what each milestone built, how to test it, and what is not verified.
- `docs/nexus`: the live probe of the Nexus API.
