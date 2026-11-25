import { words, languages } from './monitor-language.js';

const element = (id) => document.getElementById(id);
const frame = element('frame');
const video = element('video');
const message = element('message');
const sample = element('sample');
const pause = element('pause');
const refresh = element('refresh');
const windows = element('windows');
const signal = element('signal');
const sharpButton = element('sharp');
const fullscreenButton = element('fullscreen');
const screen = element('screen');
// "Sharper": the watched window made big on the PC while it is watched. Off until asked for.
let sharp = false;
let sharpNote;
function renderSharp() {
  sharpButton.lastElementChild.textContent = sharpNote ? t('macro') : t('sharp');
  sharpButton.title = t('sharpHelp');
  sharpButton.setAttribute('aria-pressed', String(sharp));
}
// The language picked on this page; else the app's (it comes with the list of windows); else the browser's.
const known = (code) => (code && words[code] ? code : undefined);
const fromBrowser = () => {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const code = known(String(tag).toLowerCase().split('-')[0]);
    if (code) return code;
  }
  return 'en';
};
let picked;
try {
  picked = known(localStorage.getItem('monitor-language'));
} catch {}
let language = picked ?? fromBrowser();
const t = (key, value) => {
  const text = words[language][key] ?? words.en[key];
  return typeof text === 'function' ? text(value) : text;
};
const fragment = location.hash.slice(1);
if (/^[0-9a-f]{48}$/.test(fragment)) {
  sessionStorage.setItem('monitor-session', fragment);
  history.replaceState(null, '', location.pathname);
}
const key = sessionStorage.getItem('monitor-session');
const headers = { Authorization: `Bearer ${key ?? ''}` };

let source;
let running = false;
let controller;
let timer;
let objectUrl;
let videoView = false;
let livePlayer;
let liveClock;
let opening = false;
let data;
let sessionText = location.protocol === 'https:' ? 'remote' : 'lan';
let detail = ['loadingWindows'];
let messageKey = 'choose';
let statusKey = 'waiting';
let statusLive = false;
// Pictures of the windows for their tiles: taken one window at a time, a few seconds apart, while the page is shown.
const thumbs = new Map();
let thumbTimer;
let thumbTurn = 0;
let listing = false;

function setDetail(key, value) {
  detail = [key, value];
  element('window-status').textContent = t(...detail);
}

function sourceLabel(item) {
  if (!item) return t('noSelection');
  if (item.id === 'demo') return t('demo');
  return item.label || t('unknownWindow', item.ordinal);
}

const pillWords = { live: 'tileLive', seen: 'tileSeen', none: 'tileNone', waiting: 'tileWaiting' };

function tileState(item) {
  if (source?.id === item.id && (liveClock || running)) return 'live';
  const thumb = thumbs.get(item.id);
  if (!thumb) return 'waiting';
  return thumb.url ? 'seen' : 'none';
}

const corners = () => ['tl', 'tr', 'bl', 'br'].map((at) => {
  const mark = document.createElement('span');
  mark.className = `corner ${at}`;
  mark.setAttribute('aria-hidden', 'true');
  return mark;
});

// The window's last picture, or its name's first letter, large and quiet, while there is none.
function tilePicture(item, name) {
  const url = thumbs.get(item.id)?.url;
  if (url) {
    const picture = document.createElement('img');
    picture.alt = '';
    picture.src = url;
    return picture;
  }
  const initial = document.createElement('span');
  initial.className = 'tile-initial';
  initial.setAttribute('aria-hidden', 'true');
  initial.textContent = [...name.trim()][0]?.toUpperCase() ?? '';
  return initial;
}

