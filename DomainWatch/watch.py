# Watches Certificate Transparency for web addresses that look like Bloxkeep, and says so on Discord: a site that
# copies Bloxkeep (it's open source) to hand out a cookie-stealing build needs an HTTPS certificate, and every
# certificate is published in CT logs within minutes of being issued, usually before the site is advertised.
# Reported early, Google Safe Browsing, Cloudflare and the registrar can take it down before it spreads.
#
# Asks crt.sh for names starting with "bloxkeep" and a few lookalikes. crt.sh only answers prefix searches
# ("bloxkeep%") reliably: a search with a leading wildcard ("%bloxkeep%") comes back empty even when there are
# matches (seen 2026-10-16), so "getbloxkeep.com" is found only because it's searched by itself, while
# "bloxkeep-download.com", "bloxkeep.app" and "bloxkeep.anything.xyz" are all found by "bloxkeep%".
# crt.sh often answers 502 or 404 or times out: a search that fails is tried again next run, and names are
# remembered once posted, so nothing is lost and nothing is posted twice.
# Run by .github/workflows/domain-watch.yml; what it saw is kept in domain-status.json on the domain-status branch.
#
#   python watch.py --status domain-status.json [--dry] [--pattern bloxkeep ...]
import argparse, datetime, json, os, re, sys, time, urllib.parse, urllib.request

CRTSH = 'https://crt.sh/'
UA = 'Bloxkeep-DomainWatch (+https://github.com/minhhoang0122/bloxkeep)'
# Each is searched as the start of a name ("bloxkeep%"): bloxkeep.com, bloxkeep-app.net, bloxkeeper.io, bloxkeep.x.y.
PATTERNS = ['bloxkeep', 'bl0xkeep', 'bloxkep', 'bloxkeeep', 'blox-keep', 'bloxxkeep',
            'getbloxkeep', 'download-bloxkeep', 'downloadbloxkeep']
# Bloxkeep's own addresses: names equal to one of these or under it are never posted. Lower case, no "*.".
OFFICIAL = []
TRIES = 3
WAIT = [20, 60]                      # seconds before the 2nd and 3rd try of a search
PAUSE = 5                            # seconds between two searches, to be gentle with crt.sh
TIMEOUT = 90                         # seconds for one answer (a slow one takes 30)
BUDGET = 20 * 60                     # seconds for a whole run: searches left after that are tried next run
BLIND_AFTER = datetime.timedelta(days=2)
MAX_LISTED = 15
COLORS = {'new': 0xD9453D, 'first': 0x2E9E5B, 'blind': 0xE0A030, 'back': 0x2E9E5B}


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


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


def listing(names, found):
    lines = [f'`{n}` · {found[n]["issuer"]} · first seen {found[n]["firstSeen"][:10]} · [crt.sh]({found[n]["crtsh"]})'
             for n in names[:MAX_LISTED]]
    if len(names) > MAX_LISTED:
        lines.append(f'and {len(names) - MAX_LISTED} more (domain-status.json on the domain-status branch)')
    return '\n'.join(lines)


failed_posts = 0


def post(webhook, title, text, color):
    """A post that fails doesn't stop the status being saved; the run fails after, and the names aren't posted again."""
    global failed_posts
    if not webhook:
        print(f'[discord] {title}\n{text}')
        return
    embed = {'title': title, 'description': text[:4000], 'color': color,
             'footer': {'text': 'Bloxkeep domain watch'}, 'timestamp': now()}
    body = json.dumps({'embeds': [embed], 'allowed_mentions': {'parse': []}}).encode()
    req = urllib.request.Request(webhook, data=body, method='POST',
                                 headers={'User-Agent': UA, 'Content-Type': 'application/json'})
    try:
        urllib.request.urlopen(req, timeout=30).close()
    except OSError as e:
        print(f'posting to Discord failed: {e}', file=sys.stderr)
        failed_posts += 1


def check(path, dry, patterns):
    before = json.load(open(path, encoding='utf-8')) if os.path.exists(path) else None
    status = json.loads(json.dumps(before)) if before else {'schema': 1, 'names': {}}
    webhook = None if dry else os.environ.get('DOMAIN_WATCH_WEBHOOK') or os.environ.get('DISCORD_WEBHOOK') or None
    wanted = matcher(patterns)
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

    new = sorted(n for n in found if n not in status['names'])
    for n in new:
        status['names'][n] = {**found[n], 'postedAt': now()}
    if before is None:
        searched = ', '.join(f'`{p}`' for p in patterns if p not in failed)
        text = (f'Already in Certificate Transparency logs ({len(new)}), worth a look:\n' + listing(new, found) if new
                else f'No web address starting with {searched} has a certificate now.')
        if failed:
            text += (f'\ncrt.sh didn\'t answer for {", ".join(f"`{p}`" for p in failed)} this time: tried again next '
                     'run, and anything found then is posted as new.')
        post(webhook, 'Domain watch started', text, COLORS['new' if new else 'first'])
    elif new:
        post(webhook, 'New web address that looks like Bloxkeep',
             'A certificate was issued for a name like Bloxkeep. If it isn\'t yours, check what it hands out and report '
             'it (docs/domain-watch.md).\n' + listing(new, found), COLORS['new'])

    # crt.sh down: said once after two days without a full answer, and once when it answers again.
    health = status.get('crtsh') or {}
    if failed:
        health.setdefault('failingSince', now())
        since = datetime.datetime.fromisoformat(health['failingSince'].replace('Z', '+00:00'))
        if not health.get('told') and datetime.datetime.now(datetime.timezone.utc) - since >= BLIND_AFTER:
            post(webhook, 'Domain watch can\'t reach crt.sh', 'crt.sh hasn\'t answered every search since '
                 f'{health["failingSince"][:16].replace("T", " ")} UTC (this time: {", ".join(failed)}). New lookalike '
                 'addresses may be missed until it does. Said once.', COLORS['blind'])
            health['told'] = True
        status['crtsh'] = health
    else:
        if health.get('told'):
            post(webhook, 'Domain watch reaches crt.sh again', 'Every search answered again; anything issued '
                 'meanwhile is posted as it\'s found.', COLORS['back'])
        status.pop('crtsh', None)

    # Saved only when something changed, so the branch gets a commit only then.
    if before is None or {**before, 'changedAt': None} != {**status, 'changedAt': None}:
        status['changedAt'] = now()
        with open(path, 'w', encoding='utf-8', newline='\n') as f:
            json.dump(status, f, indent=2, ensure_ascii=False, sort_keys=True)
            f.write('\n')
    print(f'{len(found)} matching names, {len(new)} new, {len(failed)} of {len(patterns)} searches failed'
          + (f' ({", ".join(failed)})' if failed else ''))
    return 2 if failed_posts else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--status', required=True)
    ap.add_argument('--dry', action='store_true', help='print what would go to Discord instead of posting it')
    ap.add_argument('--pattern', action='append', help='search this prefix instead of the usual ones (repeatable)')
    a = ap.parse_args()
    sys.stdout.reconfigure(line_buffering=True, errors='replace')
    return check(a.status, a.dry, a.pattern or PATTERNS)


if __name__ == '__main__':
    sys.exit(main())
