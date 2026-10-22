# Domain watch: lookalike Bloxkeep sites

Bloxkeep is open source, so anyone can build a copy with a cookie stealer inside and hand it out from a site that
looks like Bloxkeep's, the way fake "Multi Roblox" sites are advertised on YouTube. Such a site needs a name and an HTTPS
certificate. Every certificate is published in Certificate Transparency (CT) logs within minutes of being issued, and
every new name shows up in lists of newly registered domains (NRD) within a day or two, usually before the site is
advertised anywhere. This watch posts each new name like Bloxkeep to Discord once, so it can be reported while it's
still new. Nothing runs on your PC.

| What | Where | How often |
|---|---|---|
| Searches crt.sh (a public search over the CT logs) for names starting like Bloxkeep | GitHub Actions, `.github/workflows/domain-watch.yml` running `services/DomainWatch/watch.py --source crtsh` | every 3 hours |
| Reads two free NRD lists whole and looks for Bloxkeep anywhere in a name | the same workflow, `--source nrd` | once a day, 9:47 UTC |
| Remembers the names it posted, from either source | `domain-status.json` on the `domain-status` branch (made by the first run) | |

A name found by both is posted once, by whichever finds it first. There's no Cloudflare part, unlike the
[Roblox watch](roblox-watch.md): a few hours is early enough here, crt.sh itself is minutes to hours behind the logs,
and the NRD lists come out once a day.

## What it finds and what it can't

The words looked for are `bloxkeep`, `bl0xkeep`, `bloxkep`, `bloxkeeep`, `blox-keep`, `bloxxkeep`, `getbloxkeep`,
`download-bloxkeep` and `downloadbloxkeep` (`PATTERNS` in `services/DomainWatch/watch.py`). Add one when you see scammers use
it: both sources pick it up.

- **crt.sh: names starting with one of them**, within hours of the certificate. `bloxkeep` alone covers
  `bloxkeep.com`, `bloxkeep.app`, `bloxkeep-download.net`, `bloxkeeper.io` and `bloxkeep.anything.xyz`. Not
  `free-bloxkeep.com`: crt.sh only answers searches by the start of a name, a search like `%bloxkeep%` comes back
  empty even when there are matches (tried 2026-10-16 with a name that has dozens of certificates).
