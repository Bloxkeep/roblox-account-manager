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
COLORS = {'same': 0x2E9E5B, 'first': 0x2E9E5B, 'changed': 0xE0A030, 'renamed': 0xE0A030, 'gone': 0xD9453D, 'future': 0x5B7FD9, 'flags': 0xE0A030}


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


VERDICT_TEXT = {
    'first': 'Watching starts with this build. Nothing to compare yet.',
    'same': "No change to Roblox's single-window lock or its code names. Multi-Roblox should keep working "
            '(proven only once two windows play together).',
    'changed': "Roblox changed its single-window code in this build. Multi-Roblox may be blocked: try two windows.",
    'renamed': 'Roblox renamed its single-window lock in this build. Bloxkeep holds the new name by itself (it reads '
               "this watch's findings), but that isn't proven yet: try two windows.",
    'gone': f'The lock Bloxkeep holds ({HELD}) is gone from this build, with nothing to put in its place. '
            'Multi-Roblox is likely blocked.',
}


def listing(names, limit=12):
    shown = ', '.join(f'`{n}`' for n in names[:limit])
    return shown + (f' and {len(names) - limit} more' if len(names) > limit else '')


failed_posts = 0


def post(webhook, title, text, color, fields=()):
    """A post that fails (a deleted webhook, Discord down) doesn't stop the status being saved; the run fails after."""
    global failed_posts
    if not webhook:
        print(f'[discord] {title}: {text}', *(f'  {n}: {v}' for n, v in fields), sep='\n')
        return
    embed = {'title': title, 'description': text, 'color': color,
             'fields': [{'name': n, 'value': v[:1024], 'inline': False} for n, v in fields],
             'footer': {'text': 'Bloxkeep Roblox watch'}, 'timestamp': now()}
    body = json.dumps({'embeds': [embed], 'allowed_mentions': {'parse': []}}).encode()
    req = urllib.request.Request(webhook, data=body, method='POST',
                                 headers={'User-Agent': UA, 'Content-Type': 'application/json'})
    try:
        urllib.request.urlopen(req, timeout=30).close()
    except OSError as e:
        print(f'posting to Discord failed: {e}', file=sys.stderr)
        failed_posts += 1


def inspect(status, version, build, channel):
    """Downloads a build, compares it with the build seen before it and adds it to the status: the post's fields."""
    fp = fingerprint(download_player(build))
    before = next((b for b in reversed(status['builds']) if b['hash'] != build), None)
    verdict, hold, added, removed = judge(fp, before)
    entry = {'version': version, 'hash': build, 'channel': channel, 'seenAt': now(), 'verdict': verdict, 'hold': hold,
             **fp, 'added': added, 'removed': removed}
    if channel == 'live':
        entry['liveAt'] = entry['seenAt']
    status['builds'] = ([b for b in status['builds'] if b['hash'] != build] + [entry])[-MAX_BUILDS:]
    fields = [('Version', f'`{version}`'), ('Hash', f'`{build}`'), ('Lock names', listing(fp['locks']))]
    if hold != [HELD]:
        fields.append(('Bloxkeep holds', listing(hold) if hold else '(nothing found to hold)'))
    if added:
        fields.append(('New in this build', listing(added)))
    if removed:
        fields.append(('Gone in this build', listing(removed)))
    return verdict, fields


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
        verdict, fields = inspect(status, beta['version'], beta['clientVersionUpload'], 'beta')
        post(webhook, 'Upcoming Roblox update: Multi-Roblox check',
             "Not live yet: checked on Roblox's beta channel before release. " + VERDICT_TEXT[verdict], COLORS[verdict], fields)
        changed = True
    if beta:
        status['beta'] = {'version': beta['version'], 'hash': beta['clientVersionUpload']}

    entry = seen(build)
    if entry is None or force:
        verdict, fields = inspect(status, version, build, 'live')
        post(webhook, 'Live Roblox update: Multi-Roblox check', VERDICT_TEXT[verdict], COLORS[verdict], fields)
        changed = True
    elif 'liveAt' not in entry:
        entry['liveAt'] = now()
        post(webhook, 'Roblox update is live', 'Checked before release: ' + VERDICT_TEXT[entry['verdict']],
             COLORS[entry['verdict']], [('Version', f'`{version}`'), ('Hash', f'`{build}`')])
        changed = True
    if status.get('live') != {'version': version, 'hash': build}:
        status['live'] = {'version': version, 'hash': build}
        changed = True

    # Uploaded with its download hidden: said once, unless the beta channel already offered that version.
    upload = latest_upload()
    if upload and version_key(upload[0]) > version_key(version) and (status.get('future') or {}).get('version') != upload[0]:
        status['future'] = {'version': upload[0], 'uploadedAt': upload[1]}
        if not any(b['version'] == upload[0] for b in status['builds']):
            post(webhook, 'Future Roblox update uploaded', "Roblox uploaded a Windows build that isn't live yet. Its "
                 'download is still hidden: it is checked as soon as Roblox offers it.', COLORS['future'],
                 [('Version', f'`{upload[0]}`'), ('Uploaded', f'{upload[1]} (Roblox time)')])
        changed = True

    flags = single_window_flags()
    before = status.get('flags')
    if flags != before:
        if before is not None:
            lines = [f'`{k}`: {before.get(k, "(none)")} → {flags.get(k, "(removed)")}'
                     for k in sorted(set(flags) | set(before)) if flags.get(k) != before.get(k)]
            post(webhook, 'Roblox changed a single-window setting', 'Roblox can switch its single-window check from its '
                 'servers, without a new build. Multi-Roblox may be affected: try two windows.', COLORS['flags'],
                 [('Changed', '\n'.join(lines))])
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
