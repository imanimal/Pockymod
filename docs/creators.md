# For mod authors

The Create view is for people who make Cyberpunk 2077 mods. It has four tabs: **My mods**, **New mod**, **Check and
pack** and **Publish a version**. Most of an author's routine (seeing how mods are doing, checking a build, packing it,
pushing a new version) happens in the app. Nexus makes a few things possible on its website only; for those the app
opens the right page in its own Nexus window, never an outside browser.

What the app knows about the Nexus API, and how it was checked, is in [nexus/live-probe.md](nexus/live-probe.md)
("Author tools").

## What you need

- To see your mods: nothing. Type your Nexus user name, member ID or the link to your profile. Without a key this is
  read-only and shows published mods only.
- To publish versions: your personal API key from the Nexus site (Site preferences, API keys), added in Settings. It is
  kept in Windows Credential Manager. With it, My mods opens on your own mods by itself.

## My mods

- LCDs at the top: how many Cyberpunk mods you have, their downloads (and unique downloads), endorsements, and this
  month's downloads (Nexus counts that one across all your games).
- One card per mod: picture, status (published, hidden, not published, under moderation, removed), version, downloads,
  unique downloads, endorsements, category, last update, how many mods it requires and how many require it, and its
  current files with category, version, size and file ID. Old versions are counted, not listed.
- **Manage** opens a drawer for the mod:
  - keys for the website's pages, in the Nexus window: edit the page, manage files, the mod page, Files, Posts, Bugs
    and Logs;
  - share: copy the page link, a link to the Files tab, a BBCode link or a Markdown link; each file row also copies a
    link to that file;
  - every file version, with downloads, unique downloads, the virus scan state when it is not a plain pass, and the
    update chain the version belongs to;
  - the changelog, one entry per version, from the lines Nexus keeps on each file;
  - what the mod requires (mods and DLC, with your notes) and the mods that list it as a requirement.
- **Add a mod by its link**: for a page you just made that Nexus does not list yet, or a mod you work on in someone
  else's team. The link or number is kept, and the mod shows in My mods once Nexus lists it.
- **Look up another member** shows anyone's mods, read-only.

The list is cached for ten minutes; the refresh key in the band asks Nexus again.

## Update a file (publish a new version)

1. Build your mod as usual, then open **Check and pack**. Choose the mod's folder. The check shows where every file
   lands in a player's game, what would break installs, and which frameworks the files need.
2. Set the version and press **Pack a clean zip**. The zip has each file at its game path (archive/pc/mod, r6/scripts …),
   with docs and tools left out, and is saved next to your folder. You can also skip packing and choose an archive you
   made yourself.
3. In **My mods**, press **New version** on the file you are updating (or **Publish a version** at the top). The publish
   form opens with the mod and the file already picked. With your key, the form also shows the files Nexus takes uploads
   for and uses Nexus's own ID for the one you picked, so there is no ID to look up.
4. Check the version (**Use 1.2.4** suggests the next one), the file's name and category, and write the changelog.
   Choose whether the previous version moves to Old files and whether the mod's version follows this file. A line above
   **Publish** reminds you that Nexus wants the page's AI statement updated when a version adds AI-made parts, with a
   link to the mod's edit page: the form cannot do that for you, and nothing here skips it.
5. Press **Publish**. The LCD follows the upload. Nexus then scans the file; players see the new version once the scan
   passes. My mods refreshes.

You can still enter a file ID and mod ID by hand ("Enter the IDs by hand"). The file ID is on the mod page's Files tab
under "Advanced", or in the file's edit menu on Manage files.

## Upload a new mod

Nexus's Upload API can update files but cannot create a mod page, so **New mod** guides you through the website's form:

1. **Check and pack** your folder (the same check and pack as above, on the left of the tab).
2. **Make the page on Nexus.** Press **Open the add a mod page**; the form opens in the Nexus window. Tick each part off
   as you fill it in. The checklist holds what to type, with copy keys:
   - the name (from Pack it);
   - a summary you write here (it is kept between visits);
   - the category, from the list Nexus uses for Cyberpunk;
   - the file: the zip you just packed, as the Main file, with its version, and mod manager downloads on;
   - the requirements the check found in your files (ArchiveXL, redscript …), each with a key that copies its Nexus link;
   - at least one picture;
   - publishing the page, or keeping it hidden while Nexus scans the file.
3. **Show it in My mods.** Press **Look for it** once the page is published: your newest mods are listed, and the new one
   is marked. Before it is published, paste the page's link (or its number) and it is kept in My mods until Nexus lists
   it.

From then on, every update goes through **Publish a version**.

## What still needs the website

- Making a mod page, and its first file.
- Writing the description, adding images and videos, changing the category, tags, permissions and requirements.
- A second file on an existing mod (for example a new optional file). The Upload API has a call for it, but the app does
  not use it yet.
- Hiding, publishing, deleting or archiving files and pages.
- Reading and answering comments, bug reports and messages: Nexus does not give apps a mod's posts.

The Manage drawer has a key for each of these pages.

## Not verified without an author's key

These could only be checked as far as an anonymous request allows (see the live probe). Each is marked UNVERIFIED in
the code.

- Every Upload API call that publishes: the multipart upload, finalising it, the new version, the changelog. They follow
  Nexus's own GitHub Action.
- That the Upload API's file id equals the file's update chain (`groupId`) that the cards and the publish form start
  with. With a key the form asks Nexus for the real list (`GET /mods/{uid}/files`) and uses its ids; the shape of that
  answer is also unverified, and the app accepts both shapes the schema allows.
- That the changelog goes to the mod's composite id (gameId << 32 | modId) rather than the number in its address. The
  schema says "unique identifier", the composite id is what Nexus's other v3 calls return, and the old code used the
  page number.
- That hidden, unpublished and under-moderation mods show in My mods when you are signed in. The app asks for them
  (`viewUploaderHidden`), but without the owner's key Nexus lists published mods only. Use **Add a mod by its link** for
  a page that does not show.
- The website addresses: the editor (`/cyberpunk2077/mods/edit/?id=<id>`, with `&step=files` for Manage files), the add
  a mod form (`/cyberpunk2077/mods/add`), the tabs (`?tab=files`, `posts`, `bugs`, `logs`) and the profile
  (`/users/<member id>`). Nexus's site turns away anything but a browser, so they could not be fetched; they follow the
  site's long-standing paths. If one moves, the mod page's own menu still gets you there.

## Requests it makes

- My mods: one GraphQL request per 100 mods (mods and member together), one per ten mods for their files, and one small
  v3 request per mod for unique downloads, cached for an hour. Anonymous, except that the key is sent when there is one.
- Manage: one GraphQL request, plus the cached unique downloads.
- Publish form with a key: one v3 request for the file list, plus one GraphQL request to match it to the update chains.
