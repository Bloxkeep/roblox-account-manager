// Settings search finds a row by its English words too in another language (someone following an English guide, or
// used to the English names): each language's strings are indexed by key once, and a row's text in the app's language
// gives back its English text. Only whole strings are found (a sentence with a value filled in has no English twin).
type Strings = Record<string, unknown>;
declare global { interface Window { I18N?: Record<string, Strings> } }

const indexes = new Map<string, Map<string, string>>();

function index(lang: string): Map<string, string> {
  let map = indexes.get(lang);
  if (map) return map;
  map = new Map();
  const own = window.I18N?.[lang] || {}, en = window.I18N?.en || {};
  for (const [key, value] of Object.entries(own)) {
    const english = en[key];
    if (typeof value === 'string' && typeof english === 'string' && value !== english) map.set(value.trim(), english);
  }
  indexes.set(lang, map);
  return map;
}

/** The English for a settings text shown in lang, or '' when it is English already or has no English twin. */
export function englishOf(text: unknown, lang: string): string {
  if (lang === 'en' || typeof text !== 'string' || !text) return '';
  return index(lang).get(text.trim()) || '';
}
