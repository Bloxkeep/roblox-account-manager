// Do the pieces of Bloxkeep's interface agree on the words they share? A report, not a gate: it reads the sources (no browser,
// no build, no app) and says where the C# host, the page and the fake host used by Tests/ui drifted apart.
//   node Tests/ui/contract.mjs            the report; the exit code is 0
//   node Tests/ui/contract.mjs --strict   the same, the exit code is the number of problems found (1 to 99)
//   node Tests/ui/contract.mjs --all      no limit on how many names are listed under each heading
//
// 1. Commands. The C# switch (AppController.DispatchCommandAsync) must know every command the page sends, and the fake host
//    (ui/dev/*-mock.js) must not answer a command nobody sends: dispatch ⊇ page ⊇ mock.
// 2. The lock screen's allow-list (AppController.ScreenLockCommands) names only commands the switch has.
// 3. Words. Every key the page asks for exists in English and in Vietnamese, and the two languages have the same keys.
//
// What it cannot see: a command named by a variable (listed as "chosen at run time"), an argument renamed on one side.
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const strict = process.argv.includes('--strict');
const limit = process.argv.includes('--all') ? Infinity : 25;
const read = path => readFileSync(join(root, path), 'utf8');

/** Files under a folder with one of the endings, without node_modules. */
function walk(folder, endings, found = []) {
  const full = join(root, folder);
  if (!existsSync(full)) return found;
  for (const name of readdirSync(full)) {
    const path = join(folder, name);
    if (name === 'node_modules' || name === 'out') continue;
    if (statSync(join(root, path)).isDirectory()) walk(path, endings, found);
    else if (endings.some(e => name.endsWith(e))) found.push(path.replaceAll('\\', '/'));
  }
  return found;
}

let problems = 0;
function section(title, items, problem = true, note = '') {
  const unique = [...new Set(items)];
  console.log(`\n${title}: ${unique.length}${note ? '  (' + note + ')' : ''}`);
  for (const item of unique.slice(0, limit)) console.log('  ' + item);
  if (unique.length > limit) console.log(`  … and ${unique.length - limit} more (--all lists them)`);
  if (problem) problems += unique.length;
}

// ── The C# side ────────────────────────────────────────────────────────────────────────────────────────────────────────
const controller = read('RobloxManager/AppController.cs');
const switchStart = controller.indexOf('Task<object?> DispatchCommandAsync(');
const switchEnd = controller.indexOf('\n    };', switchStart);
if (switchStart < 0 || switchEnd < 0) {
  console.log('Could not find DispatchCommandAsync in RobloxManager/AppController.cs: the report below is empty.');
  process.exit(strict ? 1 : 0);
}
const host = new Set([...controller.slice(switchStart, switchEnd).matchAll(/^ {8}"(\w+)" =>/gm)].map(m => m[1]));
const lockMatch = controller.match(/ScreenLockCommands\s*=\s*\[([^\]]*)\]/);
const allowed = lockMatch ? [...lockMatch[1].matchAll(/"(\w+)"/g)].map(m => m[1]) : [];

