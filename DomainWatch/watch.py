# Watches for web addresses that look like Bloxkeep, and says so on Discord: a site that copies Bloxkeep (it's open
# source) to hand out a cookie-stealing build needs a name and an HTTPS certificate, and both show up in public lists
# within a day or two, usually before the site is advertised. Reported early, Google Safe Browsing, Cloudflare and the
# registrar can take it down before it spreads.
#
# Two sources, each posting a name only once (a name found by both is posted by whichever finds it first):
# - crtsh: Certificate Transparency through crt.sh, every 3 hours. Every certificate is published within minutes, so
#   this is the early one, but crt.sh only answers prefix searches ("bloxkeep%") reliably: a search with a leading
#   wildcard ("%bloxkeep%") comes back empty even when there are matches (seen 2026-10-16), so "getbloxkeep.com" is
#   found only because it's searched by itself, while "bloxkeep-download.com", "bloxkeep.app" and
#   "bloxkeep.anything.xyz" are all found by "bloxkeep%". crt.sh often answers 502 or 404 or times out: a search that
#   fails is tried again next run.
# - nrd: lists of newly registered names, read whole once a day and searched for the patterns anywhere in a name, so
#   "free-bloxkeep.com" is found too, even before it has a certificate. hagezi's NRD list (GPL-3.0, from Stamus Labs
#   data, on GitHub) has every name of the last 7 days, about 450,000 a day; WhoisDS's free daily list (free to reuse,
#   "including for commercial purposes") has 70,000 a day, of which about 1 in 9 isn't in hagezi's.
# Names are remembered once posted, so a failed run loses nothing and nothing is posted twice.
# Run by .github/workflows/domain-watch.yml; what it saw is kept in domain-status.json on the domain-status branch.
#
#   python watch.py --status domain-status.json [--dry] [--source crtsh|nrd ...] [--pattern bloxkeep ...]
import argparse, base64, datetime, io, json, os, re, sys, time, urllib.parse, urllib.request, zipfile

CRTSH = 'https://crt.sh/'
HAGEZI = 'https://raw.githubusercontent.com/hagezi/nrd/main/domains/nrd7.txt'   # 7 days ago to yesterday, ~55 MB
WHOISDS = 'https://www.whoisds.com/whois-database/newly-registered-domains/{}/nrd'  # {} = base64 of "2026-10-15.zip"
UA = 'Bloxkeep-DomainWatch (+https://github.com/minhhoang0122/bloxkeep)'
# crt.sh searches each as the start of a name ("bloxkeep%"): bloxkeep.com, bloxkeep-app.net, bloxkeeper.io,
# bloxkeep.x.y. The NRD lists are searched for each anywhere in a name: free-bloxkeep.com, getbl0xkeep.net.
PATTERNS = ['bloxkeep', 'bl0xkeep', 'bloxkep', 'bloxkeeep', 'blox-keep', 'bloxxkeep',
            'getbloxkeep', 'download-bloxkeep', 'downloadbloxkeep']
SOURCES = ['crtsh', 'nrd']
# Bloxkeep's own addresses: names equal to one of these or under it are never posted. Lower case, no "*.".
OFFICIAL = []
TRIES = 3
WAIT = [20, 60]                      # seconds before the 2nd and 3rd try of a crt.sh search
PAUSE = 5                            # seconds between two searches, to be gentle with crt.sh
TIMEOUT = 90                         # seconds for one answer (a slow one takes 30)
BUDGET = 20 * 60                     # seconds for a whole run: searches left after that are tried next run
WHOISDS_DAYS = 5                     # WhoisDS's lists of the last days read each time: a few missed runs lose nothing
HAGEZI_STALE = datetime.timedelta(days=2)   # hagezi's list not updated for this long counts as not answering
# Said once when a source hasn't answered for this long. hagezi's list covers 7 days, so a short gap loses nothing.
BLIND_AFTER = {'crtsh': datetime.timedelta(days=2), 'hagezi': datetime.timedelta(days=3),
               'whoisds': datetime.timedelta(days=3)}
LABEL = {'crtsh': 'crt.sh', 'hagezi': 'hagezi\'s NRD list', 'whoisds': 'WhoisDS'}
MAX_LISTED = 15
# How the messages look: docs/discord-style.md (sender, Discord's colours, short title, facts in fields).
LOGO = 'https://raw.githubusercontent.com/minhhoang0122/bloxkeep/main/docs/brand/bloxkeep.png'
DOCS = 'https://github.com/minhhoang0122/bloxkeep/blob/main/docs/domain-watch.md'
GREEN, YELLOW, RED = 0x57F287, 0xFEE75C, 0xED4245


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def get(url, timeout=TIMEOUT):
    return urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': UA}), timeout=timeout)