function renderWindows() {
  const note = element('window-note');
  const items = data?.windows ?? [];
  const noteKey = data?.accountState === 'locked' ? 'lockedNames' :
    items.some((item) => !item.label) ? 'noNames' : undefined;
  note.hidden = !noteKey;
  note.textContent = noteKey ? t(noteKey) : '';
  windows.replaceChildren(...items.map((item) => {
    const button = document.createElement('button');
    const text = document.createElement('span');
    const title = document.createElement('span');
    const pill = document.createElement('span');
    const dot = document.createElement('i');
    const words = document.createElement('span');
    const state = tileState(item);
    const name = sourceLabel(item);
    button.type = 'button';
    button.className = 'tile';
    button.dataset.source = item.id;
    button.setAttribute('aria-label', `${name} · ${t(videoView ? 'watch' : 'preview')}`);
    button.append(tilePicture(item, name));
    text.className = 'tile-text';
    title.className = 'tile-name';
    title.textContent = name;
    pill.className = 'pill';
    pill.dataset.state = state;
    words.textContent = t(pillWords[state]);
    pill.append(dot, words);
    text.append(title, pill);
    button.append(...corners(), text);
    button.addEventListener('click', () => setSource(item));
    return button;
  }));
  renderPresets();
  syncSelection();
}

function stopThumbs() {
  clearTimeout(thumbTimer);
  thumbTimer = undefined;
}

function nextThumb(delay) {
  stopThumbs();
  thumbTimer = setTimeout(takeThumb, delay);
}

// One window's picture at a time, round the windows (the one watched live left out): every window about every 6 s.
async function takeThumb() {
  thumbTimer = undefined;
  const items = (data?.windows ?? []).filter((item) => !(liveClock && item.id === source?.id));
  if (!key || document.hidden || !items.length) return;
  if (listing) {
    nextThumb(1000);
    return;
  }
  const item = items[thumbTurn++ % items.length];
  let wait = Math.max(1200, 6000 / items.length);
  try {
    const response = await fetch(`/api/thumb/${item.id}`, { headers });
    if (response.status === 401) {
      disableSession();
      return;
    }
    const previous = thumbs.get(item.id)?.url;
    if (response.status === 200) {
      const blob = await response.blob();
      thumbs.set(item.id, { url: URL.createObjectURL(blob) });
      if (previous) URL.revokeObjectURL(previous);
    } else if (response.status === 204) {
      thumbs.set(item.id, {});
      if (previous) URL.revokeObjectURL(previous);
    } else {
      wait = 2500;
    }
    // (Only that tile's picture changes, so a tile in focus or under the finger stays as it is.)
    const tile = [...windows.querySelectorAll('.tile')].find((node) => node.dataset.source === item.id);
    if (tile) {
      tile.firstElementChild.replaceWith(tilePicture(item, sourceLabel(item)));
      syncSelection();
    }
  } catch {
    wait = 4000;
  }
  if (!document.hidden && key && data && !thumbTimer) nextThumb(wait);
}

function renderLanguage() {
  document.documentElement.lang = language;
  document.title = t('title') + ' · Bloxkeep';
  element('language-name').textContent = languages.find(([code]) => code === language)?.[1] ?? language;
  element('language').setAttribute('aria-label', t('language'));
  for (const button of element('language-list').children) {
    button.setAttribute('aria-current', String(button.dataset.lang === language));
  }
  fullscreenButton.title = t('fullscreen');
  sample.title = t('demoHelp');
  frame.alt = t('stillAlt');
  video.setAttribute('aria-label', t('videoLabel'));
  for (const node of document.querySelectorAll('[data-i18n]')) {
    node.textContent = t(node.dataset.i18n);
  }
  element('session-state').textContent = t(sessionText);
  element('window-status').textContent = t(...detail);
  message.textContent = t(messageKey);
  status(statusKey, statusLive);
  renderWindows();
  renderSharp();
  renderCommandNote();
}

const languageButton = element('language');
const languageList = element('language-list');
function showLanguages(open) {
  languageList.hidden = !open;
  languageButton.setAttribute('aria-expanded', String(open));
}
languageList.replaceChildren(...languages.map(([code, name]) => {
  const button = document.createElement('button');
  button.type = 'button';
  button.lang = code;
  button.dataset.lang = code;
  button.textContent = name;
  button.addEventListener('click', () => {
    language = picked = code;
    try {
      localStorage.setItem('monitor-language', code);
    } catch {}
    showLanguages(false);
    renderLanguage();
  });
  return button;
}));
languageButton.addEventListener('click', () => showLanguages(languageList.hidden));
document.addEventListener('click', (event) => {
  if (!languageList.hidden && !event.target.closest('.lang')) showLanguages(false);
});
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && !languageList.hidden) {
    showLanguages(false);
    languageButton.focus();
  }
});

