# Assumptions

Decisions made at the start, so the build could go ahead without questions.

1. **Class names come from the Pocket reference.** `design/reference/pocket.css` is the source of truth, so the app uses its classes (`.btn`, `.nv`, `.panel`/`.ph`, `.display`, `.fbox`, `.chip`, `.seg`, `.cchip`, `.row`/`.grp`, `.deck`, `.mtop`/`.winkeys`) and not the spec's shorthand (`.k`, `.pk`, `.mod`, `.lcd` …). The gallery has a table mapping one to the other. Parts the reference lacks (slide switch `.sw`, program-key LED `.led` and foot progress `.prog`, LCD block level `.d-lvl`, mod row `.row.modrow`) are added in its style.
2. **The reference's look scoping (`.bset-b8`, `.fset-f3`) is dropped.** This app has one look, so B8 keys and F3 modules are the default. The Standard look (L8) and the CRT layer (C2 warp) are not included.
3. **Night is `html.night`**, as the spec says, and the same rule also answers to `data-scheme="dark"`, the reference's switch. Both are set together.
4. **Tokens the spec does not list** (radii, shadows, motion, `--accent-soft`, `--warn-ink`, `--on-bad`) are taken from the reference's `:root` and dark blocks.
5. **No blur.** The reference blurs behind dialogs (`.ov`); the spec says no blur over text, so the scrim is solid.
6. **Front end: React 19 + TypeScript + Vite.** Tauri 2 shell. The UI talks to Rust only through `src/lib/tauri.ts`; outside Tauri the same calls go to an in-memory demo backend, so the UI runs in a plain browser.
7. **Core logic is a separate Rust crate** (`crates/pocket-core`) with no Tauri dependency, so it can be tested on its own and ported to Linux later.
8. **Single branch, one PR.** Each milestone is one or more commits on the session's `animal/` branch, and the PR description tracks the milestones.
9. **Installer: NSIS, per-user**, with the WebView2 bootstrapper embedded (`embedBootstrapper`).
10. **App identity**: product name "Pocket Mods", identifier `app.pocketmods.desktop`. Sent to Nexus as `Application-Name: Pocket Mods`.