def search(prefix, deadline):
    """crt.sh's unexpired certificates with a name starting with prefix, or None when crt.sh didn't answer."""
    url = CRTSH + '?' + urllib.parse.urlencode({'q': prefix + '%', 'output': 'json', 'deduplicate': 'Y', 'exclude': 'expired'})
    for attempt in range(TRIES):
        if attempt:
            if time.monotonic() + WAIT[attempt - 1] + TIMEOUT > deadline:
                break
            time.sleep(WAIT[attempt - 1])
        try:
            req = urllib.request.Request(url, headers={'User-Agent': UA, 'Accept': 'application/json'})
            with urllib.request.urlopen(req, timeout=TIMEOUT) as r:
                rows = json.loads(r.read())
            if isinstance(rows, list):
                return rows
            print(f'{prefix}: crt.sh answered something other than a list', file=sys.stderr)
        except (OSError, ValueError) as e:
            # 502, 404, timeouts and HTML instead of JSON: crt.sh is busy.
            print(f'{prefix}: try {attempt + 1} of {TRIES}: {e}', file=sys.stderr)
    return None


def matcher(patterns):
    """A name worth posting contains one of the patterns (the other names on a certificate are left out)."""
    return re.compile('|'.join(re.escape(p.lower()) for p in patterns))


def official(name):
    return any(name == d or name.endswith('.' + d) for d in OFFICIAL)


def issuer(text):
    """'C=US, O=Let's Encrypt, CN=R11' -> "Let's Encrypt (R11)"."""
    parts = dict(p.split('=', 1) for p in re.split(r',\s*(?=[A-Z]+=)', text or '') if '=' in p)
    org, cn = parts.get('O'), parts.get('CN')
    return f'{org} ({cn})' if org and cn and cn != org else org or cn or text or '?'


def names_in(rows, wanted):
    """{name: {issuer, firstSeen, crtsh}} for the names in rows that match, each with its earliest certificate."""
    found = {}
    for row in rows:
        when = (row.get('entry_timestamp') or row.get('not_before') or '')[:19]
        for raw in str(row.get('name_value', '')).split():
            name = raw.lower().removeprefix('*.').rstrip('.')
            if not wanted.search(name) or official(name):
                continue
            if name not in found or when < found[name]['firstSeen']:
                found[name] = {'issuer': issuer(row.get('issuer_name')), 'firstSeen': when,
                               'crtsh': f'https://crt.sh/?id={row.get("id")}'}
    return found


def from_crtsh(patterns, wanted):
    """({name: info}, [prefixes crt.sh didn't answer for])."""
    found, failed = {}, []
    deadline = time.monotonic() + BUDGET
    for i, prefix in enumerate(patterns):
        if time.monotonic() + TIMEOUT > deadline:
            failed.append(prefix)
            continue
        if i:
            time.sleep(PAUSE)
        rows = search(prefix, deadline)
        if rows is None:
            failed.append(prefix)
            continue
        for name, info in names_in(rows, wanted).items():
            if name not in found or info['firstSeen'] < found[name]['firstSeen']:
                found[name] = info
        print(f'{prefix}%: {len(rows)} certificates')
    return found, failed


def nrd_add(found, name, wanted, listed, day):
    name = name.strip().lower().rstrip('.')
    if not name or name.startswith('#') or not wanted.search(name) or official(name):
        return
    info = found.setdefault(name, {'list': listed, 'firstSeen': day})
    if listed not in info['list']:
        info['list'] += ', ' + listed
    info['firstSeen'] = min(info['firstSeen'], day)


def from_hagezi(wanted, found):
    """Reads hagezi's 7-day list into found; False when it couldn't be read or hasn't been updated lately."""
    try:
        modified, count = None, 0
        with get(HAGEZI, timeout=120) as r:
            for raw in io.TextIOWrapper(r, encoding='utf-8', errors='replace'):
                if raw.startswith('#'):
                    m = re.match(r'#\s*Last modified:\s*(.+?)\s*UTC', raw)
                    if m:
                        modified = datetime.datetime.strptime(m[1], '%d %b %Y %H:%M').replace(tzinfo=datetime.timezone.utc)
                    continue
                count += 1
                if modified:
                    nrd_add(found, raw, wanted, 'hagezi NRD', modified.strftime('%Y-%m-%d'))
    except (OSError, ValueError) as e:
        print(f'hagezi: {e}', file=sys.stderr)
        return False
    if not modified or count < 100000:
        print(f'hagezi: {count} names, last modified {modified}: not the list it should be', file=sys.stderr)
        return False
    print(f'hagezi: {count} names, last modified {modified:%Y-%m-%d %H:%M} UTC')
    if datetime.datetime.now(datetime.timezone.utc) - modified > HAGEZI_STALE:
        print(f'hagezi: not updated for more than {HAGEZI_STALE.days} days', file=sys.stderr)
        return False
    return True


