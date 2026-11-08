# Building Pockymod

You need Windows 10 or 11 (x64), Rust (stable) and Node 20 or newer.

```
npm install
npm run tauri dev      # the desktop app
npm run dev            # the UI alone in a browser, with demo data
```

[release.md](release.md) covers the installer. On Linux, Tauri also needs the WebKitGTK development packages.

## Tests

```
npm test                                               # UI tests
cargo test -p pocket-core                              # core tests
cargo test -p pocket-core --test live -- --ignored     # live checks against the Nexus API (no key needed)
cargo run -p pocket-core --example nexus_probe         # log the live mods() response shape
```

## Where things are

- `src/`: the React UI. It holds no Nexus or file logic.
- `crates/pocket-core`: Nexus client, cache, install and Cyberpunk logic in Rust.
- `src-tauri`: the Tauri 2 shell that exposes the core to the UI.
- `src/styles/pocket-os.css`: the Pocket stylesheet the app uses, with bundled fonts. `src/styles/themes/` holds the Standard and Redline themes, `fx.css` the CRT layer.
- `design/reference/pocket.css`: the Pocket stylesheet from Collegiapp, the reference for tokens and component names.
- `marketplace/`: the themes and extensions listed in the app's Add-ons screen. [marketplace.md](marketplace.md) explains how to add yours.
- `docs/milestones`: what each milestone built, how to test it, and what isn't verified yet. Assumptions are in [assumptions.md](assumptions.md).
- `docs/nexus`: the live probe of the Nexus API, and [how the app follows Nexus's API policy](nexus/compliance.md).
