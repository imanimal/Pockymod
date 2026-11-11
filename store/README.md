# Store pages

Text and images for the places Pockymod is listed. Copy from here into each site's form.

| File | For |
| --- | --- |
| [nexus.md](nexus.md) | The Nexus Mods page, its file and permissions, and the email that registers the app with Nexus |
| [gamebanana.md](gamebanana.md) | The GameBanana tool page |
| [github.md](github.md) | The repository's About box, social preview and release notes |
| [posts.md](posts.md) | A short post for Reddit or a Discord server |

## Images

- `images/cover.png` (1920x1080) is the main image on Nexus and the first screenshot on GameBanana.
- `images/github-social.png` (1280x640) is the GitHub social preview.
- The screenshots are the README's, in `docs/screens/`. Upload them in this order: browse, install, health,
  collection, library.

To change the cover, edit `images/src/cover.html` and run `sh store/images/src/render.sh` from Git Bash.

## The file

Both sites get `Pockymod-<version>-setup.zip` from the GitHub release. After a local build,
`scripts\store-zip.ps1` makes the same zip in `target\store\`.

## Order

1. Send the registration email in [nexus.md](nexus.md).
2. Publish a GitHub release with a tag like `v1.0.0-alpha.3`. The workflow attaches the zip.
3. Make the Nexus page, then the GameBanana page, with that zip.
4. Put the Nexus link in the GitHub About box and in the posts.
