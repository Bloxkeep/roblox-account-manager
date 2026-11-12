// The interface's words for the GPUI window: wwwroot/i18n.js (the same file the web page reads) as assets/i18n.json.
// Run by build.rs; the JSON is generated, not kept in git.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const src = path.resolve(here, '../../RobloxManager/wwwroot/i18n.js');
const out = path.resolve(here, '../assets/i18n.json');
const box = { window: {} };
vm.runInNewContext(fs.readFileSync(src, 'utf8'), box, { filename: src });
const { en, vi } = box.window.I18N;
fs.writeFileSync(out, JSON.stringify({ en, vi }));
console.log(`i18n.json: ${Object.keys(en).length} en, ${Object.keys(vi).length} vi`);
