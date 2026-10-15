# Watches Roblox for what can stop Multi-Roblox working, and says so on Discord:
#   - a new Windows build, as soon as Roblox's beta channel offers it (before release) and again when it goes live:
#     its RobloxPlayerBeta.exe is downloaded and its single-window lock names and code identifiers are compared with
#     the build before (a rename or a new lock means Multi-Roblox may be blocked);
#   - a build uploaded ahead of release whose download is still hidden (DeployHistory.txt);
#   - Roblox's client settings (flags) that switch its single-window check, which change without any new build.
# Nothing here can prove Multi-Roblox works: only two Roblox windows playing together can (Bloxkeep notices that).
# Run by .github/workflows/roblox-watch.yml; what it saw is kept in multi-status.json on the roblox-status branch.
#
#   python watch.py --status multi-status.json [--dry] [--force]   check, write the status, post to Discord
#   python watch.py --fingerprint RobloxPlayerBeta.exe|RobloxApp.zip  print what is compared between builds
import argparse, datetime, hashlib, json, os, re, sys, tempfile, urllib.request, zipfile

LIVE = 'https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer'
# Offers the next build before release (seen 2026-10-16: 0.743.0.7431134 there while 0.743.0.7431129 was live).
BETA = 'https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer/channel/zbeta'
HISTORY = 'https://setup.rbxcdn.com/DeployHistory.txt'
FLAGS = 'https://clientsettingscdn.roblox.com/v2/settings/application/PCDesktopClient'
CDN = 'https://setup.rbxcdn.com'
UA = 'Bloxkeep-RobloxWatch (+https://github.com/minhhoang0122/bloxkeep)'
# The name Bloxkeep takes so Roblox can't set up its single-window check (RobloxManager/MultiInstance.cs).
HELD = 'ROBLOX_singletonEvent'
LOCK_NAME = re.compile(rb'ROBLOX_[A-Za-z0-9_]{3,64}')
LOCK_NAME_WIDE = re.compile(rb'R\x00O\x00B\x00L\x00O\x00X\x00_\x00(?:[A-Za-z0-9_]\x00){3,64}')
# Roblox's single-window code: its source file, log category, telemetry and the flags that tune it.
RELATED = re.compile(rb'(?<![A-Za-z0-9_:])[A-Za-z0-9_:]*(?:SingleInstance|AppReload|WndProcessCheck|EventWaitResult|MutexWaitResult)[A-Za-z0-9_:]*')
FLAG = re.compile(r'SingleInstance|AppReload|WndProcessCheck|Singleton(?:Event|Mutex)|MultipleInstance')
SINGLETON = re.compile(r'(?i)singleton')
FUTURE_LINE = re.compile(r'New WindowsPlayer (\S+) at (.+?), file version: (\d+), (\d+), (\d+), (\d+)')
MAX_BUILDS = 40
# How the messages look: docs/discord-style.md (sender, Discord's colours, short title, facts in inline fields).
LOGO = 'https://raw.githubusercontent.com/minhhoang0122/bloxkeep/main/docs/brand/bloxkeep.png'
DOCS = 'https://github.com/minhhoang0122/bloxkeep/blob/main/docs/roblox-watch.md'
GREEN, YELLOW, RED, BLURPLE = 0x57F287, 0xFEE75C, 0xED4245, 0x5865F2
# DeployHistory.txt's times are Roblox's (US Pacific).
ROBLOX_TIME = 'America/Los_Angeles'


def get(url, headers=None):
    req = urllib.request.Request(url, headers={'User-Agent': UA, **(headers or {})})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def version_key(v):
    return tuple(int(x) for x in re.findall(r'\d+', v))


def fingerprint(exe):
    locks = {m.group().decode() for m in LOCK_NAME.finditer(exe)}
    locks |= {m.group().decode('utf-16le') for m in LOCK_NAME_WIDE.finditer(exe)}
    related = {m.group().decode() for m in RELATED.finditer(exe)}
    return {'locks': sorted(locks), 'related': sorted(related)}


