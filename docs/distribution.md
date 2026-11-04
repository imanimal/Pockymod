# Distribution, Nexus approval, SEO and funding

Last reviewed: 2026-11-04.

This document covers how to get the app approved by Nexus Mods, how to release it, how discoverable it will realistically be, and how it can be funded. It sits alongside `docs/build-prompt.md`, which remains the product spec.

## How to read this document

Every claim carries one of three labels:

- **Sourced**: taken from a page or search excerpt, with a link in the Sources section.
- **Judgment**: a reasoned estimate with no measured data behind it.
- **Unverified**: a claim that was repeated but could not be confirmed. Do not rely on these until checked.

Important gap: the full text of the Nexus API Acceptable Use Policy (AUP) could not be read when this was written. The network policy of the research environment blocked `help.nexusmods.com`, `www.nexusmods.com` and `forums.nexusmods.com`. Everything about the AUP comes from search excerpts. The monetization, ads and attribution clauses in particular have not been read. Re-read the live AUP before sending the application.

## 1. Summary

| Question | Short answer |
| --- | --- |
| Will Nexus approve the app? | Judgment: approval with conditions is the most likely result. Rejection of the whole app is unlikely. Restriction of the in-app browsing is a real possibility. |
| Biggest risk | The "fewer page views" objection to in-app browsing. |
| How good is SEO? | Judgment: weak for general terms for at least six months, decent for the exact app name, useful over time for error-specific pages. The GitHub README will outrank a new site early. |
| Can the app earn money? | Yes, through donations. Ads and paywalled Nexus features are risky. |
| Best funding setup | GitHub Sponsors plus Ko-fi. Patreon only if recurring perks will be delivered. |
| Public release before approval? | Do not ship a public build that relies on users pasting personal API keys. |

## 2. Nexus API access

### 2.1 What the policy says (sourced, from excerpts)

- The AUP covers API use in any third-party site or app and extends Nexus's Fair Use Policy.
- Personal API keys are tolerated for apps in the testing stage or intended for personal use only.
- A public-facing app must contact Nexus support once it has a testing build that demonstrates intended API usage. Nexus checks policy compliance and that the app does not consume an excess amount of requests.
- Nexus reserves the right to deny registration if it has doubts about the nature of the app, how data is handled, or whether the code complies. Including source code with the request is encouraged.
- A registered app receives a "slug" for use with SSO.
- Nexus releases its own tools as open source and strongly encourages API apps to use open-source licensing.
- Apps are asked to identify themselves in request headers.
- Misuse may lead to Nexus blocking the app's access to its data.

### 2.2 Enforcement is real (sourced)

A Gothic remake mod manager removed its Nexus API use because it read the AUP as forbidding personal keys in public apps. Treat the personal-key rule as enforced.

### 2.3 The auth model may be changing (unverified)

Community documentation says all applications now need OAuth 2.0 with a registered Client ID, with public apps using PKCE and no client secret. Nexus also announced turning off its legacy API. The spec assumes an `APIKEY` header or SSO. Ask Nexus which flow they want, and keep key storage behind an interface so OAuth can be swapped in without touching UI code.

### 2.4 Cost

Judgment from the policy excerpts: registration is not described as paid. Premium membership is not required to apply. Confirm in the reply from support.

### 2.5 Timing

Apply once these are done:

1. The final name is chosen (see section 6.1).
2. One end-to-end smoke test on Windows: install, free-account click flow, nxm handling, hard-link deploy and uninstall.
3. A short written statement of what data the app keeps. Per the spec: everything stays local, the key lives in Windows Credential Manager, nothing is sent anywhere except to Nexus.

Do not wait for a complete build. The requirement is a working testing build.

## 3. The page-view objection

### 3.1 Why it matters

Nexus says advertising is a major income source and that it relies on a small share of users for Premium. An app that lets people browse without visiting the site touches that revenue directly. This is the point reviewers are most likely to question. Nexus also retired its own cross-platform app in January 2026 to focus on Vortex, so it is attentive to what runs alongside Vortex.