const livePost = (path) => fetch(`/api/live/${path}`, { method: 'POST', headers, keepalive: true });

function stopLive() {
  if (!liveClock) return Promise.resolve();
  clearInterval(liveClock);
  liveClock = undefined;
  livePlayer?.destroy();
  livePlayer = undefined;
  sharpButton.hidden = true;
  video.hidden = true;
  video.removeAttribute('src');
  video.load();
  return livePost('stop').catch(() => {});
}

function lostVideo() {
  if (!liveClock) return;
  void stopLive();
  syncSelection();
  empty('liveEnded');
  status('stopped');
}

async function startLive(item) {
  if (opening) return;
  opening = true;
  try {
    await stopLive();
    stopPoll();
    running = false;
    source = item;
    syncSelection();
    empty('opening');
    status('connecting');
    // (The view before this one may still be ending on the PC: asked again a few times before saying it is in use.)
    const path = () => `start/${item.id}${sharp ? '?sharp=1' : ''}`;
    let reply = await livePost(path());
    let body = await reply.json();
    for (let i = 0; i < 6 && reply.status === 409 && body.code === 'inUse'; i++) {
      await new Promise((resolve) => setTimeout(resolve, 700));
      reply = await livePost(path());
      body = await reply.json();
    }
    // A macro plays on the PC: no window is resized under it. Watched as it is, and the button says why for a moment.
    if (reply.status === 409 && body.code === 'macro' && sharp) {
      sharp = false;
      sharpNote = setTimeout(() => { sharpNote = undefined; renderSharp(); }, 5000);
      renderSharp();
      reply = await livePost(path());
      body = await reply.json();
    }
    if (reply.status === 401) {
      disableSession();
      return;
    }
    if (!reply.ok) throw new Error(body.code ?? 'openFailed');
    if (document.hidden) {
      await livePost('stop').catch(() => {});
      throw new Error('foregroundTab');
    }
    liveClock = setInterval(async () => {
      try {
        const keep = await livePost('keep');
        if (!keep.ok) lostVideo();
      } catch {
        lostVideo();
      }
    }, 4000);
    syncSelection();
    // The encoder needs a first media segment before Safari can start a native playlist.
    let ready = false;
    for (let i = 0; i < 30 && liveClock; i++) {
      const manifest = await fetch(body.playlist);
      if (manifest.ok) {
        ready = true;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 180));
    }
    if (!liveClock) return;
    if (!ready) throw new Error('noVideoFrame');
    const { default: Hls } = await import('/hls.js');
    if (!liveClock) return;
    if (Hls.isSupported()) {
      livePlayer = new Hls({ liveSyncDuration: 2.5, liveMaxLatencyDuration: 5, maxLiveSyncPlaybackRate: 1.1 });
      livePlayer.attachMedia(video);
      livePlayer.loadSource(body.playlist);
      livePlayer.on(Hls.Events.ERROR, (_, error) => { if (error.fatal) lostVideo(); });
    } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
      video.src = body.playlist;
    } else {
      throw new Error('hlsUnsupported');
    }
    video.hidden = false;
    sharpButton.hidden = false;
    renderSharp();
    element('placeholder').hidden = true;
    await video.play().catch(() => {});
  } catch (error) {
    await stopLive();
    empty(Object.hasOwn(words[language], error.message) ? error.message : 'openFailed');
    status('playbackFailed');
    syncSelection();
  } finally {
    opening = false;
  }
}
sharpButton.addEventListener('click', () => {
  if (opening || !source || !liveClock) return;
  sharp = !sharp;
  renderSharp();
  // (The same window again, with or without: the app makes it big, or puts it back, as the view starts.)
  void startLive(source);
});
video.addEventListener('playing', () => { if (liveClock) status('playing', true); });
video.addEventListener('error', lostVideo);

