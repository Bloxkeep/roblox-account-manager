# Security policy

## Supported versions

Only the newest release gets security fixes. Bloxkeep updates itself, so keeping it open (or opening it now and then)
is enough to stay current.

## Report a vulnerability

Please don't open a public issue for a security problem. Report it privately on GitHub instead:
**Security › Report a vulnerability** in this repository
([direct link](https://github.com/minhhoang0122/Bloxkeep/security/advisories/new)).

Include, if you can:

- the Bloxkeep version and your Windows version;
- what someone could do with the problem (read accounts, run code, change an update…);
- the steps to show it, and any proof of concept.

Never include a real cookie, password or 2-step code, yours or anyone else's.

### What counts

- The vault: its encryption, the master password, auto-lock, backups and exports.
- How Bloxkeep keeps and uses Roblox cookies, passwords and authenticator keys.
- Updates and downloads: anything that could make Bloxkeep install a file that isn't from this repository's releases.
- The Discord webhook link, the feedback form, and anything else Bloxkeep sends over the network.

Problems in Roblox itself, or on a PC that malware already controls, are out of scope.

### What to expect

Bloxkeep is a one-person project: you'll get an answer within a few days. A confirmed problem is fixed in the next
release, or sooner as a quiet update, and you're credited in the notes if you'd like. There is no bug bounty.

## Fake copies and scams

Bloxkeep is only published here, on [GitHub Releases](https://github.com/minhhoang0122/Bloxkeep/releases), and on its
website, [minhhoang0122.github.io/Bloxkeep](https://minhhoang0122.github.io/Bloxkeep/). Check a download against
`SHA256SUMS.txt` in the same release.

Nobody from Bloxkeep will ever ask for your cookie, password or 2-step code, in Discord, in a direct message or anywhere
else. Bloxkeep is not a browser extension: "Multi Roblox" extensions and ZIP files shared in videos are often built to
steal Roblox cookies.
