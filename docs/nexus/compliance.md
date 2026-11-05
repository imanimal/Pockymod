# How Pockymod follows Nexus's API policy

Each point of the [API Acceptable Use Policy](https://help.nexusmods.com/article/114-api-acceptable-use-policy)
(last updated 1 December 2020, read 5 November 2026), and where the app meets it.

| The policy | In Pockymod |
|---|---|
| A public app registers with Nexus, from a testing build | In testing. Registration is asked for with a testing build before any public release ([release.md](../release.md)). |
| Personal keys only for testing or personal use | Each tester pastes their own key; none is shipped in the app. Single sign-on is built and turns on when Nexus issues the app's slug (`SSO_SLUG` in `src/lib/sso.ts`); the key field then stays only as an advanced option. |
| `Application-Name` and `Application-Version` on every request, consistent across versions | Every request, keyed or not, sends `Application-Name: Pockymod` and the build's version (`crates/pocket-core/src/nexus/client.rs`). |
| No fetching en masse to rehost | Nothing is fetched ahead or in bulk for display. Search pages, mod details and file lists are fetched when the user opens them and cached on their own machine (pages 10 minutes, details until the mod changes). Nothing is sent anywhere but Nexus. |
| No storing keys on your own server | There is no server. The key is kept in Windows Credential Manager on the user's PC and goes only to Nexus. |
| No using keys without the user starting the action | Keyed requests follow what the user does: opening the window, pressing Check now, installing, endorsing, tracking. Checking for mod updates every six hours (and Premium downloads ahead) is off until the user turns it on in Settings, Background. |
| No blank or impersonating request metadata | The app names itself as Pockymod only. |
| Excessive requests can get users rate limited | The app reads `x-rl-daily-*` and `x-rl-hourly-*` on every response, spaces requests out before the limit, stops at zero or on a 429 until the reset, and never retries blindly (`nexus/ratelimit.rs`). Filter counts come from one request, requirement links come inside the search request, and matching an existing Vortex setup runs four requests at a time, each cached. |
| Open source encouraged; source with the request helps | MIT licensed ([LICENSE](../../LICENSE)). The source goes with the registration request. |

## Free and Premium

- Free accounts download through the site's **Mod Manager Download** button, one file at a time, on the mod's own
  Nexus page (in the user's own browser by default, so Nexus's page, countdown and ads are what they see). Nothing
  skips or automates that step; the app only catches the nxm:// link the button produces.
- Direct links (`download_link.json` without an nxm key) are asked for only for Premium accounts.
- Before an install that would take a free account several clicks, one quiet line says what Premium would do instead,
  with a link to Nexus's Premium page (`src/ui/Premium.tsx`). It can be snoozed or turned off. After upgrading, the
  app notices on its own and the waiting files download without clicks.

## Linking back to Nexus

- Every mod and collection has **View on Nexus**, plus keys for the page in the app's pane or the user's browser.
- Endorsing and tracking are offered in the app (a star on each mod not endorsed yet, Endorse all, a gentle line after
  playing), so authors get endorsements from players who never open the page.
- Descriptions link to their Nexus pages; requirements open their own mod.

## Not done, on purpose

- Publishing collections to Nexus: off, because Nexus has not said other apps may.
- Adult content is hidden from searches by default.
- No telemetry and no analytics: the app talks to Nexus, to GitHub for its own updates, and to the sites a mod's
  description links to (for the link previews the user opens).
