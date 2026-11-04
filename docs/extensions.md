# Writing an extension

An extension adds things to Pocket Mods: shelves on the Browse front page, rules that sort mods into Library groups, programs and launch options in the Play menu, console commands, links, and panels (pages of their own). It is one JSON file. Most extensions need no code at all; a panel is the only part that runs any.

Nothing in an extension does anything until the user has seen what it adds and what it can do, and said yes.

- [Quick start](#quick-start)
- [The manifest](#the-manifest)
- [Contributions](#contributions): [shelves](#shelves), [group rules](#group-rules), [tools](#tools), [launch options](#launch-options), [command packs](#command-packs), [links](#links), [panels](#panels)
- [Permissions](#permissions)
- [The panel API](#the-panel-api) and [a complete panel](#a-complete-panel)
- [Testing your extension](#testing-your-extension)
- [Updates and approval](#updates-and-approval)
- [Limits](#limits)
- [How panels are kept apart from the app](#how-panels-are-kept-apart-from-the-app)

Themes are a separate kind of add-on: see [themes.md](themes.md). To share either kind, see [marketplace.md](marketplace.md).

## Quick start

1. In Pocket Mods, open **Add-ons**, then **Make your own**, then **Save a starter extension**. You get `my-extension.pmext.json`:

   ```json
   {
     "format": "pocket-mods.extension",
     "version": 1,
     "id": "my-extension",
     "name": "My extension",
     "release": "1.0.0",
     "author": "You",
     "description": "A shelf of mods and a launch option, to start from.",
     "contributes": {
       "shelves": [{ "id": "photo", "title": "Photo mode", "hint": "Poses and cameras", "query": { "tags": ["Poses"], "sort": "endorsements" } }],
       "launchOptions": [{ "id": "skip", "name": "Skip the start screen", "args": ["-skipStartScreen"] }],
       "links": [{ "id": "wiki", "title": "Cyberpunk modding wiki", "url": "https://wiki.redmodding.org/cyberpunk-2077-modding" }]
     }
   }
   ```

2. Open it in any text editor. Change `id` and `name` to your own, and add what you need from [Contributions](#contributions).
3. In **Add-ons**, use **Install from file** and pick the file. If something is wrong, the app says which field (for example `contributes.shelves[0].query.sort: must be one of …`). Fix it and install again.
4. Read the check screen, then **Install and turn on**. Your shelf is on the Browse front page; the launch option is in the menu beside **Play**.
5. Each time you change the file, raise `release` (1.0.1, 1.1.0 …) and install it again.

The four extensions in [marketplace/extensions](../marketplace/extensions) are worked examples of every contribution.

## The manifest

| Field | Needed | What it is |
| --- | --- | --- |
| `format` | yes | Always `"pocket-mods.extension"`. |
| `version` | yes | The format version. Always `1`. |
| `id` | yes | Lowercase letters, digits, dots and dashes, up to 64 characters: `photo-shelves`. It is the file name too, so pick one and keep it. |
| `name` | yes | Up to 60 characters, shown everywhere. |
| `release` | yes | Your extension's own version, like `1.0.0` ([semver](https://semver.org)). Raise it for every change. |
| `author` | no | Your name or handle as you want it shown. |
| `description` | no | Up to 400 characters: what it is for. |
| `homepage` | no | An `https://` page about it. |
| `icon` | no | A built-in icon name (`star`, `play`, `library`, `chip` …) or your own 9×9 pixel icon: nine strings of nine `.` and `#`. |
| `tags` | no | Up to eight short words for search. |
| `permissions` | no | What its panels may ask the app for. See [Permissions](#permissions). |
| `contributes` | yes | What it adds. At least one entry. |

Unknown fields are refused, so a typo shows up straight away instead of being ignored.

A 9×9 icon, a camera:

```json
"icon": [".........", "...###...", "#########", "#.......#", "#..###..#", "#..#.#..#", "#..###..#", "#.......#", "#########"]
```

## Contributions

Every list entry except group rules has an `id` (same rules as the extension id), unique in its list. Lists hold up to 50 entries.

### Shelves

A saved search shown as a row of mods on the Browse front page, after the first two of the app's own shelves. **See all** opens it as a full search.

```json
"shelves": [
  { "id": "poses", "title": "Poses", "hint": "Pose packs for photo mode", "icon": "user", "query": { "tags": ["Poses"], "sort": "endorsements" } }
]
```

| Field | What it is |
| --- | --- |
| `title` | Up to 50 characters. |
| `hint` | Up to 90 characters, shown after the title with your extension's name. |
| `icon` | A built-in icon name. |
| `query` | The search. Any of the fields below. |

Query fields (they are the Browse filters):

| Field | Example | Meaning |
| --- | --- | --- |
| `text` | `"photo mode"` | Words to search for. |
| `categories` | `["Visuals and Graphics"]` | Nexus categories, as Browse shows them. |
| `tags` | `["Poses"]` | Nexus tags. A mod needs all of them. |
| `excludeTags` | `["AI Assisted"]` | Leave out mods with any of these. |
| `sort` | `"endorsements"` | One of `endorsements`, `downloads`, `updated`, `created`, `relevance` (with `text`), `size`, `name`, `random`. |
| `ascending` | `false` | Reverse the order. |
| `frameworks` | `["cet", "archivexl"]` | Mods that need these: `cet`, `red4ext`, `redscript`, `archivexl`, `tweakxl`, `codeware`, `cybercmd`. |
| `modType` | `"redmod"` | `redmod`, `archive`, `script` or `framework`. |
| `endorsementsMin` | `100` | At least this many endorsements. |
| `downloadsMax` | `25000` | At most this many downloads (for hidden gems). |
| `createdWithinDays` | `30` | First released in the last N days. |

The game version filter in Browse applies on top, as for every shelf. A shelf with no results is not shown.

### Group rules

Rules that put mods into a Library group. They apply when the user presses **Sort into groups** in Library, which shows every move first. Mods in a group the user made themselves are never moved.

```json
"groupRules": [
  { "group": "Photo mode", "match": { "name": ["photo mode", "poses"], "category": ["Animations"] } },
  { "group": "psiberx", "match": { "author": ["psiberx"] } }
]
```

`group` is the group's name (up to 40 characters); it is made if it does not exist. A mod matches when **any one** of the `match` lists matches it:

| Match | Matches when |
| --- | --- |
| `name` | the mod's name contains one of these (any case). |
| `category` | its Nexus category is one of these (any case). |
| `author` | its author is one of these (any case). Mods from a file have no author. |
| `modType` | its type is one of `redmod`, `archive`, `script`, `framework`, `mixed`, `unknown`. |
| `frameworks` | it needs one of these frameworks. |
| `mods` | its Nexus mod id is one of these numbers. |

The first matching rule wins, across all extensions that are on.

### Tools

Programs started from the Play menu, such as WolvenKit or a save editor. A tool is either inside the game folder (`path`), or a program the user picks once (`pick`).

```json
"tools": [
  { "id": "redlauncher", "name": "REDlauncher", "description": "The game's own launcher.", "path": "REDprelauncher.exe" },
  { "id": "wolvenkit", "name": "WolvenKit", "pick": { "title": "Find WolvenKit.exe", "file": "WolvenKit.exe" },
    "download": "https://github.com/WolvenKit/WolvenKit/releases" }
]
```

| Field | What it is |
| --- | --- |
| `name`, `description` | Shown in the Play menu and the check screen. |
| `path` | A `.exe` inside the game folder, with forward slashes: `tools/redmod/bin/redMod.exe`. No drive, no leading slash, no `..`. |
| `pick` | The user chooses the program: on install, from Installed, or the first time they start it. `file` is the file name it must have (`WolvenKit.exe`); `title` is the file picker's title. |
| `args` | Arguments, passed to the program as they are (there is no shell). `{game}` becomes the game folder. No quotes or line breaks; up to 24. |
| `download` | An `https://` page where the program can be had. Shown as **Get it**. |

The app starts a tool only if it is listed by an extension that is on and approved, by checking the file on disk each time. It never starts anything that is not an `.exe` file.

### Launch options

Named sets of game arguments, in the menu beside **Play**. Starting the game this way runs the same checks as Play (Health first, the REDmod deploy when REDmods changed) and adds `-modded` when REDmods are on.

```json
"launchOptions": [
  { "id": "skip-start", "name": "Skip the start screen", "description": "Straight to the main menu.", "args": ["-skipStartScreen"] }
]
```

`args` holds 1 to 16 arguments, each without spaces, quotes or braces.

### Command packs

Console commands for Cyber Engine Tweaks' console, to copy. All packs are in the Play menu under **Console commands**. A pack that lists mod ids in `mods` also shows in those mods' **Console commands** in Browse.

```json
"commandPacks": [
  {
    "id": "vehicles", "title": "Cars and bikes", "mods": [107],
    "description": "Adds the vehicle to the ones you can call.",
    "commands": [
      { "label": "Rayfield Caliburn", "command": "Game.GetVehicleSystem():EnablePlayerVehicle(\"Vehicle.v_sport1_rayfield_caliburn_player\", true, false)" }
    ]
  }
]
```

Each command is one line of up to 300 characters, with an optional `label` saying what it does. Up to 200 commands a pack. The app never runs them; the user copies and pastes them in game.

### Links

Pages the user may want, opened in their browser. Only `https://` addresses.

```json
"links": [{ "id": "wiki", "title": "Cyberpunk 2077 modding wiki", "url": "https://wiki.redmodding.org/cyberpunk-2077-modding" }]
```

Links show in Add-ons, Installed, under the extension's details. The check screen shows each link's address before install.

### Panels

A page of its own, opened as a tab in Add-ons. A panel is HTML with inline `<style>` and `<script>`, and it runs in a sandbox (see [How panels are kept apart](#how-panels-are-kept-apart-from-the-app)).

```json
"panels": [{ "id": "checklist", "title": "Checklist", "icon": "check", "html": "<h2>Before you play</h2> …" }]
```

`html` is the inside of the page's `<body>`, up to 512 KB. The app puts it in a page that already has:

- the app's colours as CSS custom properties (`--bg`, `--surface`, `--surface-2`, `--line`, `--text`, `--text-2`, `--text-3`, `--accent`, `--on-accent`, `--chrome-bg`, `--chrome-fg`, `--disp-bg`, `--disp-fg`, `--ok`, `--warn`, `--bad`, `--acc2`), kept up to date when the theme changes;
- plain styles for headings, buttons (`class="primary"` and `class="ghost"`), inputs, tables, `.dim`, `.row`, `.card` and `.lcd`;
- the `pocket` object, described below.

Because JSON strings cannot hold line breaks, write the panel as an `.html` file and put it in the manifest with a small script, for example:

```js
// node make.mjs
import { readFileSync, writeFileSync } from "node:fs";
const ext = JSON.parse(readFileSync("my-extension.json", "utf8"));
ext.contributes.panels[0].html = readFileSync("panel.html", "utf8");
writeFileSync("my-extension.json", JSON.stringify(ext, null, 2));
```

## Permissions

A panel can ask the app for things only through the [panel API](#the-panel-api), and only for the permissions its extension lists. The check screen shows each one in these words before install.

| Permission | Shown to the user as | Lets a panel |
| --- | --- | --- |
| `library.read` | See your mod list | Read the installed mods: names, versions, groups, sizes, on or off. |
| `library.toggle` | Turn your mods on and off | Turn a mod on or off in the active profile. The app shows a note each time. |
| `nexus.search` | Search Nexus Mods | Search Nexus through the app (up to 30 searches a minute). |
| `queue.add` | Offer mods to download | Ask to add a mod to the download queue. The app asks the user every time. |
| `toast` | Show short notes | Show a short note in the corner, with the extension's name. |
| `nexus.open` | Open Nexus Mods pages | Open a mod's page in the app's Nexus window. |
| `storage` | Keep its own notes | Keep up to 256 KB of its own data. Removed with the extension. |

Ask for as few as you need. Shelves, group rules, tools, launch options, command packs and links need no permissions; the check screen lists them all anyway.

## The panel API

In a panel's script, `pocket` is ready to use. Every call returns a promise. A call the extension has no permission for fails with a message saying which permission is missing.

```js
const info = await pocket.ready;        // wait for this before anything else
```

| Call | Permission | Returns |
| --- | --- | --- |
| `pocket.ready` | none | `{ extension: { id, name, release }, panel: { id, title }, permissions, theme: { scheme, tokens } }` |
| `pocket.library.list()` | `library.read` | A list of `{ id, modId, name, version, enabled, group, category, modType, frameworks, files, source, size }`. `id` is the install's id; `modId` is the Nexus id or `null` for mods from a file; `size` is the archive in bytes or `null`. |
| `pocket.library.setEnabled(id, on)` | `library.toggle` | `{ ok: true }` or `{ ok: false, reason }` (for example when files would clash with another mod). |
| `pocket.nexus.search({ text, sort, count })` | `nexus.search` | `{ total, mods: [{ modId, name, summary, author, category, version, downloads, endorsements, updatedAt, modType, downloaded }] }`. `sort` is `relevance`, `endorsements`, `downloads`, `updated` or `created`; `count` is up to 30. |
| `pocket.nexus.open(modId)` | `nexus.open` | Nothing. The mod's page opens in the Nexus window. |
| `pocket.queue.add(modId, fileId)` | `queue.add` | `{ queued: true }` if the user said yes, `{ queued: false }` if not. `fileId` may be left out for the mod's main file. Its required mods come along. |
| `pocket.toast(text)` | `toast` | Nothing. Up to 200 characters. |
| `pocket.storage.get(key)` | `storage` | The value saved under `key`, or `null`. |
| `pocket.storage.set(key, value)` | `storage` | Nothing. `value` is anything JSON can hold. |
| `pocket.storage.remove(key)` | `storage` | Nothing. |
| `pocket.storage.keys()` | `storage` | The saved keys. |
| `pocket.on("theme", fn)` | none | Calls `fn({ scheme, tokens })` when the user changes the theme. The colours are applied for you; this is for anything you draw yourself. |

Under the hood these are messages to the app: the page sends `{ pocket: 1, id, method, params }` with `window.parent.postMessage` and gets back `{ pocket: 1, id, result }` or `{ pocket: 1, id, error }`. `pocket.call(method, params)` sends one directly. The methods are the ones above: `hello`, `library.list`, `library.setEnabled`, `nexus.search`, `nexus.open`, `queue.add`, `toast`, `storage.get`, `storage.set`, `storage.remove`, `storage.keys`.

## A complete panel

This is the **Checklist** panel from the Library report extension in the marketplace. Save it as `checklist.pmext.json`, install it from the file, and open **Add-ons**, **Checklist**.

```json
{
  "format": "pocket-mods.extension",
  "version": 1,
  "id": "my-checklist",
  "name": "My checklist",
  "release": "1.0.0",
  "author": "You",
  "description": "A list of things to check before playing.",
  "icon": "check",
  "permissions": ["storage", "toast"],
  "contributes": {
    "panels": [
      {
        "id": "checklist",
        "title": "Checklist",
        "icon": "check",
        "html": "<h2>Before you play</h2>\n<p class=\"dim\">Your own list, kept by this extension.</p>\n<ul id=\"list\" style=\"list-style: none; padding: 0; display: flex; flex-direction: column; gap: .4em;\"></ul>\n<div class=\"row\">\n  <input id=\"text\" placeholder=\"Add a step\" style=\"flex: 1;\">\n  <button class=\"primary\" id=\"add\">Add</button>\n</div>\n<script>\n  const $ = (id) => document.getElementById(id);\n  let items = [];\n  const save = () => pocket.storage.set(\"items\", items).catch((e) => pocket.toast(e.message));\n\n  function draw() {\n    $(\"list\").replaceChildren(...items.map((item, i) => {\n      const li = document.createElement(\"li\");\n      li.className = \"card row\";\n      const box = document.createElement(\"input\");\n      box.type = \"checkbox\";\n      box.checked = item.done;\n      box.onchange = () => { item.done = box.checked; save(); };\n      const label = document.createElement(\"span\");\n      label.textContent = item.text;\n      label.style.flex = \"1\";\n      const remove = document.createElement(\"button\");\n      remove.className = \"ghost\";\n      remove.textContent = \"Remove\";\n      remove.onclick = () => { items.splice(i, 1); save(); draw(); };\n      li.append(box, label, remove);\n      return li;\n    }));\n  }\n\n  $(\"add\").onclick = () => {\n    const text = $(\"text\").value.trim();\n    if (!text) return;\n    items.push({ text, done: false });\n    $(\"text\").value = \"\";\n    save();\n    draw();\n  };\n\n  pocket.ready.then(async () => {\n    items = (await pocket.storage.get(\"items\")) ?? [{ text: \"Back up your saves\", done: false }];\n    draw();\n  });\n</script>"
      }
    ]
  }
}
```

The same `html`, written out:

```html
<h2>Before you play</h2>
<p class="dim">Your own list, kept by this extension.</p>
<ul id="list" style="list-style: none; padding: 0; display: flex; flex-direction: column; gap: .4em;"></ul>
<div class="row">
  <input id="text" placeholder="Add a step" style="flex: 1;">
  <button class="primary" id="add">Add</button>
</div>
<script>
  const $ = (id) => document.getElementById(id);
  let items = [];
  const save = () => pocket.storage.set("items", items).catch((e) => pocket.toast(e.message));

  function draw() {
    $("list").replaceChildren(...items.map((item, i) => {
      const li = document.createElement("li");
      li.className = "card row";
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = item.done;
      box.onchange = () => { item.done = box.checked; save(); };
      const label = document.createElement("span");
      label.textContent = item.text;   // textContent, never innerHTML, for anything typed or read
      label.style.flex = "1";
      const remove = document.createElement("button");
      remove.className = "ghost";
      remove.textContent = "Remove";
      remove.onclick = () => { items.splice(i, 1); save(); draw(); };
      li.append(box, label, remove);
      return li;
    }));
  }

  $("add").onclick = () => {
    const text = $("text").value.trim();
    if (!text) return;
    items.push({ text, done: false });
    $("text").value = "";
    save();
    draw();
  };

  pocket.ready.then(async () => {
    items = (await pocket.storage.get("items")) ?? [{ text: "Back up your saves", done: false }];
    draw();
  });
</script>
```

For a panel that reads and changes the library, see **Mods by size** in [library-report.json](../marketplace/extensions/library-report.json): it lists mods with `pocket.library.list()`, turns them on and off with `pocket.library.setEnabled()`, and opens their Nexus pages with `pocket.nexus.open()`.

## Testing your extension

- **Install from file** (Add-ons) checks the file and shows each problem with the field it is in. Fix and install again; installing the same `id` replaces it.
- Check a file the way the marketplace does, from a copy of this repository: `node scripts/validate-marketplace.mjs --file my-extension.json`.
- The browser version of the app works too: `npm run dev`, open the address it prints, and use **Install from file** there. Panels, shelves, group rules and command packs work on its demo data; tools and launch options need the desktop app.
- A panel's script errors show in the developer tools of the app's window (in a development build, right-click, Inspect, then pick the panel's frame in the console).
- In a panel, `alert`, `confirm` and `prompt` do nothing (the sandbox blocks them). Use `pocket.toast` or draw your own message.

## Updates and approval

On install, the user sees everything the extension adds, its tools and their paths, its launch options and their arguments, its links and their addresses, and its permissions in plain words. They approve all of it, or nothing is installed.

When an update arrives (from the marketplace or a file with the same `id`):

- If it needs nothing new (no new permission, and tools, launch options and links the same), it is applied straight away.
- If it needs anything new, it waits: the extension does nothing until the user reviews it in Add-ons, Installed, where the new items are marked **New**.

Turning an extension off takes away all it adds at once. Removing it deletes its file and its stored notes; mods it sorted stay in their groups.

## Limits

| What | Limit |
| --- | --- |
| Manifest | 1 MB |
| A panel's `html` | 512 KB |
| Entries in each contribution list | 50 |
| Commands in a pack | 200, one line of up to 300 characters each |
| Tool arguments | 24 |
| Launch option arguments | 16, no spaces |
| Panel storage | 256 KB for the extension |
| Panel calls | 40 a second; `nexus.search` 30 a minute; `toast` 3 every 5 seconds; `library.setEnabled` 20 every 10 seconds; one download question at a time |

A panel cannot load anything from the internet: no `fetch`, no remote images, fonts or scripts, no frames. Put images in the page as `data:` addresses.

## How panels are kept apart from the app

A panel is code written by someone else, so it runs with no access to the app beyond the permissions the user approved. In the desktop app:

1. **Its own page and policy.** The panel's page is served by a `pmext` address the app registers (`src-tauri/src/commands/extensions.rs`), only for an extension that is on and approved. The page comes with its own Content Security Policy: `default-src 'none'`, inline script and style only, images and fonts from `data:` only, no forms, and `sandbox allow-scripts`. So it can make no network requests at all, including to the app's internal addresses. The app's own policy (`src-tauri/tauri.conf.json`) allows frames only from YouTube's privacy domain and `pmext`.
2. **An opaque origin.** The frame has `sandbox="allow-scripts"` and not `allow-same-origin`. Its origin is `null`: it cannot read the app's page, storage, cookies or variables, cannot open windows or pop-ups, cannot show dialogs, cannot navigate the app, and cannot submit forms.
3. **No Tauri IPC.** Tauri 2 puts its IPC script (`window.__TAURI_INTERNALS__`) in the window's main frame only, never in frames. Every IPC request must also carry a key that only that script has, and an `Origin` that Tauri can match against the app's capabilities. A sandboxed frame sends `Origin: null`, has no key, and the app's only capability (`src-tauri/capabilities/default.json`) is for the main window's own page. On top of that, the panel's policy blocks the requests before they are sent.
4. **One narrow door.** The app listens for messages from that frame's window only (checked by identity, since the origin is `null`), and answers only the methods in [the panel API](#the-panel-api). Each method needs its permission, arguments are checked, calls are rate limited, and downloads always ask the user.

Tools, launch options and links are not run by panels at all. The app starts them only when the user picks them, and the Rust side starts only what the manifest on disk lists for an extension that is on and approved for its installed release.

What was checked: in the browser build, a panel's frame reports origin `null`, `window.parent.document` and `localStorage` throw a security error, `window.__TAURI_INTERNALS__` is undefined, `fetch` is refused by the policy, and calls without a permission are refused. The Tauri source (2.12) shows its IPC scripts are added for the main frame only and that requests without the invoke key or with an origin that is not a URL are refused. What was not checked: the same in WebView2 on Windows.
