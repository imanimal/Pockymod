# Milestone 7: Polish

## Summary

- **Installer**: NSIS, per-user install (no admin rights), WebView2 bootstrapper embedded for machines without the runtime (`src-tauri/tauri.conf.json`, `bundle.windows`). How to build it and what a public release still needs (code signing for SmartScreen, Nexus app registration, signed self-updates) is in `docs/release.md`.
- **CI** (`.github/workflows/ci.yml`): on Linux, the type check, front-end tests, core tests and a Tauri build; on Windows, the core tests (so hard links, backups, rollback and the registry code run on NTFS) and the unsigned installer, uploaded as the `pocket-mods-windows-installer` artifact.
- **Update checks**:
  - mods: milestone 5's checks (start-up and every six hours, badges in the rail and Library);
  - the app itself: Settings, About, "Check for updates" asks GitHub for the latest release and says whether it is newer. It does not download anything.
- **mod.io, read-only experiment** (Settings, Experiments, off by default): with a mod.io API key (checked with mod.io, kept in Windows Credential Manager as `mod.io-api-key.Pocket Mods`), Browse gets a "Nexus Mods / mod.io" switch and lists Cyberpunk 2077 mods on mod.io with downloads and size. Installing stays with the game's own Mods menu, as the spec plans.

## Run and test

```
npm run tauri dev
npx tauri build            # Windows: the installer
```

1. Run the installer from CI's artifact on a clean Windows 10 or 11 machine. It installs without admin rights; SmartScreen warns because it is unsigned.
2. Settings, About: "Check for updates" says no release is published yet (or shows the newer one).
3. Settings, Experiments: turn on mod.io, paste a mod.io API key. Browse shows the source switch; mod.io lists mods or says Cyberpunk 2077 is not available for that key.

## Checked here

- mod.io's API answers 401 without a key (so a key is required, as the spec says); the client parses mod.io's documented mod shape.
- The GitHub release check answers 404 while no release is published, and the app says so.
- Version comparison for app updates; 74 core tests in all.

## Not verified

- The installer has not been run (CI builds it; see the PR checks).
- mod.io with a real key: the game ID lookup by name, the mod fields, and whether Cyberpunk 2077 is open to third-party keys.
- WebView2 bootstrapper behaviour on a machine without the runtime.
