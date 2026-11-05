# Round 4: the tube, Nexus pages where you want them, Vortex's profiles, and doing more from a card

## Reported problems, and what changed

**The start-up static looked bad.** The SVG noise overlay is gone. Starting and closing are Collegiapp's tube TV
(its `fx.js` powerOn and powerOff and its `crt.css` layer, ported as they are, in `src/ui/fx.ts` and
`src/styles/fx.css`): the beam line opens into a raster of snow while the degauss coil swirls through it, the picture
rolls in, the static clears and colour locks; the app's own start (modules settling in, LCDs switching on) waits for
the static to clear, so it plays through the last of it. Closing collapses the picture to a line, then to a dot that
fades. Closing by any means (the close key, Alt+F4, the taskbar) plays it first: the main window asks the page and
quits when it answers, or after 2.6 seconds if it does not (`src-tauri/src/power.rs`, after Collegiapp's). Without
the CRT layer both only fade. A view change with full effects is Collegiapp's channel change.

**Page switching showed scrollbars (Redline).** The old channel change skewed the whole view, which overflowed the
window. The window and the shell never scroll now, and the view's scrollbars stay hidden for the moment a new view
settles in.

**Redline is the default theme**, and where Ctrl+Alt+Shift+T resets to.

**Tidy up all came after the list.** It, and Use mine for all, are above their lists now.

**Mods brought in from Vortex lost their profiles.** Vortex's profiles for Cyberpunk 2077 now come along: each
becomes a profile here with the same mods on, and the one Vortex has deployed becomes the active one, so nothing in
the game changes. For mods brought in before this, Library shows a notice with **Bring in N profiles**, and Load
order, Profiles has the same key (it syncs again later too). Vortex's settings are a LevelDB folder
(`%APPDATA%\Vortex\state.v2`); it is read from a temporary copy and never opened where it is.

**The marketplace "has no index.json yet".** Not intentional: the app reads the marketplace from this repository's
main branch, and the marketplace (with the docs and the example themes the app links to) is on the feature branch
until it is merged. The app now carries the marketplace as it was when it was built and shows that copy, with a
note, when the online one is missing or unreachable. The docs links work once the branch is on main.

**The Nexus window loaded without its CSS and scripts the first time.** The cause could not be found from here
(the app's own request filters do not cover nexusmods.com). Two guards: a new webview is made blank and then sent to
the page (the path that worked), and a page whose stylesheets all failed loads again, once.

**In-app logins are not instant, and the in-app browser was "just a new window".** Nothing lets an embedded
WebView2 use the passwords or the sign-in of your own browser, and Google refuses sign-ins inside embedded browsers.
So Nexus pages now open **in your own browser by default**: you are signed in there, your saved passwords and
extensions work, and Mod Manager Download comes straight back to the queue through the nxm:// link handler (the app
says when another manager has taken that handler, with Take them back). In Settings, **Nexus pages, In the app** opens
them in a **pane docked in the main window**: beside the app with a handle to resize it, over the whole view, hidden
behind a tab at the window's edge (a page waiting for your click brings it back), popped out into its own window
and back, or closed. Its bar has back, forward, reload, the address, and Open in your browser. The pane has its own
profile, so you sign in there once and it stays signed in, and unpacked Chrome extensions in its extensions folder
load into it. **Open in browser** is on every mod and collection either way.

**Pictures were cut off in the sidebar and could not be zoomed.** The viewer is over the whole window now (it was
trapped inside the drawer), always fits, and zooms with the wheel where the pointer is, + - 0 1, double-click and its
keys; drag moves a zoomed picture.

## New

- **Expand** a mod's drawer into a page laid out like its Nexus page: the banner, the media on a big stage, the full
  description, and the files, requirements and archive list beside them. Requirements open as pages, with Back.
  The choice of drawer or page is kept.
- **Quick install** on every Browse card: the newest main file and what it needs go straight to the queue (Premium
  downloads at once; a free account gets one click on Nexus). A card for a mod you have shows **Installed** (or Off,
  or Update) and opens a menu with everything the app can do to it: on or off, update or skip, deploy, change
  options, move to a group or a new one, on or off in each profile, remove, endorse, track, its Nexus page here or in
  your browser, show in Library, hide and ignore. Undo covers it.
- **Collections** open as a page from anywhere on the card: the picture, the description and its media, and every
  mod in it against what you have, with Install. Check on the card is still the quick dialog.
- **Fix all** in Health runs every fix the app can do by itself (downloads, deploys, scripts, then the REDmod
  deploy); Frameworks has Update all; Logs can turn off every mod with errors at once (one undo); Downloads can
  retry all failed.

## Test

1. Start: the tube comes on through the snow and the modules settle in as it clears. Close with the close key and
   with Alt+F4: the picture collapses to a dot, then the app quits. In Settings, Appearance, Effects off: both fade.
2. In Redline, switch between every view: no scrollbar appears.
3. With Vortex profiles: Library shows Your Vortex profiles; Bring in. The profile switch in the title bar lists
   them, the active one is the one Vortex had deployed, and switching changes the game folder.
4. Browse: Quick install on a card (Downloads shows it queued); on an installed mod's card, the menu: turn it off,
   Ctrl+Z, move it to a new group, turn it on in another profile.
5. Open a mod, Expand, open a picture, wheel-zoom and drag; open a requirement and go Back.
6. Settings, Nexus pages: In your browser (a free account's Install opens the file page in your browser; press Mod
   Manager Download there and the queue goes on), then In the app (the pane: resize, full, hide, pop out and back).
7. Collections: click a card anywhere for its page; Check on a card for the dialog.
8. Add-ons with no network: the marketplace that came with the app, with its note.

## Not verified (needs Windows, a real Vortex install or the live API)

- The pane and its window in WebView2: child webviews in the main window, bounds while resizing, hiding under the
  app's own windows, the separate profile folder, extensions loading, and sign-in popups.
- Whether the unstyled first load is gone (its cause was not found).
- Vortex's `state.v2` key layout against a real folder (it follows Vortex's source; the reader is tested on a LevelDB
  folder written the same way).
- `Collection.description` and `Collection.headerImage` (fetched apart, so a missing field costs only itself), and
  the collection page address `www.nexusmods.com/games/cyberpunk2077/collections/<slug>`.
- The power-off hook on Windows (CloseRequested from Alt+F4 and the taskbar).
