# Marketplace

This folder is the Pocket Mods marketplace. The app's **Add-ons** screen reads it from GitHub, so what is merged here shows in every copy of the app within an hour.

- `index.json` lists every theme and extension.
- `themes/<id>.json` are themes, `extensions/<id>.json` are extensions.

To get add-ons, open **Add-ons** in the app. To share yours, use **Share** on it in **Add-ons**, **Installed**, or open a pull request that adds its file here and an entry to `index.json`. Check your change with `node scripts/validate-marketplace.mjs` from the repository root.

- [docs/marketplace.md](../docs/marketplace.md): installing, updating, sharing, what the review checks, the index fields.
- [docs/extensions.md](../docs/extensions.md): the extension format, every contribution, permissions and the panel API.
- [docs/themes.md](../docs/themes.md): the theme format.