def download_player(build):
    """RobloxPlayerBeta.exe of a build, from Roblox's CDN as Bloxstrap gets it; the zip is checked against the manifest."""
    manifest = get(f'{CDN}/{build}-rbxPkgManifest.txt').decode().split()
    i = manifest.index('RobloxApp.zip')
    md5 = manifest[i + 1]
    with tempfile.TemporaryFile() as f:
        h = hashlib.md5()
        req = urllib.request.Request(f'{CDN}/{build}-RobloxApp.zip', headers={'User-Agent': UA})
        with urllib.request.urlopen(req, timeout=300) as r:
            while chunk := r.read(1 << 20):
                h.update(chunk)
                f.write(chunk)
        if h.hexdigest() != md5:
            raise RuntimeError(f'RobloxApp.zip of {build} does not match its manifest')
        f.seek(0)
        return zipfile.ZipFile(f).read('RobloxPlayerBeta.exe')


def latest_upload():
    """The newest Windows build in Roblox's deploy history (its hash is hidden until release): (version, when)."""
    tail = get(HISTORY, {'Range': 'bytes=-4000'}).decode(errors='replace')
    found = FUTURE_LINE.findall(tail)
    if not found:
        return None
    _, at, *parts = found[-1]
    return '.'.join(parts), at


def single_window_flags():
    settings = json.loads(get(FLAGS)).get('applicationSettings', {})
    return {k: v for k, v in sorted(settings.items()) if FLAG.search(k)}


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def holds(fp, before):
    """The lock names Bloxkeep should hold for a build (the app reads them): the ones held for the build before while
    they're still there, else its own; when that is gone too and exactly one new singleton name came in its place,
    that one (Roblox renamed its lock). Empty when there's no telling."""
    prev = (before or {}).get('hold') or [HELD]
    kept = [n for n in prev if n in fp['locks']]
    if kept:
        return kept
    if HELD in fp['locks']:
        return [HELD]
    new = [n for n in fp['locks'] if SINGLETON.search(n) and n not in (before or {}).get('locks', [])]
    return new if len(new) == 1 else []


def judge(fp, before):
    """(verdict, names to hold, names added, names removed) for a build against the build seen before it."""
    hold = holds(fp, before)
    if before is None:
        return ('first' if hold else 'gone'), hold, [], []
    added = sorted(set(fp['locks'] + fp['related']) - set(before['locks'] + before['related']))
    removed = sorted(set(before['locks'] + before['related']) - set(fp['locks'] + fp['related']))
    if not hold:
        return 'gone', hold, added, removed
    if hold != (before.get('hold') or [HELD]):
        return 'renamed', hold, added, removed
    return ('changed' if added or removed else 'same'), hold, added, removed


# What a verdict means to someone using Multi-Roblox: (the word for it, the colour, one plain sentence).
VERDICTS = {
    'first': ('Watching', BLURPLE, 'This is the first build the watch has seen, so there is nothing to compare it with yet.'),
    'same': ('No change', GREEN, 'Nothing changed in the lock Multi-Roblox uses, so it should keep working.'),
    'changed': ('May be blocked', YELLOW, 'Roblox changed the code that keeps it to one window. Try two windows before '
                'counting on Multi-Roblox.'),
    'renamed': ('Lock renamed', YELLOW, 'Roblox renamed the lock Multi-Roblox uses. Bloxkeep now holds the new name by '
                'itself; try two windows to be sure.'),
    'gone': ('Likely blocked', RED, 'The lock Multi-Roblox uses is gone from this build, so Roblox will probably close '
             'each window when the next one opens.'),
}


def stamp(when=None, style='f'):
    """A Discord timestamp: each reader sees it in their own time zone and language."""
    when = when or datetime.datetime.now(datetime.timezone.utc)
    return f'<t:{int(when.timestamp())}:{style}>'


def roblox_time(text):
    """A DeployHistory.txt time ("10/15/2026 11:43:03 AM", Roblox's), as a Discord timestamp; as written if unreadable."""
    try:
        from zoneinfo import ZoneInfo
        at = datetime.datetime.strptime(text, '%m/%d/%Y %I:%M:%S %p').replace(tzinfo=ZoneInfo(ROBLOX_TIME))
        return stamp(at, 'f')
    except Exception:
        return f'{text} (Roblox time)'


