'use strict';
// Fake app for the overlay's page (overlay.html) in a normal browser, over a stand-in game picture (the page itself is
// see-through over the game). ?mode=edit (default) a macro's dots and areas, areas the areas only, pick the frozen
// picture to click on, screen the colour picker; ?new=1 a macro with no name yet, ?long=1 long names, ?theme=light a
// theme, ?lang=vi Vietnamese. What the page sends shows in window.__acts (and the page's calls).
(() => {
  const params = new URLSearchParams(location.search);
  const vi = params.get('lang') === 'vi';
  const mode = params.get('mode') || 'edit';
  const long = params.has('long');
  const GAME = 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">
    <rect width="1280" height="440" fill="#7ec8f0"/><rect y="440" width="1280" height="280" fill="#5aa04a"/>
    <rect x="140" y="300" width="240" height="140" fill="#b97a4a"/><rect x="170" y="240" width="180" height="70" fill="#8c2f2f"/>
    <rect x="820" y="250" width="300" height="190" fill="#9aa3ad"/><rect x="900" y="330" width="60" height="110" fill="#2b2b2b"/>
    <rect x="600" y="310" width="64" height="64" fill="#f2c94c"/><rect x="590" y="374" width="84" height="96" fill="#3b82f6"/>
    <rect x="598" y="470" width="30" height="78" fill="#1f2937"/><rect x="636" y="470" width="30" height="78" fill="#1f2937"/>
    <rect x="1000" y="560" width="200" height="70" rx="10" fill="#15803d"/><rect x="1006" y="566" width="188" height="58" rx="8" fill="#22c55e"/>
    <rect x="24" y="20" width="44" height="44" rx="8" fill="#000" fill-opacity=".45"/><rect x="1104" y="20" width="152" height="40" rx="8" fill="#000" fill-opacity=".4"/></svg>`);
  // The stand-in game under the page (the app's window lies over the real one).
  addEventListener('DOMContentLoaded', () => { const img = document.querySelector('.ov-standin'); if (img) img.src = GAME; });
  const name = params.has('new') ? '' : long ? 'Claim the daily chest and the weekly quest rewards on every alt' : vi ? 'Nhận rương hằng ngày' : 'Claim daily chest';
  const data = {
    mode, name,
    dots: mode === 'edit' ? [{ step: 0, x: 0.3, y: 0.62 }, { step: 2, x: 0.86, y: 0.82 }, { step: 4, x: 0.5, y: 0.45 }] : [],
    areas: mode === 'edit' || mode === 'areas' ? [
      { name: 'a1', label: long ? 'The claim button at the bottom right' : vi ? 'Nút nhận' : 'Claim button', colour: '#22c55e', rect: { x: 0.74, y: 0.74, w: 0.22, h: 0.16 } },
      { name: 'a2', label: vi ? 'Thanh máu' : 'Health bar', colour: '#f59e0b', rect: { x: 0.04, y: 0.2, w: 0.3, h: 0.1 } },
    ] : [],
    picture: mode === 'pick' || mode === 'screen' ? GAME : null,
    theme: params.get('theme') || 'dark', customTheme: null, language: vi ? 'vi' : 'en',
  };
  window.__acts = [];
  let emit = () => {};
  window.__mockBridge = {
    attach(fn) { emit = fn; },
    async call(cmd, a) {
      if (cmd === 'ready') setTimeout(() => emit('overlay', data), 0);
      if (cmd === 'act') window.__acts.push(a);
      return null;
    },
  };
})();
