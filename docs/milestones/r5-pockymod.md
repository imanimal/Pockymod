# Round 5: Pockymod, the tray, curated collections, and doing everything faster

## What changed

- **Name and icon.** The app is Pockymod. Its mark is an angular P on a chamfered plate, in Redline (the default and
  the installer's), Standard and Pocket colours; the taskbar and tray wear the theme's, and a theme can bring its own
  icon (a PNG, `icon` in the package). Saved API keys, file formats and folders keep their old names, so nothing is
  lost. Theme descriptions are plain.
- **Background.** Closing the window turns the tube off and destroys the window: nothing is drawn and little memory
  is held while the app sits in the tray, where downloads, browser links and checks carry on. Every six hours, quietly:
  mod updates are checked and, for Premium, downloaded ahead (Update all is then instant; optionally they go in while
  you are away and the game is closed), and a new Pockymod is downloaded and installed when the window is closed or
  you quit, never during a download, then started again in the tray. The view, the search and the open mod come
  back. All of it is in Settings, Background, with Start with Windows (off by default). The release workflow
  publishes the installer with its checksum.
- **Installer options** on one page: every step that applies at once, a search over every option (/ focuses it),
  steps that depend on earlier picks appearing as you pick (already picked), Recommended, Defaults, a count, jump
  keys. Step visibility is now read from the installer.
- **Collections as the curator made them.** The collection's manifest is read at the check: each mod's installer
  picks are used instead of asking, its rules decide which mod's files win (never replacing your own choices), its
  clashes and notes are shown.
- **Customize** any theme in place (Settings, Appearance): five swatches and palettes, every colour under Advanced,
  kept per theme and mode, Reset, Save as a new theme.
- **Endorsing:** a quiet strip in Library (Endorse all, Pick with Not for me, Not now), a star on each mod not
  endorsed yet, Endorse for selected mods, and one toast after a play session at most weekly.
- **Shortcuts:** / and Ctrl+F find, Ctrl+1 to 9 views, ? the list, Escape, Alt+Left back, Ctrl+B the Nexus pane.
- **Hover tips:** every title shows as the app's own tip, on hover and on keyboard or controller focus.
- **Controller:** D-pad or stick between keys, A press, B back, X the mod's key, Y find, LB/RB views, triggers scroll,
  View the button list, Menu to Play; only while the window has focus.

## Not verified (needs Windows, a live key or a release)

- The tray, closing to it and opening again, the per-theme taskbar icon, and Start with Windows.
- The silent update: the installer's `/S` run and the restart, end to end (needs a published release).
- The collection manifest (`collectionRevision.downloadLink` and its download) and the endorsements list
  (`v1/user/endorsements.json`).
- A real controller in WebView2 (tested with a simulated one).