### 3.2 Levers (judgment unless stated)

1. **Free users still pass through the site.** The free path needs the Mod Manager Download button on the real page, which shows ads and the countdown. The app does not skip, scrape or automate this. State it plainly.
2. **Premium users see no ads anyway.** In-app browsing costs Nexus no ad revenue from them. Frame the app as helping Premium conversion by showing the value of direct downloads to free users.
3. **Browse shows API metadata only.** Cards and filters use API data. The detail view shows a summary and a clear "View on Nexus" key that opens the real page.
4. **Load the real page faithfully.** Inside the WebView2, do not block scripts, ads or analytics, do not inject adblocking, and keep normal cookies and login. Whether ad networks treat embedded views as normal traffic is unknown. Ask Nexus.
5. **Make creators benefit.** Support endorse and track in-app, and update notices that send users to author pages. Ask Nexus whether downloads made via the API count toward author statistics.
6. **Reduce their cost.** Cache with TTLs, one `mods()` call per page, read rate-limit headers, slow down early, and offer a hard per-install request budget.
7. **Offer attribution.** If Nexus has a referral or Premium upsell route, offer to use it for the in-app Premium prompt. Whether one exists is unverified.

### 3.3 Fallback design

Build so that the browser degrades to "open on Nexus" without breaking installs. If Nexus restricts what the detail view may show, the change should be a configuration switch, not a rewrite.

## 4. Likelihood and scenarios

These probabilities are judgment, not data.

| Scenario | Likelihood | What it means | What to do |
| --- | --- | --- | --- |
| Approved as proposed | Moderate | Full browse, detail and install. | Ship. Keep the request budget conservative. |
| Approved with conditions | Most likely | For example limits on detail content, request volume, or caching duration. | Implement the conditions, keep the fallback switch. |
| Browse restricted, install allowed | Real possibility | Search results link out to Nexus. | Keep install, queue, health and collections. These do not depend on browsing. |
| Rejected | Unlikely for a well-behaved open-source app | No public API access. | Stay a personal-use tool, or ask what would change the decision. |

Factors in favour: open source is encouraged, the free-user flow respects the ad and countdown step, the app is Cyberpunk-only, and Nexus has just narrowed its own manager effort.

Factors against: Nexus can deny on "doubts about the nature of the application", and the product's stated goal is that users rarely open a browser.

## 5. The application

### 5.1 Where to send it

Use the Nexus support contact on the help site. Check the AUP page for the current address or form before sending.

### 5.2 Draft

Replace the bracketed items.

> Subject: API registration request: [App name], an open-source Cyberpunk 2077 mod manager
>
> Hello,
>
> I am requesting registration for [App name], a Windows mod manager for Cyberpunk 2077. It is open source under [licence], at [repository link]. A testing build that works with a personal key is available at [link].
>
> What it does: it lets users search Nexus mods for Cyberpunk 2077, queue installs, and manage framework health after game patches.
>
> How it uses the API:
> - One `mods()` query per results page, requesting only the fields shown on cards. Detail data is fetched only when a user opens a mod.
> - Responses are cached, and the app reads the rate-limit headers and slows down before limits are reached.
> - Every request sends `Application-Name` and `Application-Version`.
> - No scraping.
>
> Free accounts: downloads go through the Mod Manager Download button on the Nexus page, including the ads and countdown. The app only receives the resulting nxm link. It does not bypass or automate that step. Premium accounts use the official download-link endpoint.
>
> Data: everything stays on the user's machine. The key is stored in Windows Credential Manager. The app sends nothing anywhere except to Nexus.
>
> Browsing: results come from API metadata. Opening a mod's full page happens in an embedded view that loads the real Nexus page unmodified, with ads and analytics intact.
>
> Questions:
> 1. Which authentication flow do you want for a public app?
> 2. Is the v2 GraphQL API open to third-party apps, and what request volume is acceptable?
> 3. How much mod detail may the app display before sending users to the page?
> 4. Do downloads made through the API count toward author statistics?
> 5. Is a donation link on the app's own site and repository acceptable?
> 6. Is there a referral or attribution route you would like us to use for Premium?
>
> I am glad to share source access and adjust the design to your guidance.
>
> Thank you.

