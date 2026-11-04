# Nexus live probe

Run: 2026-11-04 09:23 UTC. Auth: none (anonymous). Game: cyberpunk2077 (gameId 3333).

## Schema

- `mods()` arguments: viewUploaderHidden, viewUserBlockedContent, facets, filter, postFilter, sort, offset, count
- `ModPage` fields: facets, facetsData, nodes, nodesCount, nodesFacets, nodesFilter, totalCount
- `ModsFilter` fields (27): filter, op, name, nameStemmed, modId, id, gameId, gameDomainName, createdAt, updatedAt, hasUpdated, uploaderId, adultContent, fileSize, downloads, endorsements, tag, description, author, uploader, supportsVortex, languageName, categoryName, status, gameName, primaryImage, directDownloadEnabled
- `ModsSort` keys (10): relevance, name, downloads, uniqueDownloads, endorsements, random, createdAt, updatedAt, size, lastComment
- `ModsFacet` fields: gameDomainName, gameName, gameId, adult, languageName, status, categoryName, tag
- `Mod` fields (38): adultContent, author, category, createdAt, description, directDownloadEnabled, downloads, endorsements, fileSize, game, gameId, id, isBlockedFromEarningDp, legacyModRequirementsEnabled, mirrors, modCategory, modId, modRequirements, name, pictureUrl, status, summary, supportsVortex, tags, thumbnailBlurredUrl, thumbnailLargeBlurredUrl, thumbnailLargeUrl, thumbnailUrl, uid, updatedAt, uploader, version, viewerBlocked, viewerDownloaded, viewerEndorsed, viewerIsBlocked, viewerTracked, viewerUpdateAvailable

## A `mods()` page

Variables sent:

```json
{
  "count": 3,
  "facets": {
    "categoryName": [],
    "tag": []
  },
  "filter": {
    "adultContent": [
      {
        "value": false
      }
    ],
    "gameDomainName": [
      {
        "value": "cyberpunk2077"
      }
    ]
  },
  "offset": 0,
  "sort": [
    {
      "downloads": {
        "direction": "DESC"
      }
    }
  ]
}
```

Response: HTTP 200, totalCount 22615, nodesCount 22615, facetsData keys: categoryName, tag.

First node, with the type of each field:

```
adultContent: bool
author: string
category: string
createdAt: string
downloads: number
endorsements: number
fileSize: number
modCategory: object
  categoryId: number
  name: string
modId: number
modRequirements: object
  nexusRequirements: object
    nodes: array[0]
    totalCount: number
name: string
status: string
summary: string
tags: array[8]
  id: string
  name: string
thumbnailUrl: string
uid: string
updatedAt: string
uploader: object
  memberId: number
  name: string
version: string
```

Response headers: cache-control, cf-cache-status, cf-ray, content-type, date, etag, referrer-policy, server, vary, x-content-type-options, x-country, x-frame-options, x-permitted-cross-domain-policies, x-request-id, x-runtime, x-xss-protection.

Rate-limit headers: none.

## Section 4 claims

| Claim | Result | Notes |
|---|---|---|
| Search by name and description | confirmed | "better vehicle": Ok(368) of 22615 |
| Category filter | confirmed | Gameplay OR Weapons: Ok(5771) |
| Tag filter | confirmed | tag REDMod: Ok(412) |
| Author filter | confirmed | ModsFilter has author and uploader |
| Date filters | confirmed | updated in the last 7 days: Ok(1497). Values must be Unix seconds as strings; ISO dates fail |
| Downloads range | confirmed | 100k–200k downloads: Ok(1393) |
| Endorsements filter | confirmed | ≥10k endorsements: Ok(84) |
| File size filter | confirmed | ≥1,000,000 KB: Ok(61) |
| Adult-content flag | confirmed | with adult Ok(25907), without 22615 |
| Update flag | confirmed | ModsFilter has hasUpdated (boolean); not exercised |
| Boolean AND/OR | confirmed | nested filter list with op AND/OR; several values in one field are ANDed |
| 10 sort keys | confirmed | relevance, name, downloads, uniqueDownloads, endorsements, random, createdAt, updatedAt, size, lastComment |
| Facet counts | confirmed | facets available: gameDomainName, gameName, gameId, adult, languageName, status, categoryName, tag; returned only when asked for in `facets` |
| Offset and count paging | confirmed | offset 3 first mod: Ok(Some(1511)) |
| Mod has requirement links | confirmed | modRequirements.nexusRequirements, and it can be fetched inside a mods() page |
| Mod has no returnable tags | differs | Mod.tags: [LegacyTag!]! is returned (id, name) |
| Mod has no mod-type field | confirmed | category is a site category, not REDmod/archive/script |
| Mod has no game-version field | confirmed | nothing on Mod; version is the mod's own |
| collectionsV2 has a game-version filter | confirmed | sort options: relevance, createdAt, updatedAt, endorsements, downloads, rating, recentRating |
| v1 REST has no search; mods/updated returns IDs and timestamps | not checked | v1 answers 401 without a key |
| Detail: mod() + modFiles() in one request | confirmed | ArchiveXL: 1 requirement(s) (RED4ext), 122 files, current MAIN: Some(("1.28.6", 170440)) |
| Archive file lists (modFileContents) | confirmed | ArchiveXL, all files: Ok(1069) entries; newest MAIN file: Some(0). The index does not cover every file |
| Derived filter counts in one aliased request | confirmed | Ok("fw_archivexl=5059 fw_cet=3193 fw_codeware=1210 fw_cybercmd=71 fw_red4ext=2345 fw_redscript=2642 fw_tweakxl=2761 type_archive=17119 type_framework=522 type_redmod=818 type_script=4251") |
