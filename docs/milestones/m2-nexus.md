# Milestone 2: Nexus connection

## Summary

- `pocket-core::nexus::NexusClient`: one HTTP client for v2 GraphQL and v1 REST. Every request sends `Application-Name: Pocket Mods`, `Application-Version` and a `PocketMods/<version>` user agent; keyed requests add `APIKEY`.
- Rate limits (`nexus/ratelimit.rs`): reads `x-rl-daily-*` and `x-rl-hourly-*` from every response and a 429's `Retry-After`. Below 1,000 daily or 50 hourly requests it spaces requests out (amber); at zero or after a 429 it refuses to send until the reset (red). Nothing is retried automatically.
- Key storage (`credentials.rs`): the key goes to the OS credential store through `keyring` (Windows Credential Manager on Windows, entry `nexus-api-key.Pocket Mods`). `Secret` never prints its value. Settings checks a key with Nexus before saving it, and a rejected key is not stored.
- Rate-limit LCD at the foot of the rail and in Settings: requests left this hour (or today while the daily budget lasts), blocks for the level, green, amber or red.
- GraphQL query layer (`nexus/graphql.rs`) and record types (`nexus/types.rs`) built from the live schema, plus v1 calls (`nexus/rest.rs`) and nxm:// parsing (`nexus/nxm.rs`), ready for milestones 3 and 4.
- Live test: `crates/pocket-core/examples/nexus_probe.rs`. It uses the app's own client and queries, logs the real `mods()` shape and checks every claim in section 4's unconfirmed list. Its output from the build machine is in `docs/nexus/live-probe.md` and `docs/nexus/live-sample.json`.

## Differences from section 4 (trust the live API)

From the anonymous run on 2026-11-04:

- **Tags are returnable.** `Mod.tags` gives `{ id, name }` for each tag. The spec said a mod record has no returnable tags.
- **Dates are Unix seconds as strings.** `updatedAt: [{ value: "1792627200", op: GTE }]` works; an ISO date breaks the server's search parser with a 400.
- **Several values in one filter field are ANDed.** `categoryName: [A, B]` matches nothing; OR needs `filter: [{ op: OR, categoryName: [A, B] }]`.
- **`modId` filters need `gameId`** (3333), not `gameDomainName`, and this does not carry into nested filters. Batch lookups use `legacyModsByDomain`.
- **Facets come back only for the fields asked for** in `facets` (`categoryName`, `tag` here), in `facetsData` as value → count.
- **No `x-rl-*` headers on anonymous v2 responses.** The LCD shows "---" until a keyed request reports a budget.
- **The archive file list exists**: `modFileContents` gives paths and sizes per file, but the index is partial (ArchiveXL's newest main file has no entries).
- **Requirement links can be read inside a `mods()` page**, so framework chips need no extra request per card.
- Confirmed as reported: search on name and description, category, tag, author, date, download, endorsement, size, adult and update filters, AND/OR, 10 sort keys, facet counts, offset/count paging, requirement links, no mod-type field, no game-version field, `collectionsV2` game-version filter.
- `fileSize` (on mods) and `size` (on files) are in kilobytes; files also have `sizeInBytes` as a string.

## Run

```
cargo test -p pocket-core
cargo run -p pocket-core --example nexus_probe                  # anonymous
cargo run -p pocket-core --example nexus_probe -- --stored-key  # with the key saved by the app
npm run tauri dev                                                # Settings, Nexus account
```

## Test

1. In Settings, paste a personal API key and press Check and save. Your name and Free or Premium show. A wrong key shows "Nexus did not accept the API key." and is not saved.
2. Open Windows Credential Manager, Windows Credentials: `nexus-api-key.Pocket Mods` is there. Remove key deletes it.
3. Restart the app: the key is still there and the account shows after the first request.
4. After any keyed request the rail LCD shows a number of requests left. (Needs Nexus to send `x-rl-*` headers on that request.)
5. Run the probe with `--stored-key` and commit the new `docs/nexus/live-probe.md`.

## Checked here

- `cargo test -p pocket-core` (rate-limit levels and gating, reset formats, 429 handling, secret redaction, record parsing from live JSON, filter building, nxm parsing).
- The probe against the live v2 API, anonymously.
- Settings and the LCD in a browser with the demo backend.

## Not verified (needs a local run with a key)

- Windows Credential Manager storage (built with `keyring`'s `windows-native` store; only compiled on Linux).
- All v1 REST calls: `users/validate.json`, `download_link.json`, `mods/updated.json`, endorse, abstain, track, untrack. Their paths and fields follow the public v1 docs and Vortex, and are marked UNVERIFIED in `nexus/rest.rs`.
- The `x-rl-*` header names and reset format on keyed responses (v1 and v2), and whether v2 counts against the same budget.
- The 403 Premium message used to tell "needs Premium" apart from other refusals.
- Whether v2 accepts the `APIKEY` header (it is sent when a key is set).