// ── The page ───────────────────────────────────────────────────────────────────────────────────────────────────────────
const pageFiles = [
  ...walk('ui/src', ['.ts', '.tsx']),
  ...walk('RobloxManager/wwwroot', ['.js', '.html']).filter(f => !/\/(ui|i18n|livescene)\.js$/.test(f)),
];
// call('name' … ask('name' … bridge.call('name' … host.call('name', also a choice: call(x ? 'a' : 'b'. (The small windows' own
// messages to their host, post('act') and the like, go to other C# code and are not commands of this switch.)
const direct = /\b(?:call|ask)\(\s*(['"`])([A-Za-z]\w*)\1/g;
const choice = /\b(?:call|ask)\(\s*[^'"`()]{1,60}\?\s*(['"`])([A-Za-z]\w*)\1\s*:\s*(['"`])([A-Za-z]\w*)\3/g;
const runTime = [];
const page = new Map(); // command → files
function sawCommand(name, file) {
  if (!page.has(name)) page.set(name, new Set());
  page.get(name).add(file);
}
for (const file of pageFiles) {
  const text = read(file);
  for (const m of text.matchAll(direct)) sawCommand(m[2], file);
  for (const m of text.matchAll(choice)) { sawCommand(m[2], file); sawCommand(m[4], file); }
  for (const m of text.matchAll(/\b(?:call|ask)\(\s*([a-z_]\w*(?:\.\w+)*)\s*[,)]/g))
    if (!['cmd', 'name'].includes(m[1]) || file.endsWith('.tsx')) runTime.push(`${m[1]}  (${file})`);
}

// ── The fake host ──────────────────────────────────────────────────────────────────────────────────────────────────────
const mockFiles = walk('ui/dev', ['-mock.js']).concat(existsSync(join(root, 'ui/dev/mock.js')) ? ['ui/dev/mock.js'] : []);
const mock = new Map();
for (const file of [...new Set(mockFiles)]) {
  const text = read(file);
  const at = text.search(/\basync call\s*\(/);
  for (const m of text.slice(at < 0 ? 0 : at).matchAll(/\bcase\s+(['"])([A-Za-z]\w*)\1\s*:/g)) {
    if (!mock.has(m[2])) mock.set(m[2], new Set());
    mock.get(m[2]).add(file);
  }
}
const mockOnly = name => /^debug/.test(name); // for the tests and demos: meant to be only in the fake host

console.log(`Commands: ${host.size} in the C# switch, ${page.size} sent by the page (${pageFiles.length} files), ${mock.size} answered by the fake host (${mockFiles.length} files).`);

section('Sent by the page, unknown to the C# switch (a click that ends in "Unknown command")',
  [...page].filter(([name]) => !host.has(name)).map(([name, files]) => `${name}  (${[...files].join(', ')})`));
section('Answered by the fake host, but sent by nobody and not in the C# switch (the fake invents it)',
  [...mock].filter(([name]) => !page.has(name) && !host.has(name) && !mockOnly(name)).map(([name, files]) => `${name}  (${[...files].join(', ')})`));
section('In the C# switch, sent by no file the report reads (dead, or sent from a place it cannot see)',
  [...host].filter(name => !page.has(name) && !allowed.includes(name)), false, 'information');
section('Sent by the page, not answered by any fake host (the UI tests see the fake host\'s default for it)',
  [...page].filter(([name]) => host.has(name) && !mock.has(name)).map(([name]) => name), false, 'information');
section('Chosen at run time (not checked)', runTime, false, 'information');
section('ScreenLockCommands names that are no command in the C# switch (a typo: the lock screen\'s call would be refused as unknown)',
  allowed.filter(name => !host.has(name)));
console.log(`\nScreenLockCommands: ${allowed.length ? allowed.join(', ') : 'NOT FOUND in AppController.cs'}`);
if (!allowed.length) problems++;

// ── Words ──────────────────────────────────────────────────────────────────────────────────────────────────────────────
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(read('RobloxManager/wwwroot/i18n.js'), sandbox, { filename: 'i18n.js' });
const words = sandbox.window.I18N;
const en = new Set(Object.keys(words.en ?? {}));
const vi = new Set(Object.keys(words.vi ?? {}));
console.log(`\nWords: ${en.size} English keys, ${vi.size} Vietnamese keys.`);
// A key ending in 1 beside its plural key is English's "one" form; Vietnamese has no such form.
const oneForm = k => k.endsWith('1') && en.has(k.slice(0, -1));
section('In English, missing in Vietnamese (the Vietnamese screen shows the key or English)', [...en].filter(k => !vi.has(k) && !oneForm(k)));
section('In Vietnamese, missing in English (the English screen shows the key)', [...vi].filter(k => !en.has(k)));

// t('key') tn('key', n) tIn('en', 'key') T('key') in the page; "key" with a dot. A key built at run time is not seen.
const used = new Map();
const keyPattern = /\b(?:t|tn|T|L)\(\s*(['"`])([a-z][\w]*(?:\.[\w]+)+)\1/g;
const inPattern = /\btIn\(\s*[^,()]+,\s*(['"`])([a-z][\w]*(?:\.[\w]+)+)\1/g;
for (const file of pageFiles) {
  const text = read(file);
  for (const pattern of [keyPattern, inPattern])
    for (const m of text.matchAll(pattern)) {
      if (!used.has(m[2])) used.set(m[2], new Set());
      used.get(m[2]).add(file);
    }
}
const known = key => en.has(key) || en.has(key + '1');
section('Asked for by the page, in neither language',
  [...used].filter(([key]) => !known(key) && !vi.has(key)).map(([key, files]) => `${key}  (${[...files].join(', ')})`));
section('Asked for by the page, missing in Vietnamese only',
  [...used].filter(([key]) => known(key) && !vi.has(key) && !vi.has(key + '1')).map(([key, files]) => `${key}  (${[...files].join(', ')})`));

console.log(`\n${problems === 0 ? 'No drift found.' : `${problems} problem${problems === 1 ? '' : 's'} listed above.`} Report only${strict ? ' (--strict: the exit code is the count)' : ' (exit code 0; --strict makes the count the exit code)'}.`);
process.exit(strict ? Math.min(problems, 99) : 0);