def release_notes(version):
    """Roblox's release notes for a version ("0.743.0.7431134" is release 743)."""
    m = re.match(r'0\.(\d+)\.', version)
    return f'https://create.roblox.com/docs/release-notes/release-notes-{m.group(1)}' if m else None


def lines(names, sign, limit=6):
    shown = [f'{sign} `{n}`' for n in names[:limit]]
    return '\n'.join(shown + ([f'and {len(names) - limit} more'] if len(names) > limit else []))


def message(title, color, description, fields=()):
    """A webhook post in Bloxkeep's style: one embed, the logo, short fields side by side."""
    embed = {'title': title, 'color': color, 'description': description,
             'fields': [{'name': n, 'value': v[:1024], 'inline': inline} for n, v, inline in fields],
             'thumbnail': {'url': LOGO},
             'footer': {'text': 'Bloxkeep · Roblox watch', 'icon_url': LOGO},
             'timestamp': now()}
    return {'username': 'Bloxkeep', 'avatar_url': LOGO, 'embeds': [embed], 'allowed_mentions': {'parse': []}}


def build_message(kind, version, build, verdict, added=(), removed=(), hold=None, checked_at=None):
    """A build's check. kind: 'beta' (before release), 'live' (checked as it went live), 'went-live' (checked before)."""
    word, color, sentence = VERDICTS[verdict]
    if verdict == 'renamed' and hold:
        sentence = (f'Roblox renamed the lock Multi-Roblox uses. Bloxkeep now holds the new one, `{hold[0]}`, by itself; '
                    'try two windows to be sure.')
    title = f'Roblox {version} is coming' if kind == 'beta' else f'Roblox {version} is live'
    small = {'beta': ["Checked on Roblox's test channel"], 'went-live': [f'Checked before release, {checked_at}' if checked_at
             else 'Checked before release'], 'live': []}[kind]
    small.append(f'Build `{build.removeprefix("version-")}`')
    if notes := release_notes(version):
        small.append(f'[Release notes]({notes})')
    small.append(f'[How the check works]({DOCS})')
    # Three to a row in Discord, beside the logo too.
    fields = [('Multi-Roblox', f'**{word}**', True), ('Version', f'`{version}`', True),
              ('Released', 'Not yet' if kind == 'beta' else stamp(style='R'), True)]
    if added or removed:
        fields.append(('What changed', '\n'.join(x for x in (lines(added, '+'), lines(removed, '−')) if x), False))
    return message(title, color, f'{sentence}\n-# {" · ".join(small)}', fields)


def upload_message(version, uploaded):
    return message(f'Roblox {version} is on its way', BLURPLE,
                   "Roblox uploaded it but hasn't released it, and its download is still hidden. "
                   'Bloxkeep checks it as soon as Roblox offers it.',
                   [('Version', f'`{version}`', True), ('Uploaded', roblox_time(uploaded), True)])


def flags_message(changes):
    """changes: (setting, before, after), None for missing."""
    return message('Roblox changed a one-window setting', YELLOW,
                   'Roblox can turn its one-window check on or off from its servers, without a new build. '
                   'Try two windows to be sure Multi-Roblox still works.',
                   [(name, f'`{old if old is not None else "none"}` → `{new if new is not None else "removed"}`', False)
                    for name, old, new in changes[:8]])


failed_posts = 0