function status(key, live = false) {
  statusKey = key;
  statusLive = live;
  signal.lastElementChild.textContent = t(key);
  signal.dataset.live = String(live);
}

function empty(key) {
  messageKey = key;
  if (objectUrl) {
    URL.revokeObjectURL(objectUrl);
    objectUrl = undefined;
  }
  frame.removeAttribute('src');
  frame.hidden = true;
  message.textContent = t(key);
  element('placeholder').hidden = false;
}

function stopPoll() {
  clearTimeout(timer);
  controller?.abort();
  controller = undefined;
}

function syncSelection() {
  const controls = [sample, ...windows.querySelectorAll('button[data-source]')];
  for (const control of controls) {
    control.setAttribute('aria-pressed', String((control.dataset.source ?? 'demo') === source?.id));
  }
  element('active-source').textContent = sourceLabel(source);
  pause.disabled = !source || (videoView && source.id !== 'demo' && !liveClock);
  const does = liveClock ? 'stop' : running || !source ? 'pause' : 'resume';
  screen.dataset.empty = String(!source);
  pause.dataset.does = does;
  element('pause-label').textContent = t(does);
  fullscreenButton.disabled = !source;
  syncCommands();
  for (const tile of windows.querySelectorAll('.tile')) {
    const item = data?.windows?.find((w) => w.id === tile.dataset.source);
    const pill = tile.querySelector('.pill');
    if (!item || !pill) continue;
    const state = tileState(item);
    if (pill.dataset.state !== state) {
      pill.dataset.state = state;
      pill.lastElementChild.textContent = t(pillWords[state]);
    }
  }
}

fullscreenButton.addEventListener('click', () => {
  if (document.fullscreenElement) {
    void document.exitFullscreen();
    return;
  }
  // (iPhone Safari has no full screen for a page, only its video player's own.)
  if (screen.requestFullscreen) void screen.requestFullscreen().catch(() => {});
  else if (!video.hidden && video.webkitEnterFullscreen) video.webkitEnterFullscreen();
});

function setSource(next) {
  if (opening) return;
  if (videoView && next.id !== 'demo') {
    if (liveClock && source?.id === next.id) return;
    void startLive(next);
    return;
  }
  void stopLive();
  stopPoll();
  source = next;
  running = true;
  syncSelection();
  empty('receivingSoon');
  status('connecting');
  updateFrame();
}

function disableSession() {
  void stopLive();
  stopPoll();
  stopThumbs();
  running = false;
  source = undefined;
  syncSelection();
  sample.disabled = refresh.disabled = true;
  data = undefined;
  renderWindows();
  setDetail('reopen');
  sessionText = 'unauthorized';
  element('session-state').textContent = t(sessionText);
  empty('expired');
  status('disconnected');
}

async function updateFrame() {
  if (!running || document.hidden || !source || controller) return;
  const request = new AbortController();
  controller = request;
  const until = performance.now() + (location.protocol === 'https:' ? 500 : 300);
  let delay;
  try {
    const response = await fetch(`/api/frame/${source.id}`, { headers, signal: request.signal });
    if (request.signal.aborted) return;
    if (response.status === 401) {
      disableSession();
      return;
    }
    const minimized = response.headers.get('X-Bloxkeep-Source') === 'minimized';
    if (response.status === 200) {
      const blob = await response.blob();
      if (request.signal.aborted) return;
      const previous = objectUrl;
      objectUrl = URL.createObjectURL(blob);
      frame.src = objectUrl;
      frame.hidden = false;
      element('placeholder').hidden = true;
      if (previous) URL.revokeObjectURL(previous);
      const label = source.id === 'demo' ? 'demoPlaying' :
        minimized ? 'interval' : 'receiving';
      status(label, true);
      if (minimized) delay = 7500;
    } else if (response.status === 204) {
      empty(minimized ? 'waitingMinimized' :
        'missingFrame');
      status(minimized ? 'waitInterval' : 'waitWindow');
      delay = minimized ? 8000 : 850;
    } else if (response.status === 429) {
      delay = 325;
    } else {
      status('reconnecting');
      delay = 1200;
    }
  } catch {
    if (request.signal.aborted) return;
    empty('networkLost');
    status('reconnecting');
    delay = 1500;
  } finally {
    if (controller === request) {
      controller = undefined;
      timer = setTimeout(updateFrame, delay ?? Math.max(10, until - performance.now()));
    }
  }
}

