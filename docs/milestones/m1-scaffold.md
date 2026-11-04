# Milestone 1: scaffold and design system

## Summary

- Tauri 2 app (`src-tauri`) with a frameless window: the app draws its own title bar (`.mtop`) with the three window keys.
- Rust workspace with `crates/pocket-core`, where all non-UI logic goes.
- `src/styles/pocket-os.css`: a standalone Pocket stylesheet built from `design/reference/pocket.css` plus the spec's tokens. No dependency on Collegiapp's `themes.js` or `core.css`.
- Fonts bundled in `src/styles/fonts` (Space Grotesk, Space Mono, Doto; latin and latin-ext; OFL licences alongside). Nothing loads from the network.
- Cream (default) and Night themes, following the system setting unless one is picked (title-bar key or Settings).
- 9×9 pixel icons (`src/ui/icons.ts`) drawn with `shape-rendering: crispEdges` at 9, 18, 27, 36, 54 px.
- Component gallery (rail: Gallery) showing every part in every variant and state.

## File tree (new)

```
index.html                 theme applied before first paint
vite.config.ts, tsconfig.json, package.json
Cargo.toml                 workspace
crates/pocket-core/        core crate
src-tauri/                 Tauri app, tauri.conf.json, capabilities, icons
src/main.tsx, src/App.tsx  shell: title bar, program-key rail, views
src/lib/theme.ts           Cream / Night / system
src/lib/tauri.ts           invoke wrapper with a demo backend outside Tauri
src/ui/icons.ts, Icon.tsx  pixel icons
src/ui/pocket.tsx          React wrappers that emit Pocket classes
src/screens/Gallery.tsx    component gallery
src/screens/Settings.tsx   theme and motion
src/styles/pocket-os.css   the stylesheet
design/brand/icon.svg      app icon source
```

## Run

```
npm install
npm run tauri dev      # the desktop app (Windows: needs the WebView2 runtime, present on Windows 10/11)
npm run dev            # the UI alone in a browser at http://localhost:1420
```

## Test

1. Open Gallery from the bottom of the rail. Check each module: keys (rest, hover, pressed, latched, focus, disabled), program keys (latched, LED, badge, foot progress), bands (graphite, orange, red), LCDs (green, amber, red, level blocks, small, big), search field, chips with counts, segmented control, switches, pills, tags, rows, window frame and deck, icons, tokens, type.
2. Press and release any key: it travels 3 px and springs back.
3. Toggle the theme from the sun/moon key in the title bar, and in Settings pick System, Cream, Night. With System, change the Windows colour mode and the app follows.
4. Turn Motion off in Settings: keys and LEDs stop animating.
5. In the desktop app, drag the title bar to move the window; double-click it to maximise; use the three window keys.

## Checked here

- `npm run build` (type check and bundle) passes.
- `cargo build -p pocket-mods` passes on Linux with WebKitGTK.
- Gallery screenshots in Cream and Night from a headless browser.

## Not verified

- The app has not been run on Windows. The frameless title bar, window keys and dragging are untested in WebView2.
- Segoe UI Variable and Cascadia Mono fallbacks were not seen (the bundled fonts loaded first).
- Side-by-side comparison with Collegiapp was not possible; the family resemblance is by the reference stylesheet only.
