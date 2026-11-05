# Milestones

Notes for each milestone in [build-prompt.md](build-prompt.md) §7: what was built, where it lives, how to test it, and what couldn't be verified. Anything marked UNVERIFIED in a code comment is listed again here.

## Assumptions

- **Design source.** `design/reference/pocket.css` is authoritative for class names and component shapes, so the app uses its names (`.btn`, `.nv`, `.panel` / `.ph`, `.display`, `.fbox`, `.duepill`, `.cchip`, `.row`) rather than the spec's shorthand (`.k`, `.pk`, `.mod` / `.band`, `.lcd`, `.srch`, `.pill`, `.cc`, `.frow`). The map between the two is at the top of `src/styles/pocket-os.css`. No `pocket-os.css` was attached, so the palette tokens (Cream P9, Night P11) come from the spec, and the parts the reference expects from its missing theme and core files are defined there too.
- **Additions to Pocket.** The reference has no slide switch, program-key LED or progress foot, or orange/red band classes. These are added as `.sw`, `.nv .led`, `.nv .pg`, `.ph.hot` and `.ph.bad`, and marked as additions in the stylesheet.
- **Night theme.** Night is `html[data-scheme="dark"]`, as in the reference, not a `.night` class.
- **Front end.** React 19 with TypeScript and Vite. Fonts come from `@fontsource` packages and are bundled into the build, so nothing loads from the network.
- **Core outside the shell.** The orchestration (queue driver, install pipeline, profiles, health gathering) lives in `pocket-core` as `App`, not in the Tauri layer, so it is testable without a window and portable later.
- **Nexus pages.** They open in a second in-app window (a WebView2 window), not embedded in the main one. Tauri's multi-webview API is still marked unstable.
- **Conflicts.** When two mods write the same file, the mod installed or enabled later wins by default (switchable in Settings), and the loser is restored when the winner goes.
- **Mod store location.** It sits on the game's drive: `%LOCALAPPDATA%\PocketMods\store` if that's the same drive, otherwise `<drive>:\PocketMods`.
- **Patch time.** "Compatible with my game version" compares a mod's last update with the installed exe's build time (its PE time stamp, falling back to the file's modified time). There is no table of patch dates.

## Milestone 1: scaffold and design system

**Built**
- A Cargo workspace (`crates/pocket-core`, `src-tauri`), a Tauri 2 app and the React front end.
- `src/styles/pocket-os.css`: the standalone Pocket stylesheet, plus `src/styles/app.css` for layout.
- Cream and Night themes. The app follows Windows until the person picks one; the title-bar key flips it.
- A custom window frame: title bar with window keys, program-key rail, and a key deck per screen.
- 9×9 pixel icons in `src/components/Icon.tsx`.
- React wrappers for every component in `src/components/pocket.tsx`.
- The component gallery (Settings, then Component gallery, or `?view=gallery`).

**Test**
- Run `npm run dev` and open `/?view=gallery`. Check keys (hover, press travel, focus ring), program keys with LED, progress foot and count, modules with graphite, orange and red bands, LCDs (normal, amber, red, big, block meter), the search field, chips, segmented control, switch, pills, tags, list rows and the window frame.
- Flip the theme from the title bar.
- `npm test`. `src/test/screens.test.tsx` renders every screen and fails if one shows more than one orange key or band, or puts dot-matrix digits outside an LCD.

**Not verified:** how it looks in WebView2 on Windows. The screenshots were taken in Chromium.

## Milestone 2: Nexus connection

**Built**
- `pocket-core/src/secrets.rs`: the key goes in Windows Credential Manager as a generic credential, never in files or logs.
- `nexus/mod.rs`: every request sends `apikey`, `Application-Name` and `Application-Version`. Every response updates the rate limit (`nexus/ratelimit.rs`). Requests are spaced out when the budget is low and refused locally while throttled. A 429 is never retried blindly.
- The rate-limit LCD in the title bar: green normally, amber when low, red when throttled. Settings shows today's and this hour's budget.
- `Client::probe` and `tests/live_probe.rs`: validate the key, introspect the v2 `Mod`, `ModsFilter` and `ModsSort` types, run one small `mods()` query for cyberpunk2077, and write it all to `target/nexus-probe.json`. Settings has a Live API check key that does the same into the app's data folder.

**Test**
- Run `cargo test -p pocket-core nexus`. A local HTTP server checks the headers, rate-limit parsing, the 429 lock-out, 401 wording, free-account 403, and the nxm key in the query string.
- With a real key, run `cargo test -p pocket-core --test live_probe -- --ignored --nocapture`, then compare `requestedButMissing`, `modTypeFields` and `sampleResponse` with §4's unconfirmed list.

**Not verified:** the live `mods()` response shape. No key was available here, so the probe has never run against Nexus. Section 4's unconfirmed list therefore still stands unchanged, and the GraphQL names in `nexus/queries.rs` are marked UNVERIFIED.

## Milestone 3: browser

**Built**
- Search, category and tag filters with counts, sort (relevance, downloads, endorsements, updated, created, size, ascending or descending), and ranges for updated date, downloads, endorsements and size.
- One `mods()` call per page, asking only for card fields.
- Cyberpunk filters derived locally in `classify.rs`: required framework, mod type, and works with my game version.
  - Sources: words in the name and summary, requirement links, and the archive's file list (Nexus's content preview in the detail view, the real archive after download). The results go into a local index.
  - Their counts are for the results loaded so far, and the filter rail says so.
