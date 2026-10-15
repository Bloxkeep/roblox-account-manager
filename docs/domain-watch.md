# Domain watch: lookalike Bloxkeep sites

Bloxkeep is open source, so anyone can build a copy with a cookie stealer inside and hand it out from a site that
looks like Bloxkeep's, the way fake "Multi Roblox" sites are advertised on YouTube. Such a site needs an HTTPS
certificate, and every certificate is published in Certificate Transparency (CT) logs within minutes of being issued,
usually before the site is advertised anywhere. This watch posts each new name like Bloxkeep to Discord once, so it
can be reported while it's still new. Nothing runs on your PC.

| What | Where | How often |
|---|---|---|
| Searches crt.sh (a public search over the CT logs) and posts new names to Discord | GitHub Actions, `.github/workflows/domain-watch.yml` running `DomainWatch/watch.py` | every 3 hours |
| Remembers the names it posted | `domain-status.json` on the `domain-status` branch (made by the first run) | |

There's no Cloudflare part, unlike the [Roblox watch](roblox-watch.md): a few hours is early enough here, and crt.sh
itself is minutes to hours behind the logs.

## What it finds and what it can't

- **Names starting with** `bloxkeep`, `bl0xkeep`, `bloxkep`, `bloxkeeep`, `blox-keep`, `bloxxkeep`,
  `getbloxkeep`, `download-bloxkeep` and `downloadbloxkeep` (`PATTERNS` in `DomainWatch/watch.py`). `bloxkeep` alone
  covers `bloxkeep.com`, `bloxkeep.app`, `bloxkeep-download.net`, `bloxkeeper.io` and `bloxkeep.anything.xyz`.
- **Not names with Bloxkeep in the middle**, like `free-bloxkeep.com`, unless that start is in the list. crt.sh only
  answers searches by the start of a name: a search like `%bloxkeep%` comes back empty even when there are matches
  (tried 2026-10-16 with a name that has dozens of certificates). Add a start to `PATTERNS` when you see scammers use
  one.
- **Not sites on a shared address** such as `bloxkeep.pages.dev`, `bloxkeep.vercel.app`, `bloxkeep.netlify.app` or
  `something.github.io/bloxkeep`: those use the host's one certificate for every site, so no new name shows up in CT.
  Search for them by hand now and then, and search YouTube for "Bloxkeep download".
- **Not letters that only look alike** in other alphabets (an `о` that is Cyrillic): their names are written as
  `xn--…` in certificates.
- Each name is posted once with its issuer (who gave the certificate: Let's Encrypt, Google, Cloudflare…), the day it
  was first seen in the logs, and a crt.sh link to the certificate. The other names on the same certificate aren't
  posted. Only certificates still valid are searched.
- Bloxkeep's own addresses go in `OFFICIAL` in `DomainWatch/watch.py` and are never posted, nor anything under them.
  There are none yet: the official home is the GitHub repository, which never shows in CT.

## When crt.sh fails

crt.sh often answers 502 or 404 or takes over a minute. Each search is tried 3 times (after 20 s, then 60 s), and a
run stops searching after 20 minutes. A search that still fails is simply done again next run: a name is remembered
only once posted, so a missed run loses nothing and nothing is posted twice. The run doesn't fail for it, so GitHub
doesn't email you every 3 hours. If crt.sh hasn't answered every search for two days, one message says so; one more
says when it answers again.

## Setting it up

1. **Discord.** It posts with the `DISCORD_WEBHOOK` secret the Roblox watch uses (setup in
   [roblox-watch.md](roblox-watch.md)). To post in another channel, make a webhook there (Edit Channel › Integrations ›
   Webhooks › New Webhook › Copy Webhook URL) and save it as the secret `DOMAIN_WATCH_WEBHOOK`. Secrets go in
   Settings › Secrets and variables › Actions, never in a file or a chat.
2. Once the workflow is on `main`, Actions › **Domain watch** › Run workflow. The first run posts *Domain watch
   started* with any names already in the logs and makes the `domain-status` branch. From then on it runs every 3
   hours (GitHub runs schedules only from `main`, and pauses them after 60 days without a commit to the repository:
   Actions › Domain watch › Enable workflow brings it back).

## When it posts a name

Open the crt.sh link to see the whole certificate, then look at the site from a browser you don't use for Roblox (or
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

- Actions › **Domain watch** › Run workflow.
- `python DomainWatch/watch.py --status domain-status.json --dry` searches and prints what it would post.
- `--pattern bloxstrap` (repeatable) searches other starts instead, to see what a post looks like.

## domain-status.json

```json
{
  "schema": 1,
  "names": {
    "bloxkeep-download.com": { "issuer": "Let's Encrypt (R11)", "firstSeen": "2026-10-16T08:12:45",
                               "crtsh": "https://crt.sh/?id=…", "postedAt": "2026-10-16T09:17:30Z" }
  },
  "crtsh": { "failingSince": "…", "told": true },
  "changedAt": "…"
}
```

`crtsh` is there only while crt.sh hasn't answered every search. Remove a name from `names` to have it posted again.