def from_whoisds(wanted, found):
    """Reads WhoisDS's free lists of the last days into found; False when not one of them could be read."""
    today = datetime.datetime.now(datetime.timezone.utc).date()
    read = 0
    for back in range(WHOISDS_DAYS + 1):
        day = (today - datetime.timedelta(days=back)).isoformat()
        try:
            with get(WHOISDS.format(base64.b64encode(f'{day}.zip'.encode()).decode())) as r:
                body = r.read()
            if not body.startswith(b'PK'):
                continue                 # not out yet (today's), or gone: answered with an empty page
            with zipfile.ZipFile(io.BytesIO(body)) as z:
                names = z.read(z.namelist()[0]).decode('utf-8', 'replace').split()
        except (OSError, ValueError, zipfile.BadZipFile, IndexError) as e:
            print(f'whoisds {day}: {e}', file=sys.stderr)
            continue
        for name in names:
            nrd_add(found, name, wanted, f'WhoisDS {day}', day)
        print(f'whoisds {day}: {len(names)} names')
        read += 1
    return read > 0


def from_nrd(wanted):
    """({name: info}, [lists that didn't answer])."""
    found = {}
    failed = [key for key, read in (('hagezi', from_hagezi), ('whoisds', from_whoisds)) if not read(wanted, found)]
    return found, failed


def stamp(text, style='R'):
    """An ISO time or date as a Discord timestamp, which each reader sees in their own time zone and language."""
    try:
        at = datetime.datetime.fromisoformat(text.replace('Z', '+00:00'))
        if at.tzinfo is None:
            at = at.replace(tzinfo=datetime.timezone.utc)
        return f'<t:{int(at.timestamp())}:{style}>'
    except ValueError:
        return text


def field(name, info):
    """One address as a field: where it was seen, when, and where to look it up."""
    if 'crtsh' in info:
        value = (f'Certificate from {info["issuer"]}\nFirst seen {stamp(info["firstSeen"], "D")}\n'
                 f'[crt.sh]({info["crtsh"]}) · [whois](https://who.is/whois/{name})')
    else:
        value = (f'Newly registered ({info["list"]})\n'
                 f'[whois](https://who.is/whois/{name}) · [crt.sh](https://crt.sh/?q={name})')
    return name, value, True


def fields(names, found):
    out = [field(n, found[n]) for n in names[:MAX_LISTED]]
    if len(names) > MAX_LISTED:
        out.append(('And more', f'{len(names) - MAX_LISTED} more in domain-status.json (domain-status branch)', False))
    return out


def message(title, color, description, fields=()):
    """A webhook post in Bloxkeep's style: one embed, the logo, short fields side by side."""
    embed = {'title': title[:256], 'color': color, 'description': description[:4000],
             'fields': [{'name': n[:256], 'value': v[:1024], 'inline': inline} for n, v, inline in fields],
             'thumbnail': {'url': LOGO},
             'footer': {'text': 'Bloxkeep · Domain watch', 'icon_url': LOGO},
             'timestamp': now()}
    return {'username': 'Bloxkeep', 'avatar_url': LOGO, 'embeds': [embed], 'allowed_mentions': {'parse': []}}


def found_message(names, found):
    title = (f'An address like Bloxkeep turned up: {names[0]}' if len(names) == 1
             else f'{len(names)} addresses like Bloxkeep turned up')
    return message(title, RED, "If it isn't Bloxkeep's, check what it hands out and report it.\n"
                   f'-# [What to do]({DOCS})', fields(names, found))


failed_posts = 0


def post(webhook, payload):
    """A post that fails doesn't stop the status being saved; the run fails after, and the names aren't posted again."""
    global failed_posts
    embed = payload['embeds'][0]
    if not webhook:
        print(f'[discord] {embed["title"]}\n{embed["description"]}',
              *(f'  {f["name"]}: {f["value"]}' for f in embed.get('fields', [])), sep='\n')
        return
    body = json.dumps(payload).encode()
    req = urllib.request.Request(webhook, data=body, method='POST',
                                 headers={'User-Agent': UA, 'Content-Type': 'application/json'})
    try:
        urllib.request.urlopen(req, timeout=30).close()
    except OSError as e:
        print(f'posting to Discord failed: {e}', file=sys.stderr)
        failed_posts += 1


