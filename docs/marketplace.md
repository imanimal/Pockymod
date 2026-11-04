# The marketplace

The marketplace is where Pocket Mods users get and share **themes** (the app's colours) and **extensions** (shelves, group rules, tools, launch options, console commands, links and panels). There is no server: the marketplace is the [`marketplace/`](../marketplace) folder of this repository on GitHub. The app reads it from there, and anything merged into it shows in every copy of the app within an hour.

- [Getting add-ons](#getting-add-ons): install, update, remove
- [Sharing yours](#sharing-yours): from the app, or with a pull request
- [What the review checks](#what-the-review-checks)
- [Updating something you shared](#updating-something-you-shared)
- [The index](#the-index)

To write an add-on, see [extensions.md](extensions.md) and [themes.md](themes.md).

## Getting add-ons

Open **Add-ons** in the rail.

### Install

1. Pick the **Themes** or **Extensions** tab. Search by name, author, words in the description or tags, and sort by recently updated, name or author.
2. Press **Install** on a card.
   - A **theme** installs straight away. Pick it in **Settings**, under **Look**.
   - An **extension** first shows what it adds (every shelf, rule, tool with its path, launch option with its arguments, link with its address, and panel) and what it can do (its permissions, in plain words). Nothing is installed unless you press **Install and turn on**.
3. If an extension has a tool you install yourself (like WolvenKit), choose its program on that screen, or later in **Installed**, or the first time you start it from the Play menu.

Where things show up:

| Add-on part | Where |
| --- | --- |
| Shelves | The Browse front page, after the first two shelves. |
| Group rules | Library, **Sort into groups**. It shows every move before it makes it. |
| Tools and launch options | The menu beside **Play** (the arrow key). |
| Console commands | The menu beside **Play**, and a mod's **Console commands** in Browse if the pack lists that mod. |
| Links | Add-ons, Installed, the extension's **Details**. |
| Panels | Tabs at the end of the Add-ons tab row. |

### Install from a file

Someone sent you a `.json` file, or you made one: **Add-ons**, **Install from file**. The app works out whether it is a theme or an extension. If the file has a problem, nothing is installed and the app lists each problem with the field to fix.

### Update

When the marketplace has a newer version of something you installed, its card says **Update**, and **Installed** shows an **Updates** list with **Update all**.

- An update that needs nothing new is applied at once.
- An extension update that needs something new (a permission, a tool, a launch option or a link it did not have) shows the check screen again with the new items marked **New**. Until you allow it, the old release stays installed but does nothing. You can also press **Not now** and allow it later from **Installed** (**Review**).

### Turn off or remove

In **Installed**:

- The switch on an extension turns it off: everything it adds goes away until you turn it on again.
- **Remove** deletes it. For an extension, the notes it kept are deleted too; mods it sorted into groups stay where they are.
- **Save as file** saves a copy, for a backup or to send to someone.

### Offline

The app keeps the last copy of the marketplace it read. When GitHub cannot be reached, it shows that copy with a note saying so (for up to a week). Installed add-ons are on your PC and work offline.

## Sharing yours

Anyone can share a theme or extension. It is reviewed before it is added. There are two ways.

### From the app (easiest)

1. Install your add-on from its file and check that it works.
2. In **Add-ons**, **Installed**, press **Share** on it. The app checks it the way the review does and tells you what to fix, if anything.
3. **Save the file.** Keep it: you need it for updates.
4. **Open the form on GitHub.** Your browser opens a new issue in this repository with the form filled in: kind, name, and the package itself when it is small enough to fit in the address. You need a free GitHub account.
5. If the form's **Package** box is empty, drag the file you saved into it.
6. Add tags and a line on what it is for, tick the two boxes (you made it, and it holds no keys or passwords), and submit.

A maintainer runs the checks, may ask for changes in the issue, and then adds it. It shows in the app within an hour of being merged.

### With a pull request

If you know Git:

1. Fork this repository.
2. Add your file as `marketplace/themes/<id>.json` or `marketplace/extensions/<id>.json`. The file name is the add-on's `id`.
3. Add an entry to `marketplace/index.json`, in `themes` or `extensions` (see [The index](#the-index)). Use today's date for `updated`.
4. Run the check: `node scripts/validate-marketplace.mjs` (Node 20 or newer, no install needed). It must end with "The marketplace checks out."
5. Open a pull request. CI runs the same check.

## What the review checks

The automatic check (`scripts/validate-marketplace.mjs`, run by CI on every pull request) uses the same rules as the app:

- `index.json` is valid: format, ids that are unique across themes and extensions, versions like `1.2.0`, dates like `2026-11-04`, paths to files that exist, and every file in `themes/` and `extensions/` is listed.
- Each package is valid: the right `format` and `version`, its `id` and version match its index entry, and every field is one the app knows.
- Themes: every token is a CSS custom property (`--bg`) with one plain value, and nothing loads from the internet (no `url()` or `@import`).
- Extensions: every permission is a known one and matches the index entry, tools stay inside the game folder or are picked by the user, arguments have no quotes or line breaks, links are `https`, and panels are within the size limit.

A person then checks what a program cannot:

- It does what its description says, and its name is not misleading or someone else's.
- Each permission, tool, launch option and link is needed for what it does.
- A panel's script does nothing hidden (it is read in full), and it uses `textContent`, not `innerHTML`, for text it reads or is given.
- Console commands are real and labelled.
- It holds no keys, passwords, personal details, or links to paid or adult content, and the author has the right to share it.

## Updating something you shared

1. Change your file, keeping the same `id`.
2. Raise its version: `release` in the file (for themes, `release` if you use it), and `version` in the index entry, to the same value. Use a bigger number: `1.0.1` for a fix, `1.1.0` for something new.
3. Submit it the same way (**Share** in the app, and pick "Update" on the form; or a pull request changing both files).

Users see the update on the next marketplace check (at most an hour). For an extension, if the new release needs a permission, tool, launch option or link it did not have, every user is asked again before it does anything; otherwise it is applied for them.

To take something down, open an issue asking for it; the maintainers remove the entry and the file. Copies already installed keep working until their users remove them.

## The index

`marketplace/index.json` lists everything:

```json
{
  "format": "pocket-mods.marketplace",
  "version": 1,
  "updated": "2026-11-04",
  "themes": [
    {
      "id": "neon", "kind": "theme", "name": "Neon", "author": "Pocket Mods", "version": "1.0.0",
      "description": "Blue-black with hot pink keys and a cyan display. For late nights.",
      "tags": ["dark", "colourful"], "path": "themes/neon.json", "updated": "2026-11-04",
      "preview": { "bg": "#0e0f1a", "surface": "#161827", "accent": "#ff3d8b", "text": "#f1f0ff", "chrome": "#07070e", "display": "#38d3e4" }
    }
  ],
  "extensions": [
    {
      "id": "library-report", "kind": "extension", "name": "Library report", "author": "Pocket Mods", "version": "1.0.0",
      "description": "Two panels: your mods by size, with a switch for each, and a checklist of things to do before you play.",
      "tags": ["panel", "library"], "path": "extensions/library-report.json", "updated": "2026-11-04",
      "icon": "library", "permissions": ["library.read", "library.toggle", "nexus.open", "toast", "storage"], "adds": ["2 panels"]
    }
  ]
}
```

| Field | Needed | What it is |
| --- | --- | --- |
| `id` | yes | The package's `id`. Unique across themes and extensions. |
| `kind` | yes | `theme` or `extension`, matching the list it is in. |
| `name`, `author` | yes | As shown on the card, up to 60 characters each. |
| `version` | yes | The package's version (its `release`). |
| `description` | yes | Up to 200 characters for the card. |
| `tags` | no | Up to eight. |
| `path` | yes | The package file: `themes/<id>.json` or `extensions/<id>.json`. |
| `updated` | yes | The date of this version, `YYYY-MM-DD`. Cards sort by it. |
| `preview` | themes | Colours for the card's swatch (`bg`, `surface`, `accent`, `text`, `chrome`, `display`), so it draws without downloading the theme. |
| `icon` | no | The extension's icon (as in its manifest), for the card. |
| `screenshot` | no | An image in the marketplace folder (`extensions/shots/<id>.png`, up to 1 MB), shown on the card instead of the icon. |
| `permissions` | extensions | The same list as the package, so the card says what it can do before anything is downloaded. |
| `adds` | no | What it adds, in words, for the card: `["5 Browse shelves", "3 group rules"]`. |

The app fetches files from `https://raw.githubusercontent.com/imanimal/pocket-mods/main/marketplace/` and keeps each for an hour.
