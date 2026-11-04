# Making a theme

A theme changes how Pockymod looks: its colours, its corners, its fonts and how much of the CRT effects it uses.
A theme is one small JSON file. You can write it by hand, or make it in the app and export it.

The app comes with three themes:

| Theme | Look | Schemes | Effects it suggests |
| --- | --- | --- | --- |
| Standard | a plain modern layout: flat keys, darkened bands, LCDs as status readouts, line icons | dark and light | subtle |
| Redline (the default) | a heads-up display: red lines on black, cut corners, bracket marks, uppercase labels | dark | full |
| Pocket | the handheld: raised keys, green LCDs, pixel icons, a dot grid | light (Cream) and dark (Night) | off |

Your theme builds on one of these. It keeps that theme's shapes and changes the colours and anything else you set.

## The quick way: make one in the app

1. Open **Settings**, then **Appearance**, and pick the theme you want to start from.
2. Press **Make your own**. Each colour you change shows across the whole window straight away.
3. If the theme has dark and light, use **Editing** to switch between them and set both.
4. Give it a name and press **Save and use**. It is now installed and in use.
5. Press **Export file** (in the maker) or **Export current theme** (in Appearance) to get the JSON file.

From there you can open the file in a text editor to go further: change the corners, the fonts, or add your own CSS.

## Step by step by hand

1. Copy one of the examples in [docs/examples](examples):
   - [neon-harbor.json](examples/neon-harbor.json) builds on Standard and has a dark and a light scheme.
   - [paper-terminal.json](examples/paper-terminal.json) builds on Pocket, light only, and uses the `css` field.
2. Change `id` and `name` (see the rules below). The id is also the file name, so `neon-harbor` becomes
   `neon-harbor.json`.
3. Change the colours. The full list of tokens is below; you only need the ones you want to change.
4. In the app, open **Settings**, **Appearance**, press **Import theme file** and pick your file. If something is wrong,
   the app says what and the theme is not installed.
5. Change the file and import it again: a theme with the same id replaces the one you installed before.
6. Open the **Gallery** (bottom of the left rail) to see every part in your theme: keys in each state, bands, LCDs,
   chips, switches, pills, rows, a dialog and toasts. Check both schemes if your theme has two.

Installed themes are kept as `%APPDATA%\app.pocketmods.desktop\addons\theme\<id>.json`. To remove one, press
**Remove** on its card in Appearance (twice: the first press asks).

## The file

```json
{
  "format": "pocket-mods.theme",
  "version": 1,
  "id": "my-theme",
  "name": "My theme",
  "author": "Your name or handle",
  "description": "What it is, plainly: colours and schemes.",
  "scheme": "dark",
  "base": "standard",
  "tokens": { "--accent": "#ff4fa3" },
  "dark": { "--bg": "#090d1a" },
  "light": { "--bg": "#eef0f8" },
  "css": ".nv.on { font-weight: 700; }"
}
```