def health(status, key, failing, detail, webhook):
    """A source down: said once after BLIND_AFTER without a full answer, and once when it answers again."""
    known = status.setdefault('failing', {})
    h = known.get(key) or {}
    if failing:
        h.setdefault('since', now())
        since = datetime.datetime.fromisoformat(h['since'].replace('Z', '+00:00'))
        if not h.get('told') and datetime.datetime.now(datetime.timezone.utc) - since >= BLIND_AFTER[key]:
            post(webhook, message(f'Domain watch can\'t reach {LABEL[key]}', YELLOW,
                                  f'No full answer since {stamp(h["since"])}{detail}. New addresses like Bloxkeep may be '
                                  'missed until it answers again.\n-# Said once.'))
            h['told'] = True
        known[key] = h
    else:
        if h.get('told'):
            post(webhook, message(f'Domain watch reaches {LABEL[key]} again', GREEN,
                                  'Anything it missed meanwhile is posted as it\'s found.'))
        known.pop(key, None)
    if not known:
        status.pop('failing')


def check(path, dry, patterns, sources):
    before = json.load(open(path, encoding='utf-8')) if os.path.exists(path) else None
    status = json.loads(json.dumps(before)) if before else {'schema': 1, 'names': {}}
    webhook = None if dry else os.environ.get('DOMAIN_WATCH_WEBHOOK') or os.environ.get('DISCORD_WEBHOOK') or None
    wanted = matcher(patterns)
    found, crt_failed, nrd_failed = {}, [], []
    if 'nrd' in sources:
        found, nrd_failed = from_nrd(wanted)
    if 'crtsh' in sources:
        certs, crt_failed = from_crtsh(patterns, wanted)
        found.update(certs)              # a name with a certificate is shown with it

    new = sorted(n for n in found if n not in status['names'])
    for n in new:
        status['names'][n] = {**found[n], 'postedAt': now()}
    down = [LABEL['crtsh']] * bool(crt_failed) + [LABEL[k] for k in nrd_failed]
    if before is None:
        searched = []
        if 'crtsh' in sources and len(crt_failed) < len(patterns):
            searched.append('certificates for names starting with ' + ', '.join(f'`{p}`' for p in patterns if p not in crt_failed))
        if 'nrd' in sources and len(nrd_failed) < 2:
            searched.append('newly registered names with one of them anywhere')
        note = (f'\n-# {", ".join(down)} didn\'t answer fully this time: tried again next run, and anything found then '
                'is posted as new.') if down else ''
        if new:
            post(webhook, message(f'Domain watch is on: {len(new)} address{"es" if len(new) > 1 else ""} already there', RED,
                                  "Worth a look: if one isn't Bloxkeep's, check what it hands out and report it.\n"
                                  f'-# [What to do]({DOCS})' + note, fields(new, found)))
        else:
            post(webhook, message('Domain watch is on', GREEN,
                                  ('Nothing like Bloxkeep right now. Searched ' + ' and '.join(searched) + '.'
                                   if searched else 'Nothing searched yet.') + note))
    elif new:
        post(webhook, found_message(new, found))

    if 'crtsh' in sources:
        health(status, 'crtsh', crt_failed, f' (this time: {", ".join(crt_failed)})', webhook)
    if 'nrd' in sources:
        for key in ('hagezi', 'whoisds'):
            health(status, key, key in nrd_failed, '', webhook)

    # Saved only when something changed, so the branch gets a commit only then.
    if before is None or {**before, 'changedAt': None} != {**status, 'changedAt': None}:
        status['changedAt'] = now()
        with open(path, 'w', encoding='utf-8', newline='\n') as f:
            json.dump(status, f, indent=2, ensure_ascii=False, sort_keys=True)
            f.write('\n')
    print(f'{len(found)} matching names, {len(new)} new'
          + (f', {len(crt_failed)} of {len(patterns)} crt.sh searches failed' if 'crtsh' in sources else '')
          + (f' ({", ".join(crt_failed)})' if crt_failed else '')
          + (f', lists not read: {", ".join(nrd_failed)}' if nrd_failed else ''))
    return 2 if failed_posts else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--status', required=True)
    ap.add_argument('--dry', action='store_true', help='print what would go to Discord instead of posting it')
    ap.add_argument('--source', action='append', choices=SOURCES,
                    help='search only this source: crtsh (every 3 hours) or nrd (once a day); both by default')
    ap.add_argument('--pattern', action='append', help='search this instead of the usual ones (repeatable)')
    a = ap.parse_args()
    sys.stdout.reconfigure(line_buffering=True, errors='replace')
    return check(a.status, a.dry, a.pattern or PATTERNS, a.source or SOURCES)


if __name__ == '__main__':
    sys.exit(main())
