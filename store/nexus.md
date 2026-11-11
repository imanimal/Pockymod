# Nexus Mods

Upload on the Cyberpunk 2077 game page. Software goes through the same form as a mod.

## Form settings

- **Name:** Pockymod
- **Category:** Utilities
- **Version:** the release's version, for example `1.0.0-alpha.3`
- **Language:** English
- **Main image:** `images/cover.png`, then the screenshots from `docs/screens/` (browse, install, health, collection,
  library)
- **Requirements:** none
- **Mirrors:** `https://github.com/imanimal/Pockymod/releases/latest`
- **AI tags:** Nexus asks on every upload since 3 November 2026. Pick what's true for the app's code and for this
  page's text and images. "AI Assisted" also needs a list on the page of the tools used and what they made.

## Brief overview

```
A mod manager for Cyberpunk 2077 on Windows. Browse Nexus mods in the app and press Install, and the mod's requirements come with it. It can filter by mods updated since your game patch, check a collection before you install it, and pick up mods Vortex already installed.
```

## Description (BBCode)

```
Pockymod is a mod manager for Cyberpunk 2077 on Windows. You browse Nexus mods inside it and press Install, and whatever the mod needs comes along. For Mod Settings that's redscript, RED4ext and ArchiveXL.

It's an alpha, so expect rough edges. Bug reports help a lot, in the Bugs tab here or on GitHub.

[size=4][b]What it does[/b][/size]
[list]
[*]Installs a mod with its requirements. Premium accounts download straight away. On a free account the app shows each mod's Nexus page, you press Mod Manager Download, and it moves on to the next one.
[*]Has filters Nexus doesn't, like "updated since your game patch".
[*]Health checks your frameworks against the game version. After a patch it says what will break, like an old Cyber Engine Tweaks or a missing requirement, and fixes it with one key.
[*]Before you install a collection, it shows how many mods you're missing, how many clicks it takes on a free account, which mods Nexus removed and which game version it was made for.
[*]Picks up the mods you already have. Vortex's come in with their profiles and stay in Vortex's format. Files you put in by hand are set aside instead of deleted, and Ctrl+Z undoes any change.
[*]Works with a mouse, the keyboard or a controller.
[/list]
If you make mods, the Create view shows how yours are doing and publishes new versions to Nexus.

[size=4][b]Installing[/b][/size]
Download the main file with Manual download, unzip it and run the setup exe inside. It installs for your Windows user, so it doesn't need admin rights.

The installer isn't signed yet, so Windows may say it protected your PC. Click More info, then Run anyway.

On first start, paste your Nexus API key. You'll find it on nexusmods.com under your avatar, Site preferences, API keys. You can browse without one.

[size=4][b]Updates[/b][/size]
Pockymod checks its GitHub releases and updates itself while you're away from it. You can turn that off in Settings, Background. New versions go up here too.

[size=4][b]Privacy[/b][/size]
There's no telemetry or analytics. Your key is kept in Windows Credential Manager and only goes to Nexus. The app talks to Nexus, to GitHub for its own updates, and to the sites a mod's description links to when you open a link preview.

[size=4][b]Source[/b][/size]
Pockymod is open source under the MIT license: [url=https://github.com/imanimal/Pockymod]github.com/imanimal/Pockymod[/url]

You need Windows 10 or 11 (64-bit) and a Nexus Mods account.
```

When Nexus registers the app and Sign in with Nexus is on, change the API key paragraph to:

```
On first start, press Sign in with Nexus and approve it in your browser. You can browse without signing in.
```

## The file

- **File:** `Pockymod-<version>-setup.zip` from the GitHub release
- **Name:** Pockymod installer
- **Category:** Main file
- **Description:** `Unzip it and run the setup exe. After that Pockymod updates itself.`
- **Mod Manager Download:** turn it off if the form offers it. Vortex would try to install the app as a game mod.
  Pockymod itself recognizes its own installer and skips it, so this only matters for other managers.
- Mark the older version's file as Old when you upload a new one.

## Permissions

The MIT license already allows copying, changing, forking and selling, so the page's permissions should say the same:

- Upload to other sites, modify and reupload, use the assets: allowed, with credit
- Use in files that are sold: allowed
- Forks and parallel versions: allowed
- AI training and tool use: your call (the license doesn't limit it)
- Donation Points: your call

Custom permission text:

```
Pockymod is MIT licensed. You can use, change and share it as the license says: https://github.com/imanimal/Pockymod/blob/main/LICENSE
```

## Registration email

Send this before the page goes up. The policy only tolerates personal API keys in apps that are still being tested;
a registered app gets a client ID for Sign in with Nexus, which goes in `CLIENT_ID` in
`crates/pocket-core/src/nexus/oauth.rs`. Attach `src-tauri/icons/icon.png` as the logo.

**To:** support@nexusmods.com
**Subject:** Registering Pockymod, a mod manager for Cyberpunk 2077

```
Hi,

I'd like to register Pockymod with the API so it can use Sign in with Nexus. It's a free, open-source mod manager for Cyberpunk 2077 on Windows. Right now each tester pastes their own personal API key.

Testing build (works with a personal API key): https://github.com/imanimal/Pockymod/releases/latest
Source (MIT): https://github.com/imanimal/Pockymod
Name: Pockymod
Short description: A mod manager for Cyberpunk 2077 on Windows. Browse, install and update Nexus mods and collections, with load order and conflict checks.
Logo: attached (512 px)
Sign-in: OAuth 2.0 authorization code with PKCE, as a public client with no secret. Redirect URI: http://127.0.0.1:41477/callback

Every request sends Application-Name: Pockymod and Application-Version. Free accounts download through the site's Mod Manager Download button one file at a time, and nothing skips that page. How the app follows the Acceptable Use Policy is written up here: https://github.com/imanimal/Pockymod/blob/main/docs/nexus/compliance.md

I'd also like to put the app on its own page in Cyberpunk 2077's Utilities category. Is that fine while the registration is open?

Thanks,
<your Nexus username>
```