- **NRD lists: names with one of them anywhere**, like `free-bloxkeep.com` or `roblox-bloxkeep.net`, a day or two
  after they're registered, with or without a certificate. Two lists, both free and meant to be downloaded by scripts
  (see [The NRD lists](#the-nrd-lists)).
- **Not sites on a shared address** such as `bloxkeep.pages.dev`, `bloxkeep.vercel.app`, `bloxkeep.netlify.app` or
  `something.github.io/bloxkeep`: those use the host's one certificate for every site and register no name, so they
  show up in neither source. Search for them by hand now and then, and search YouTube for "Bloxkeep download".
- **Not letters that only look alike** in other alphabets (an `о` that is Cyrillic): their names are written as
  `xn--…` in certificates and in the lists.
- A name from crt.sh is posted with its issuer (who gave the certificate: Let's Encrypt, Google, Cloudflare…), the
  day it was first seen in the logs, and a crt.sh link to the certificate. The other names on the same certificate
  aren't posted. Only certificates still valid are searched. A name from the NRD lists is posted with the lists it's
  in, a who.is link (its registrar and when it was registered) and a crt.sh search for its certificates, if any.
- Bloxkeep's own addresses go in `OFFICIAL` in `services/DomainWatch/watch.py` and are never posted, nor anything under them.
  There are none yet: the official home is the GitHub repository, which never shows in CT.

## The NRD lists

- [hagezi's NRD list](https://github.com/hagezi/nrd) (GPL-3.0, built from [Stamus Labs](https://www.stamus-networks.com/stamus-labs)
  data): `domains/nrd7.txt`, read from raw.githubusercontent.com (jsDelivr refuses a file this big). Every name
  registered from 7 days ago to yesterday, about 3.2 million (some 450,000 a day, 55 MB), updated about 6:00-8:00 UTC.
  A name a run misses is still in the next days' list, so a short gap loses nothing. Stamus Labs says its data can be
  a few days late.
- [WhoisDS's free daily list](https://www.whoisds.com/newly-registered-domains), which the page says "may be reused,
  including for commercial purposes, without a license": exactly 70,000 names a day, a sample of the day's
  registrations, not all of them. About 1 in 9 of its names isn't in hagezi's, so it's read too: the lists of the
  last 6 days each time (about 0.6 MB each). A day not out yet answers with an empty page and is skipped. Its
  robots.txt turns away a long list of crawlers, `Python-urllib` among them; the watch asks with its own name
  (`Bloxkeep-DomainWatch`), which robots.txt allows.
- Checked 2026-10-16: of 25 `.com` names picked at random from WhoisDS's 2026-10-15 list, 21 were registered on
  2026-10-13 (Verisign's RDAP), the rest were older names registered again or changed. `skyamato.com` and
  `stewarthase.com` (both registered 2026-10-13) were in both lists, and 89% of WhoisDS's names were in hagezi's.
  Searching both for `robloxs` found `robloxshuffle.com`, `roblox` found `nookcaferoblox.xyz`: the middle of a
  name, which crt.sh can't search.
- Neither list has every new name. Country domains (`.de`, `.ru`, `.vn`…) are partly missing, since many countries
  don't publish their zone files, and names come a day or two after they're registered, not within hours like a
  certificate. That's why crt.sh is still searched.
- hagezi's list is on GitHub itself, so GitHub Actions always reaches it. WhoisDS has no bot check in front (a plain
  IIS server, no Cloudflare) and answered a home PC, but hadn't been tried from GitHub's addresses yet on 2026-10-16:
  if it turns them away, the message in the next section says so after three days.

## When a source fails

crt.sh often answers 502 or 404 or takes over a minute. Each search is tried 3 times (after 20 s, then 60 s), and a
run stops searching after 20 minutes. A search that still fails is simply done again next run: a name is remembered
only once posted, so a missed run loses nothing and nothing is posted twice. The run doesn't fail for it, so GitHub
doesn't email you every 3 hours. If crt.sh hasn't answered every search for two days, one message says so; one more
says when it answers again.

The NRD lists work the same way, each on its own: hagezi's counts as failing when it can't be downloaded or hasn't
been updated for two days, WhoisDS's when not one of its last 6 lists could be read. After three days of that, one
message says so, and one more when it's back.

## Setting it up

1. **Discord.** It posts with the `DISCORD_WEBHOOK` secret the Roblox watch uses (setup in
   [roblox-watch.md](roblox-watch.md)). To post in another channel, make a webhook there (Edit Channel › Integrations ›
   Webhooks › New Webhook › Copy Webhook URL) and save it as the secret `DOMAIN_WATCH_WEBHOOK`. Secrets go in
   Settings › Secrets and variables › Actions, never in a file or a chat.
2. Once the workflow is on `main`, Actions › **Domain watch** › Run workflow. The first run posts *Domain watch
   started* with any names already in the logs and the lists and makes the `domain-status` branch. From then on
   crt.sh is searched every 3 hours and the NRD lists read once a day (GitHub runs schedules only from `main`, and pauses them after 60 days without a commit to the repository:
   Actions › Domain watch › Enable workflow brings it back).

## When it posts a name

Open the crt.sh link to see the whole certificate (a name from the NRD lists may have none yet: then the site may
not be up yet either, and you can look again in a few days), then look at the site from a browser you don't use for Roblox (or
a sandbox like urlscan.io: submit the address, it visits it for you). Don't download what it offers. If it hands out a
Bloxkeep copy or asks for Roblox cookies:

- **Google Safe Browsing**: safebrowsing.google.com/safebrowsing/report_phish (Chrome and Firefox warn within hours).
- **Microsoft SmartScreen**: microsoft.com/wdsi/support/report-unsafe-site, and the file itself at
  microsoft.com/wdsi/filesubmission as malware.
- **Cloudflare**, if the site is behind it (who.is shows `*.ns.cloudflare.com` name servers): abuse.cloudflare.com ›
  Phishing & Malware.
- **The registrar**: who.is/whois/the-name shows it and its abuse email; send the address and what it hands out.
- **The host** of the file (Discord CDN, MediaFire, GitHub…) through its own report button.

## By hand

- Actions › **Domain watch** › Run workflow: searches crt.sh and reads the NRD lists.
- `python services/DomainWatch/watch.py --status domain-status.json --dry` searches both and prints what it would post.
- `--source crtsh` or `--source nrd` searches only one of them (the NRD lists take about 20 seconds and 60 MB).
- `--pattern bloxstrap` (repeatable) searches other words instead, to see what a post looks like. A run with
  `--source nrd --pattern robloxs` on 2026-10-16 would have posted:

  > **New web address that looks like Bloxkeep**
  > A name like Bloxkeep got a certificate or was just registered. If it isn't yours, check what it hands out and
  > report it (docs/domain-watch.md).
  > `robloxshuffle.com` · newly registered, in hagezi NRD · [whois](https://who.is/whois/robloxshuffle.com) · [crt.sh](https://crt.sh/?q=robloxshuffle.com)

## domain-status.json

```json
{
  "schema": 1,
  "names": {
    "bloxkeep-download.com": { "issuer": "Let's Encrypt (R11)", "firstSeen": "2026-10-16T08:12:45",
                               "crtsh": "https://crt.sh/?id=…", "postedAt": "2026-10-16T09:17:30Z" },
    "free-bloxkeep.com": { "list": "WhoisDS 2026-10-15, hagezi NRD", "firstSeen": "2026-10-15",
                           "postedAt": "2026-10-16T09:47:30Z" }
  },
  "failing": { "crtsh": { "since": "…", "told": true } },
  "changedAt": "…"
}
```

A name found in a certificate has `issuer` and `crtsh`; one found in the NRD lists has `list` (which lists, and which
day's for WhoisDS) and `firstSeen`, the earliest of those days. `failing` is there only while a source (`crtsh`,
`hagezi`, `whoisds`) hasn't answered fully. Remove a name from `names` to have it posted again.