### 5.3 Naming

Do not use "Nexus" in the app name. Nexus said its own retired app may be forked by the community only under a different name, which shows it guards its branding (sourced, from a news summary). Whether that applies to third-party tools is unverified. Choose a name that is distinctive in search, which also helps SEO.

## 6. Distribution

### 6.1 Channels

| Channel | Cost | Notes |
| --- | --- | --- |
| GitHub Releases | Free | Primary download source and the source for in-app updates. |
| winget | Free | Submit a manifest. |
| Scoop | Free | Submit a manifest. |
| Chocolatey | Free | Submit a manifest. |
| Microsoft Store | Registration cost unverified | Unverified whether the Store removes the signing need for a Tauri installer. Check before relying on it. |
| Nexus listing (modding tools) | Free | Large trust and search benefit. Do not put donation links on it. |
| GameBanana | Free | Small extra audience. Nexus first. |

### 6.2 Code signing

The unsigned installer is the largest trust problem because of SmartScreen.

- **SignPath Foundation**: free for qualifying open-source projects. One write-up lists an OSI-approved licence and a public source repository as minimums. Check the full terms, including whether donations are acceptable.
- **Azure Artifact Signing** (formerly Trusted Signing): about 10 USD per month, with sources disagreeing on the exact figure. Microsoft documentation lists individual developers in the USA and Canada only, and a March 2026 support answer says onboarding was tightened further during the preview.
- **Traditional OV certificate**: roughly 150 to 300 USD per year per Microsoft's guidance, from a conventional certificate authority.

Recommended order: apply to SignPath first, use Azure if the developer is in an eligible country, buy a certificate only as a last resort.

### 6.3 Hosting

GitHub Pages or Cloudflare Pages are free. A domain is about 10 to 15 USD a year. Downloads live on GitHub Releases at no cost. Total cost is roughly the domain price, plus signing if a paid route is needed.

## 7. Discovery and SEO

None of these numbers are measured. No keyword tool was available.

### 7.1 What to expect

- **Exact app name**: should rank first within days to a few weeks of indexing, provided the name is unique.
- **General terms** such as "Cyberpunk 2077 mod manager" and "Vortex alternative": results are held by Nexus, Reddit, YouTube and GitHub. Expect no first-page placement for at least six months. With backlinks it is plausible within six to twelve months, but that is a guess.
- **GitHub README**: repository pages carry more authority than a new domain and will probably outrank it early. Treat the README as the first SEO page.
- **Volume**: expect a trickle from search in the first half year. Launch traffic comes from community posts and videos.

### 7.2 README checklist

- Clear title that includes the main descriptive phrase.
- First paragraph with the target phrases in plain language.
- Screenshots and a short clip.
- Repository topics such as `cyberpunk-2077`, `mod-manager`, `redmod`.
- Link to the site and to the Nexus page.

### 7.3 Site plan

1. One fast static landing page titled for what people search, with screenshots and a short video.
2. `SoftwareApplication` structured data.
3. Search Console with a sitemap submitted.
4. Error pages for common failures, such as redscript compile errors and ArchiveXL not loading. Each must be accurate, dated, and tied to a game and framework version. These double as targets for "learn more" links in the Health panel.
5. Update the error pages after each game patch. Stale content is worse than none.

### 7.4 Backlinks, in priority order

1. The Nexus tool page (after approval).
2. The RED modding wiki tools list.
3. The GitHub README.
4. Community posts that link to the site.

### 7.5 Launch tactics

1. List the tool on the RED modding wiki tools page.
2. Post in the Cyberpunk modding Discord.
3. Post in the Cyberpunk subreddits with a 30-second clip showing one-click installs and the Health panel.
4. Offer the build to modding YouTubers.
5. Release shortly after a game patch, when frameworks break and the Health panel shows its value. Per the spec, Update 2.5 shipped on 22 October 2026 with official mod.io support. That is unverified here, but if true it is a recent and active window.

