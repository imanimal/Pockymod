# How Pockymod follows Nexus's API policy

Each point of the [API Acceptable Use Policy](https://help.nexusmods.com/article/114-api-acceptable-use-policy)
(last updated 1 December 2020, read again 7 November 2026), and where the app meets it.

| The policy | In Pockymod |
|---|---|
| A public app registers with Nexus, from a testing build | In testing. Registration is asked for with a testing build before any public release ([release.md](../release.md)). |
| Personal keys only for testing or personal use | Each tester pastes their own key; none is shipped in the app. Single sign-on is built and turns on when Nexus issues the app's slug (`SSO_SLUG` in `src/lib/sso.ts`); the key field then stays only as an advanced option. |
| `Application-Name` and `Application-Version` on every request, consistent across versions | Every request, keyed or not, sends `Application-Name: Pockymod` and the build's version, and the user agent starts with the same name (`crates/pocket-core/src/nexus/client.rs`). Uploads send the same headers (`creator.rs`). |
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
- Premium links carry `utm_source=pockymod&utm_medium=app` and a `utm_campaign` naming the place they were opened
  from (`install`, `downloads`, `collection`, `settings`), so Nexus can count the sign-ups the app sends. The app
  itself records nothing.
- Every download is a real Nexus download: Premium through `download_link.json`, free accounts through the site's own
  button. Both count toward the mod's downloads and its author's Donation Points the same way a Vortex download does.

## Registering

What the policy asks for, and what goes with the request to support@nexusmods.com:

- A testing build that works with a personal API key: the installer from a GitHub release.
- The source: github.com/imanimal/pocket-mods (MIT).
- Name: Pockymod. Logo: `src-tauri/icons/icon.png` (512 px, a black P on a yellow plate, which reads on dark
  backgrounds), or `design/brand/icon-1024.png` for a larger copy.
- Short description: "A mod manager for Cyberpunk 2077 on Windows. Browse, install and update Nexus mods and
  collections, with load order and conflict checks."

## Linking back to Nexus

- Every mod and collection has **View on Nexus**, plus keys for the page in the app's pane or the user's browser.
- Endorsing and tracking are offered in the app (a star on each mod not endorsed yet, Endorse all, a gentle line after
  playing), so authors get endorsements from players who never open the page.
- Descriptions link to their Nexus pages; requirements open their own mod.

## Nexus's AI rules

Nexus's [announcement of 3 November 2026](https://www.nexusmods.com/news/15654) and its [AI policy page](https://help.nexusmods.com/article/180-ai-policy-changes-2026)
(updated 5 November 2026) are about mods uploaded to Nexus. They do not mention apps or the API. The app follows their
direction anyway:

- **Labels.** A card and a mod's page show Nexus's own tags, **AI-Generated Content**, **AI Media** and **AI Assisted**,
  first in the row, with a hover tip saying what Nexus means by each (`src/lib/ai.ts`). The names were read from
  Nexus's live tag list for Cyberpunk 2077 (see [live-probe.md](live-probe.md)); "NPC Behaviours (AI)" is the game's
  own AI and is not shown as one.
- **Choice.** Settings, AI content (and the Filters key in Browse) leaves out AI-generated mods, or every AI-tagged mod.
  It is off by default, since Nexus does not ban AI mods and the user decides. The setting is sent to Nexus as a tag
  filter, so counts and pages are right. A tag a user asks for by name is never also left out.
- **No AI in the app.** No Nexus data, mod file or description is sent to any AI service, and nothing is used to train
  one. Nexus's new "AI Training/Tool Usage" permission is about exactly that.
- **Publishing.** The publish form reminds authors to update the page's AI statement when a version adds AI-made
  parts, and links to the mod's edit page. A new mod page cannot be made through the app at all (Nexus's form is
  opened for it), so the disclosure steps there are never skipped.
- **Tags are the authors' own.** The app cannot tell an untagged AI mod from any other, and says so in Settings; a
  mislabelled mod is reported on its Nexus page.
- **How the app was made.** Settings, About, "How it was made" says the app was built with an AI coding assistant, and
  that it has no AI in it.

## Not done, on purpose

- Publishing collections to Nexus: off, because Nexus has not said other apps may.
- Adult content is hidden from searches by default.
- No telemetry and no analytics: the app talks to Nexus, to GitHub for its own updates, and to the sites a mod's
  description links to (for the link previews the user opens).
