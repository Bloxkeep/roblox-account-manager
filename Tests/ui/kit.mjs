// Do the screens use Bloxkeep's controls (ui/src/kit.tsx) rather than writing them by hand? A <button className="btn …">,
// <input className="field">, <div className="seg">, <span className="badge">, <label className="switch"> written anywhere
// but kit.tsx is a control that will drift from the others. Reads the sources with TypeScript's parser (ui/node_modules,
// so after `npm ci` in ui/); no browser, no build.
//   node Tests/ui/kit.mjs        lists each hand-written control; exit code 1 when there is one
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const src = path.join(root, 'ui', 'src');
const ts = createRequire(path.join(root, 'ui', 'package.json'))('typescript');

// The class that makes an element one of the kit's controls, and the control to use instead.
const KIT = { btn: 'Button', 'icon-btn': 'IconButton', field: 'Field or TextArea', seg: 'Segmented', switch: 'Switch', badge: 'Badge' };

const files = [];
(function walk(d) {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p);
    else if (/\.tsx$/.test(e.name) && e.name !== 'kit.tsx') files.push(p);
  }
})(src);

const found = [];
for (const file of files) {
  const sf = ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const visit = node => {
    const open = ts.isJsxElement(node) ? node.openingElement : ts.isJsxSelfClosingElement(node) ? node : null;
    // Only plain elements (lower case): a component given className="field" (TextBox, SuggestInput…) draws its own box.
    if (open && /^[a-z]/.test(open.tagName.getText(sf))) {
      const cls = open.attributes.properties.find(a => ts.isJsxAttribute(a) && a.name.getText(sf) === 'className');
      if (cls?.initializer) {
        // The class names it can have: every word in its string or template literal parts.
        const words = (cls.initializer.getText(sf).match(/[\w-]+/g) || []);
        for (const w of words) if (KIT[w]) {
          const { line } = sf.getLineAndCharacterOfPosition(open.getStart(sf));
          found.push(`${path.relative(root, file).replace(/\\/g, '/')}:${line + 1}  <${open.tagName.getText(sf)} class "${w}">: use ${KIT[w]} from ui/src/kit.tsx`);
          break;
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
}

if (found.length) {
  console.log(`Controls written by hand instead of with ui/src/kit.tsx (${found.length}):`);
  for (const f of found) console.log('  ' + f);
  process.exit(1);
}
console.log(`Every button, text box, switch, choice row and badge in ui/src comes from kit.tsx (${files.length} files).`);