async function loadWindows() {
  refresh.disabled = true;
  listing = true;
  setDetail('loadingWindows');
  try {
    const response = await fetch('/api/windows', { headers });
    if (response.status === 401) {
      disableSession();
      return;
    }
    if (!response.ok) throw new Error();
    data = await response.json();
    videoView = Boolean(data.video);
    data.windows = data.windows.map((item, index) => ({ ...item, ordinal: index + 1 }));
    takeDone(data.done);
    if (!picked && known(data.language) && data.language !== language) {
      language = data.language;
      renderLanguage();
    }
    for (const [id, thumb] of thumbs) {
      if (data.windows.some((item) => item.id === id)) continue;
      if (thumb.url) URL.revokeObjectURL(thumb.url);
      thumbs.delete(id);
    }
    if (source && source.id !== 'demo') {
      const current = data.windows.find((item) => item.id === source.id);
      source = current ?? { id: source.id, ordinal: source.ordinal };
    }
    renderWindows();
    if (!data.captureReady) setDetail('captureStarting');
    else if (data.windows.length) setDetail('windowsCount', data.windows.length);
    else setDetail('missingWindows');
    syncSelection();
  } catch {
    setDetail('windowsFailed');
  } finally {
    listing = false;
    if (!sample.disabled) refresh.disabled = false;
    if (data && !thumbTimer) nextThumb(300);
  }
}

// ── Commands for the PC: Rejoin, Switch server and Close for the chosen window, Clean RAM, and a preset to open ──
const commandNote = element('command-note');
const commandButtons = {
  rejoin: element('cmd-rejoin'), switch: element('cmd-switch'), close: element('cmd-close'), ram: element('cmd-ram'),
};
const outcomeWords = {
  rejoining: 'rejoining', cantRejoin: 'cantRejoin', switching: 'switching', closed: 'closed', noAccount: 'noAccount',
  noPreset: 'noPreset', presetAsks: 'presetAsks', presetDone: 'presetDone', presetStopped: 'presetStopped',
  ramCleaned: 'ramCleaned', ramNeedsAuto: 'ramNeedsAuto', ramFailed: 'ramFailed', locked: 'cmdLocked',
};
// Commands the app is still doing (it answered "working"): their outcome comes with a later list of windows.
const pending = new Set();
const pendingWaits = [2000, 4000, 8000, 15000, 30000, 60000];
let pendingTimer;
let pendingStep = 0;
let sending = false;
let closeSure;
let closeSureTimer;
let noteWords;

function renderCommandNote() {
  commandNote.hidden = !noteWords;
  commandNote.textContent = noteWords ? t(...noteWords) : '';
}

function showOutcome(answer) {
  const word = outcomeWords[answer?.code];
  if (word) noteWords = [word, answer];
  else if (answer?.code === 'failed' && typeof answer.text === 'string' && answer.text) noteWords = ['plain', answer.text];
  else noteWords = ['cmdFailed'];
  // A preset that opened only some: the first reason the app gave, after the count.
  if (answer?.code === 'presetDone' && answer.opened < answer.total && typeof answer.errors?.[0] === 'string') {
    noteWords = ['plain', `${t('presetDone', answer)} ${answer.errors[0]}`];
  }
  renderCommandNote();
}

function takeDone(done) {
  for (const outcome of Array.isArray(done) ? done : []) {
    if (pending.delete(outcome?.id)) showOutcome(outcome);
  }
}

