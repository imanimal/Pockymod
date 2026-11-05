# Pocket Mods

A Windows mod manager for Cyberpunk 2077 that sources mods from Nexus Mods. Browsing, installing, configuring and sharing mods all happen in one window, and the Nexus website opens only when Nexus requires it, inside the app.

It uses the Pocket design language (handheld plastic, program keys, label bands, LCD inlays).

## Status

All seven milestones in [docs/build-prompt.md](docs/build-prompt.md) are built. [docs/milestones.md](docs/milestones.md) has what each one covers, how to test it, and the list of things that couldn't be checked against the live Nexus API or a real Windows install yet.

Distribution: the app uses each person's own personal Nexus API key. Nexus says a public app must be registered with them (spec section 4), so register before publishing a release.

## Run it

Needs Node 22, Rust (stable) and, on Windows, the WebView2 runtime (Windows 10/11 have it).

```bash
npm ci
```

```bash
npm run tauri dev
```

The UI alone, against an in-memory stand-in for the core, runs in any browser. Any API key works there; one containing "premium" acts as a Premium account.

```bash
npm run dev
```

Build the Windows installer (NSIS, with the WebView2 bootstrapper):

```bash
npx tauri build
```

The installer lands in `target/release/bundle/nsis/`. It isn't code-signed yet, so SmartScreen warns on first run.

## Test it

```bash
npm run typecheck
```

```bash
npm test
```

```bash
cargo test -p pocket-core
```

The live Nexus check needs your key in `NEXUS_API_KEY` and writes the real response shape to `target/nexus-probe.json`:

```bash
cargo test -p pocket-core --test live_probe -- --ignored --nocapture
```

## Repo layout

- `crates/pocket-core/`: everything that isn't UI: the Nexus client, key storage, nxm:// links, archives, install layout, hard-link deploy, the queue, the library database, game, framework and REDmod handling, health, collections, mod.io. Portable apart from the Windows-only parts behind `cfg(windows)`.
- `src-tauri/`: the Tauri 2 shell. It forwards commands to the core, shows Nexus pages in an in-app window, catches nxm:// links and keeps one instance.
- `src/`: the React and TypeScript front end. `src/styles/pocket-os.css` is the standalone Pocket stylesheet.
- `docs/build-prompt.md`: the product, design, tech and Nexus rules for the build.
- `docs/milestones.md`: per-milestone notes, tests and unverified items.
- `design/reference/pocket.css`: the Pocket stylesheet from Collegiapp, the reference for tokens and component names.