- Local hide and downloaded / not downloaded filters.
- The mod detail module: description (safe BBCode subset, rendered without HTML), requirements, files, endorse, track, and the Nexus page in-app.
- Caching with TTLs keyed on filter, sort and offset (`library.rs` cache, `app.rs` TTLs). Description, files and requirements are fetched only in the detail view.

**Test**
- `cargo test -p pocket-core` covers classify, queries and `app::local_filters_and_facets`.
- In `npm run dev`, enter any key in Settings, then try each filter in Browse.

**Not verified:** GraphQL filter operators (`MATCHES`, `WILDCARD`, `GTE`), the sort key names, the `facetsData` shape (three shapes are parsed), the `fileSize` unit (assumed KB), the `legacyModsByDomain` / `modRequirements` names, and the content-preview JSON shape. The "manual download only" flag is a heuristic, since v1 has no such field.

## Milestone 4: install core

**Built**
- nxm:// handling:
  - Parsing in `nxm.rs`.
  - Per-user registration under HKCU in `nxm_handler.rs`. It takes the handler on first run only if nobody holds it. Settings can take it back from Vortex or Mod Organizer, and hand it back.
  - Single instance through `tauri-plugin-single-instance`: a second launch passes its link to the running window.
- The queue (`queue.rs`, driven by `App::pump`):
  - Free accounts see the click cost up front. The next mod's Nexus page opens in-app, the nxm:// navigation is caught, and the queue moves on by itself.
  - Premium runs fully automatically, including the frameworks a mod needs.
  - Manual-only mods always go through their page.
  - The queue survives restarts.
- Extraction of .zip, .7z and .rar with bundled libraries (`archive.rs`). Paths that try to escape the folder are refused.
- `layout.rs` puts files where they belong by mod type. FOMOD installers and colliding variants are refused with an explanation rather than guessed.
- Hard-link deploy (`deploy.rs`):
  - Every deploy writes a manifest.
  - Files that were already there are backed up and restored.
  - Conflicts hand off to the other mod when the winner is removed.
  - A failed deploy rolls back completely.
  - Undeploy never deletes a file another mod or the person put there since.
  - The copy fallback runs only when switched on.
- Permission errors under Program Files explain the fix (`error.rs`).