function watchPending() {
  clearTimeout(pendingTimer);
  if (!pending.size || pendingStep >= pendingWaits.length) {
    pending.clear();
    return;
  }
  pendingTimer = setTimeout(async () => {
    pendingStep++;
    if (!listing) await loadWindows();
    watchPending();
  }, pendingWaits[pendingStep]);
}

function forAccount() {
  return Boolean(source && source.id !== 'demo' && data?.commands?.includes(source.id));
}

function syncCommands() {
  const can = forAccount() && !sending;
  commandButtons.rejoin.disabled = commandButtons.switch.disabled = commandButtons.close.disabled = !can;
  commandButtons.ram.disabled = !data || sending;
  const sure = can && closeSure === source?.id;
  commandButtons.close.dataset.sure = String(sure);
  element('close-label').textContent = t(sure ? 'closeSure' : 'close');
  for (const button of element('presets').querySelectorAll('button')) button.disabled = sending;
}

function renderPresets() {
  const presets = Array.isArray(data?.presets) ? data.presets : [];
  element('presets-block').hidden = !presets.length;
  element('presets').replaceChildren(...presets.map((preset) => {
    const button = document.createElement('button');
    const icon = document.createElement('span');
    const name = document.createElement('span');
    button.type = 'button';
    button.className = 'preset';
    button.disabled = sending;
    icon.className = 'action-icon';
    icon.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l10.2-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z"/></svg>';
    name.className = 'preset-name';
    name.textContent = preset.name;
    button.append(icon, name);
    button.addEventListener('click', () => send({ do: 'preset', preset: preset.id }));
    return button;
  }));
}

async function send(order) {
  if (sending) return;
  sending = true;
  closeSure = undefined;
  syncCommands();
  noteWords = ['working'];
  renderCommandNote();
  try {
    const response = await fetch('/api/command', {
      method: 'POST', headers: { ...headers, 'Content-Type': 'application/json' }, body: JSON.stringify(order),
    });
    if (response.status === 401) {
      disableSession();
      return;
    }
    if (!response.ok) throw new Error();
    const answer = await response.json();
    if (answer?.code === 'working' && answer.id !== undefined) {
      pending.add(answer.id);
      pendingStep = 0;
      watchPending();
    } else {
      showOutcome(answer);
    }
  } catch {
    noteWords = ['cmdFailed'];
    renderCommandNote();
  } finally {
    sending = false;
    syncCommands();
  }
  // The list of windows follows what the command did (a window closed, more opened).
  if (!pending.size) setTimeout(() => { if (!listing && key) loadWindows(); }, 2500);
}

for (const what of ['rejoin', 'switch']) {
  commandButtons[what].addEventListener('click', () => {
    if (forAccount()) send({ do: what, window: source.id });
  });
}
commandButtons.close.addEventListener('click', () => {
  if (!forAccount()) return;
  clearTimeout(closeSureTimer);
  if (closeSure === source.id) {
    send({ do: 'close', window: source.id });
    return;
  }
  closeSure = source.id;
  syncCommands();
  closeSureTimer = setTimeout(() => {
    closeSure = undefined;
    syncCommands();
  }, 4000);
});
commandButtons.ram.addEventListener('click', () => send({ do: 'ram' }));

sample.addEventListener('click', () => setSource({ id: 'demo' }));
refresh.addEventListener('click', loadWindows);
pause.addEventListener('click', () => {
  if (liveClock) {
    void stopLive();
    status('stopped');
    empty('watchAgain');
    syncSelection();
    return;
  }
  running = !running;
  syncSelection();
  if (running) updateFrame();
  else {
    stopPoll();
    status('paused');
  }
});
document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    if (liveClock) lostVideo();
    stopPoll();
    stopThumbs();
  } else {
    if (running) updateFrame();
    if (data && key) nextThumb(300);
  }
});
window.addEventListener('pagehide', stopLive);

renderLanguage();
if (key) loadWindows();
else disableSession();