def post(webhook, payload):
    """A post that fails (a deleted webhook, Discord down) doesn't stop the status being saved; the run fails after."""
    global failed_posts
    embed = payload['embeds'][0]
    if not webhook:
        print(f'[discord] {embed["title"]}: {embed.get("description", "")}',
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


def inspect(status, version, build, channel):
    """Downloads a build, compares it with the build seen before it and adds it to the status: its message, by kind."""
    fp = fingerprint(download_player(build))
    before = next((b for b in reversed(status['builds']) if b['hash'] != build), None)
    verdict, hold, added, removed = judge(fp, before)
    entry = {'version': version, 'hash': build, 'channel': channel, 'seenAt': now(), 'verdict': verdict, 'hold': hold,
             **fp, 'added': added, 'removed': removed}
    if channel == 'live':
        entry['liveAt'] = entry['seenAt']
    status['builds'] = ([b for b in status['builds'] if b['hash'] != build] + [entry])[-MAX_BUILDS:]
    return lambda kind: build_message(kind, version, build, verdict, added, removed, hold)


def check(path, dry, force):
    status = json.load(open(path, encoding='utf-8')) if os.path.exists(path) else {'schema': 1, 'builds': []}
    webhook = None if dry else os.environ.get('DISCORD_WEBHOOK') or None
    seen = lambda build: next((b for b in status['builds'] if b['hash'] == build), None)
    changed = False
    live = json.loads(get(LIVE))
    version, build = live['version'], live['clientVersionUpload']
    try:
        beta = json.loads(get(BETA))
    except OSError as e:
        print(f'beta channel: {e}', file=sys.stderr)
        beta = None
    if beta and version_key(beta['version']) <= version_key(version):
        beta = None

    # The next build, before release: when it goes live it isn't downloaded again.
    if beta and (seen(beta['clientVersionUpload']) is None or force):
        post(webhook, inspect(status, beta['version'], beta['clientVersionUpload'], 'beta')('beta'))
        changed = True
    if beta:
        status['beta'] = {'version': beta['version'], 'hash': beta['clientVersionUpload']}

    entry = seen(build)
    if entry is None or force:
        post(webhook, inspect(status, version, build, 'live')('live'))
        changed = True
    elif 'liveAt' not in entry:
        entry['liveAt'] = now()
        seen_at = datetime.datetime.strptime(entry['seenAt'], '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=datetime.timezone.utc)
        post(webhook, build_message('went-live', version, build, entry['verdict'], entry.get('added', []),
                                    entry.get('removed', []), entry.get('hold'), stamp(seen_at, 'R')))
        changed = True
    if status.get('live') != {'version': version, 'hash': build}:
        status['live'] = {'version': version, 'hash': build}
        changed = True

    # Uploaded with its download hidden: said once, unless the beta channel already offered that version.
    upload = latest_upload()
    if upload and version_key(upload[0]) > version_key(version) and (status.get('future') or {}).get('version') != upload[0]:
        status['future'] = {'version': upload[0], 'uploadedAt': upload[1]}
        if not any(b['version'] == upload[0] for b in status['builds']):
            post(webhook, upload_message(upload[0], upload[1]))
        changed = True

    flags = single_window_flags()
    before = status.get('flags')
    if flags != before:
        if before is not None:
            post(webhook, flags_message([(k, before.get(k), flags.get(k)) for k in sorted(set(flags) | set(before))
                                         if flags.get(k) != before.get(k)]))
        status['flags'] = flags
        changed = True

    if changed:
        status['changedAt'] = now()
        with open(path, 'w', encoding='utf-8', newline='\n') as f:
            json.dump(status, f, indent=2, ensure_ascii=False)
            f.write('\n')
    print(f'live {version} {build}' + (f', beta {beta["version"]}' if beta else '') + f', {"changed" if changed else "nothing new"}')
    return 2 if failed_posts else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--status')
    ap.add_argument('--fingerprint')
    ap.add_argument('--dry', action='store_true', help='print what would go to Discord instead of posting it')
    ap.add_argument('--force', action='store_true', help='check the live build again even if it was checked before')
    a = ap.parse_args()
    if a.fingerprint:
        blob = zipfile.ZipFile(a.fingerprint).read('RobloxPlayerBeta.exe') if a.fingerprint.endswith('.zip') else open(a.fingerprint, 'rb').read()
        print(json.dumps(fingerprint(blob), indent=2))
    elif a.status:
        return check(a.status, a.dry, a.force)
    else:
        ap.error('--status or --fingerprint')


if __name__ == '__main__':
    sys.exit(main())