| Field | Needed | What it is |
| --- | --- | --- |
| `format` | yes | Always `"pocket-mods.theme"`. |
| `version` | yes | Always `1`. |
| `id` | yes | Lowercase letters, digits, dots and dashes; starts with a letter or digit; at most 64 long; no `..`. It cannot be `standard`, `redline` or `pocket`. |
| `name` | yes | What the theme is called in the app, up to 60 characters. |
| `author` | no | Who made it, up to 80 characters. Shown on its card. |
| `icon` | no | The app's icon in the taskbar and tray while the theme is on: a PNG (16 to 512 pixels square, under 256 KB) as a data address, `data:image/png;base64,…`. Without one, the base look's mark shows. |
| `description` | no | Up to 300 characters. Shown on its card. Say what it is plainly (for example "Charcoal with amber keys. Dark only."). |
| `scheme` | yes | `"dark"` or `"light"`: the scheme it starts in. |
| `base` | no | The built-in look it builds on: `"standard"` (when left out), `"redline"` or `"pocket"`. |
| `tokens` | no | Tokens used in every scheme. |
| `dark`, `light` | no | Tokens for one scheme; they win over `tokens`. |
| `css` | no | Your own CSS, up to 64 KB. See [The css field](#the-css-field). |

### Schemes

- A theme that has both a `dark` and a `light` object has both schemes. It follows the Dark, Light and System choice
  in Appearance and the sun and moon key in the title bar.
- Any other theme has one scheme, the one named by `scheme`. The app shows it in that scheme whatever the choice is,
  and the title bar key says it comes in one scheme only.
- Tokens you leave out come from the base look in the scheme that is showing. So a dark theme that builds on Standard
  and only sets `--accent` is Standard's dark with your accent.
- In a two-scheme theme, `tokens` holds what both share (corners, fonts), and `dark` and `light` hold the colours.

## Tokens

Tokens are CSS custom properties. Every token name starts with `--`. Values are ordinary CSS values: colours as
`#rrggbb`, `rgb()`, `hsl()` or `color-mix()`, sizes as `px` or `em`, fonts as a font list.

### Colours

| Token | What it colours |
| --- | --- |
| `--bg` | The window behind everything (the desk). |
| `--bg2` | Fields, search boxes, sunken areas. |
| `--surface` | Modules (panels), cards and dialogs. |
| `--surface-2` | Raised parts on a module: keys, counts, hover on rows. |
| `--line` | Lines and outlines. |
| `--text` | Titles, mod names and strong text. |
| `--text-2` | Body text. |
| `--text-3` | Quiet text: labels, dates, hints. |
| `--accent` | The one primary key on a screen (Install, Save), the do-this-first band, the current view, progress. |
| `--on-accent` | Text and icons on the accent. |
| `--accent-ink` | The accent used as text or an icon on a surface; set a darker one when the accent is light (Standard light uses a dark yellow). |
| `--acc2` | The second accent: links, and in Standard and Redline what is selected and focused. |
| `--chrome-bg` | Module bands, the title bar (Standard), dark keys. |
| `--chrome-fg` | Text and icons on bands. |
| `--disp-bg` | LCD background. |
| `--disp-fg` | LCD figures. |
| `--disp-hot` | An LCD that is waiting or soon (amber). |
| `--disp-bad` | An LCD that shows a problem (red). |
| `--ok` | Good: installed, up to date. |
| `--warn` | Needs a look. |
| `--bad` | A problem, a conflict, a failed download. |
| `--on-bad` | Text on a `--bad` fill. |

### Selection and focus

| Token | What it does |
| --- | --- |
| `--sel` | What is selected: chips that are on, the picked row, switches (Standard and Redline; defaults to `--acc2`). |
| `--sel-soft` | The faint fill behind something selected. |
| `--focus` | The keyboard focus ring. |
| `--on-ring` | The outline of a latched key and the current view key (Pocket). |
| `--k-framework` | The dot on Framework tags. |

### Shape and type

| Token | What it does |
| --- | --- |
| `--radius` | Corner radius of modules and cards. |
| `--btn-r` | Corner radius of keys. |
| `--win-r` | Corner radius of the window frame in the gallery. |
| `--font-body` | Titles and body text. |
| `--font-mono` | Bands, legends and sizes (Pocket). |
| `--font-num` | LCD figures. |
| `--num-scale` | How large LCD figures are drawn, about `0.6` to `0.8`. |
| `--card-shadow` | The outline and shadow of a module. |
| `--drop` | The shadow under dialogs and menus. |
| `--scrim` | The shade behind a dialog. |

Fonts must already be in the app; a theme cannot load a font. These are bundled: `"Space Grotesk"`, `"Space Mono"`,
`"Doto"` (dot matrix, for LCDs) and `"Chakra Petch"` (500 to 700, Redline's labels). Windows fonts such as
`"Segoe UI Variable"`, `"Bahnschrift"` and `"Cascadia Mono"` work too. Keep body text in a clear sans at its normal
size.

### Effects and icons

| Token | What it does |
| --- | --- |
| `--fx` | The effects level your theme suggests: `off`, `subtle` or `full`. The user can pick another in Appearance. |
| `--icons` | `line` for line icons, `pixel` for Pocket's 9 by 9 pixel icons. |
| `--scan` | How dark the scanlines are, from `0` to `1`. |
| `--glow-rgb` | The CRT glow colour as three numbers, for example `255, 95, 87`. |
| `--lcd-glow` | A `text-shadow` for LCD figures when effects are full, or `none`. |

What the effects levels do: **off** has no scanlines or flickers. **Subtle** adds faint scanlines on LCDs and the
title bar, a soft edge to the window, and short flickers when the app starts, the view changes and the theme
changes; nothing over body text is blurred or warped. **Full** adds scanlines over the whole window, warped glass on
the title bar and toasts, glowing LCD figures, and longer moments: the picture opens from a line when the app starts,
jumps on a channel change and wobbles in colour after a theme change. All motion follows the Motion switch and the
Windows setting for reduced motion.

Tokens a theme cannot set: `--m` (the motion setting), `--press`, `--i` and `--p` (set by the app on each part).

## The css field

`css` is for anything the tokens cannot do. The app scopes it to your theme before it loads it, so it only applies
while your theme is showing and cannot change anything else:

- Every selector is put under `html[data-theme="<your id>"]`. Write `.btn { … }` and it becomes
  `html[data-theme="my-theme"] .btn { … }`.
- A selector that starts with `:root`, `html` or `&` means the window itself:
  `:root[data-scheme="light"] .panel` becomes `html[data-theme="my-theme"][data-scheme="light"] .panel`.
  Use that to style one scheme only.
- `@media`, `@supports`, `@container` and `@layer` blocks are kept, with their rules scoped the same way.
  `@keyframes` and `@property` are kept as they are.
- `@import`, `@font-face`, `@namespace`, `@page`, `@scope` and comments are removed.
- `url()` is only kept for `data:image` (PNG, JPEG, GIF, WebP, AVIF, SVG). Any other `url()` is replaced by `none`,
  so a theme never fetches anything from the web or reads a file.

The class names are those of the Pocket kit in `src/styles/pocket-os.css`: `.btn` keys (`.primary`, `.dark`,
`.ghost`, `.on`), `.nv` view keys in the rail, `.panel` modules with their `.ph` band and `.pb` body, `.display`
LCDs (`.d-lbl`, `.d-num`, `.d-sub`), `.fbox` search fields, `.chip` and `.seg`, `.sw` switches, `.pill` and
`.cchip` tags, `.row` list rows, `.mcard` and `.lcard` cards, `.ov` and `.dialog`, `.toast`, `.mtop` the title bar,
`.side` the rail and `.onb` the first-run setup. The Gallery shows each of them.

## Limits and checks

A theme that breaks a rule is not installed, and the app says which rule and where.

- The file is JSON, at most 4 MB.
- Ids, names, authors and descriptions follow the table in [The file](#the-file).
- Up to 200 tokens in each of `tokens`, `dark` and `light`; each value up to 400 characters.
- Token names are CSS custom properties (`--` then letters, digits, `-` or `_`).
- A token value cannot contain `{`, `}` or `\`, nor `;`, `<`, `>` or `!` outside a string, nor a comment. Brackets
  and quotes must match.
- In token values and in `css`: no `url()` other than `data:image`, no `expression()`, `image-set()`, `image()`,
  `cross-fade()`, `element()` or `src()`, nothing with `javascript:`, no `behavior` and no `-moz-binding`.
- In `css`: every `{` has its `}`, escapes (`\`) only appear inside strings, and every string is closed.

## Tips

- Check contrast: `--text-2` on `--surface` and `--disp-fg` on `--disp-bg` should reach at least 4.5 to 1.
  `--text-3` is for small labels, so keep it readable too.
- Keep `--accent` for one thing per screen. Standard and Redline use `--acc2` for what is selected so the accent
  stays rare.
- Try your theme with effects off, subtle and full, and with Motion off.
- Dark themes look best with `--bg` darker than `--surface`, and bands (`--chrome-bg`) darker still.

## Sharing a theme

To share a theme with others on the marketplace, see [docs/marketplace.md](marketplace.md). The theme file you export
from the app is the file you share.

## If a theme makes the app hard to use

- Picking a theme you installed asks **Keep this theme?** and goes back to the one you had after 12 seconds unless
  you press Keep.
- **Customize** (Settings, Appearance) changes the colours of the theme in use, built-in or installed, without making a
  new one: five swatches (accent, highlight, background, text, displays; the shades around them follow), one-click
  palettes, and every colour under Advanced. They are kept per theme and mode; Reset takes them off, and Save as a
  new theme turns them into a file to share.
- **Ctrl+Alt+Shift+T** switches back to Redline, the default, from anywhere, even when the theme hides the menus.
- A theme's own css is switched off while the extension review dialog is open, so what an extension asks for always
  shows as it is.
- Theme css cannot load anything: `url()`, `image-set()`, `@import` and `@font-face` are refused, and so are
  backslash escapes, carriage returns and form feeds (they let a checker and the browser read css differently). Put a
  picture in a token as a `data:image` value instead.
