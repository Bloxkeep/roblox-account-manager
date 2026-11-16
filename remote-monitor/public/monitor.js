import { words } from './monitor-language.js';

const element = (id) => document.getElementById(id);
const frame = element('frame');
const video = element('video');
const message = element('message');
const sample = element('sample');
const pause = element('pause');
const refresh = element('refresh');
const windows = element('windows');
const signal = element('signal');
const languageChoice = element('language');
let language = localStorage.getItem('monitor-language') === 'vi' ? 'vi' : 'en';
const t = (key, value) => {
  const text = words[language][key];
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

function setDetail(key, value) {
  detail = [key, value];
  element('window-status').textContent = t(...detail);
}

function sourceLabel(item) {
  if (!item) return t('noSelection');
  if (item.id === 'demo') return t('demo');
  return item.label || t('unknownWindow', item.ordinal);
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
    const label = document.createElement('span');
    const title = document.createElement('strong');
    const action = document.createElement('small');
    button.type = 'button';
    button.className = 'source-button';
    button.dataset.source = item.id;
    label.className = 'window-label';
    title.textContent = sourceLabel(item);
    action.textContent = t(videoView ? 'watch' : 'preview');
    label.append(title, action);
    button.append(label);
    button.addEventListener('click', () => setSource(item));
    return button;
  }));
}

function renderLanguage() {
  document.documentElement.lang = language;
  document.title = t('title') + ' · Bloxkeep';
  languageChoice.value = language;
  languageChoice.setAttribute('aria-label', t('language'));
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
  syncSelection();
}

languageChoice.addEventListener('change', () => {
  language = languageChoice.value === 'en' ? 'en' : 'vi';
  localStorage.setItem('monitor-language', language);
  renderLanguage();
});

const livePost = (path) => fetch(`/api/live/${path}`, { method: 'POST', headers, keepalive: true });

function stopLive() {
  if (!liveClock) return Promise.resolve();
  clearInterval(liveClock);
  liveClock = undefined;
  livePlayer?.destroy();
  livePlayer = undefined;
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
    const reply = await livePost(`start/${item.id}`);
    const body = await reply.json();
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
  pause.textContent = t(liveClock ? 'stop' : running ? 'pause' : 'resume');
}

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
    if (!sample.disabled) refresh.disabled = false;
  }
}

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
  }
  else if (running) updateFrame();
});
window.addEventListener('pagehide', stopLive);

renderLanguage();
if (key) loadWindows();
else disableSession();