## 8. Funding

### 8.1 Open source and money

Open source does not block earning money. The constraint is the Nexus relationship, not the licence.

### 8.2 Options

| Option | Fit | Notes |
| --- | --- | --- |
| GitHub Sponsors | Good | 0 percent fees for personal accounts and 3 to 6 percent for organisations, per one comparison. Sits on the repository. |
| Ko-fi | Good | One-off tips suit the modding audience. Free tier fee is reported as 5 percent on memberships and shop sales, with sources disagreeing on donations. |
| Patreon | Weak | Higher overhead and an expectation of recurring perks. Use only if perks will be delivered. |
| In-app ads | Avoid | Competes with Nexus's inventory and erodes trust. AUP clauses unread. |
| Paywalled features that use Nexus data | Avoid | Likely AUP conflict (unverified). Ask Nexus before considering. |
| Perks that do not weaken the open-source promise | Fine | Credits, a Discord role, roadmap votes, early binary builds. |

No reliable income benchmarks were found. Judgment: for a niche game tool, expect enough to cover domain and signing costs, not a salary.

### 8.3 Rules of thumb

- Keep all donation links on the app's own site and repository, not on the Nexus page. A search excerpt says Nexus disallows links to Patreon and Ko-fi on mod pages. The thread itself could not be read and it concerns mods, so ask in the application.
- Prefer GPL-3.0 over MIT (judgment). It discourages closed forks that add ads or paywalls to an app given away for free.
- Do not use donations or perks to gate access to Nexus data.

## 9. Open questions to resolve

1. Full text of the AUP, especially clauses on ads, monetization, branding and attribution. Needs network access to the Nexus help site.
2. Required auth flow: personal key, SSO slug, or OAuth 2.0.
3. Whether the v2 GraphQL API is available to third parties and at what volume.
4. How much mod detail an app may show before linking out.
5. Whether API downloads count toward author statistics.
6. Whether a referral or attribution route exists for Premium.
7. Whether the Microsoft Store removes signing needs for a Tauri installer.
8. SignPath Foundation terms regarding donations.
9. Whether Nexus's paid-mod and link policies apply to tool pages.

## 10. Action checklist

- [ ] Choose a distinctive name without "Nexus" in it.
- [ ] Windows smoke test: install, free-user flow, nxm, deploy, uninstall.
- [ ] Write the short data statement.
- [ ] Re-read the live AUP.
- [ ] Put the key store behind an interface so OAuth can replace it.
- [ ] Send the application with a testing build and source link.
- [ ] Apply to SignPath Foundation.
- [ ] Prepare the README and the landing page.
- [ ] Submit winget, Scoop and Chocolatey manifests once signed.
- [ ] Prepare the launch clip and community posts for the first release after a game patch.
- [ ] After approval, create the Nexus tool page without donation links.

## Sources

- [Nexus API Acceptable Use Policy](https://help.nexusmods.com/article/114-api-acceptable-use-policy) (excerpts only)
- [Why Nexus Mods uses advertising](https://help.nexusmods.com/article/94-why-does-nexus-mods-use-advertising) (excerpts only)
- [Nexus news: turning off the legacy API](https://www.nexusmods.com/news/14028)
- [Coverage of the Nexus Mods App retirement](https://www.gamingonlinux.com/2026/01/nexus-mods-retire-their-in-development-cross-platform-app-to-focus-back-on-vortex)
- [Community OAuth 2.0 guide](https://modding.wiki/en/api/oauth2-guide) (unofficial)
- [Gothic remake mod manager release notes](https://github.com/ISKL23/G1L-Releases/releases)
- [Microsoft: code signing options for Windows apps](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
- [SignPath Foundation application page](https://signpath.org/apply.html)
- [GitHub Sponsors vs Ko-fi comparison](https://www.wmtips.com/technologies/compare/github-sponsors-vs-ko-fi)