**Test**
- `cargo test -p pocket-core` covers deploy, layout, archive, queue and the `app::` install tests. On Windows, CI's `cargo test -p pocket-core` runs the same tests on NTFS, plus the registry and Credential Manager tests.
- On Windows: install a mod from Browse with a free account and check that the page opens in-app and the click is caught. Install a REDmod. Remove a mod and check the game folder is as before.

**Not verified:**
- That WebView2 reports the site's nxm:// navigation to `on_navigation`. If it doesn't, the registered handler still delivers the link through the single-instance path.
- The `nmm=1` page URL for the mod-manager download step.
- Running against real Nexus downloads.

## Milestone 5: Cyberpunk layer

**Built**
- `game.rs` detects installs:
  - Steam: registry plus `libraryfolders.vdf` and `appmanifest`.
  - GOG: registry.
  - Epic: launcher manifests, best effort.
  - It reads the exe's version and build time, and flags installs under Program Files.
- `frameworks.rs` detects frameworks and their versions: CET, RED4ext, redscript, ArchiveXL, TweakXL, Codeware, cybercmd.
- `redmod.rs` lists REDmods and runs `redMod.exe deploy` in load order. It explains failures in plain language, and runs automatically after a REDmod install (switchable).
- The Health panel (`health.rs`):
  - Missing frameworks, including the ones those need.
  - Frameworks installed before the last game update.
  - Script mods older than the build.
  - Updates, conflicts, files changed outside the app, and REDmod state.
  - Each module's band turns red when broken, and one orange fix key sits on top.
- Profiles: switching profile undeploys and deploys the difference. Load order is kept per profile.

**Test**
- `cargo test -p pocket-core` covers game, frameworks, redmod, health and the `app::` profile and REDmod tests.
- On Windows: check Settings, Game shows the right folder and version, and that Health lists your frameworks.

**Not verified:**
- The framework Nexus mod IDs and marker file paths.
- The GOG product ID and Epic manifest fields.
- That Cyberpunk's exe has a real PE time stamp.
- The `redMod.exe deploy -root= -mod=` argument shape.

## Milestone 6: collections

**Built**
- Browse Nexus collections, with an option for matching your game version.
- A pre-install check: game version, what's missing, how many clicks, and the size. It reads the revision's `collection.json` (Vortex's ICollection shape) before anything downloads.
- Install puts the missing Nexus mods in the queue. Off-site mods are listed for doing by hand.
- Local collections: pick mods from the library and export a `.json` that pins mod ID, file ID, MD5 and size, plus `requires` rules (from requirements and frameworks) and the REDmod load order.
- Exported or opened files can be checked and installed.
- Publishing to Nexus sits behind an experiment flag and only explains that it's unconfirmed.

**Test**
- `cargo test -p pocket-core collections` and `app::export_a_local_collection`.
- In the app: Collections, Mine, pick mods, Export, then Open a collection file with the result.

**Not verified:**
- The `collectionsV2` and `collectionRevision` names and fields.
- That `downloadLink` gives a v1 path returning `download_links`, and that the download is an archive holding `collection.json`.
- Whether free accounts can download the manifest.

## Milestone 7: polish

**Built**
- An NSIS installer, per-user install with no admin rights, embedding the WebView2 bootstrapper (`tauri.conf.json`). CI builds it unsigned.
- Update checks:
  - Installed mods: one `updated` call, then files only for the mods that changed. It runs once per launch, and from Library.
  - The app: GitHub's latest release, from Settings and once per launch.
- Endorse and track on the detail view.
- The read-only mod.io experiment: switch it on in Settings, add a key and the game ID, then use the deck key in Browse.

**Test**
- Run `npx tauri build` on Windows and install the result.
- Check Library, then Check for updates.
- Switch on the mod.io experiment.

**Not verified:**
- The mod.io fields for Cyberpunk, and whether its key and game ID are open to third parties (spec §6 says to ask mod.io).
- Code signing: still a release task, so SmartScreen will warn.
