// P2 (building macros) on top of mock.js, as the app now does it: where each ended run was (lastStep, the step a failed
// run stopped at), which macros on the selected accounts Run can still go to with none selected (macroCanRun: those
// with a game set, while an account plays), and the five built-in templates with their notes and to-do fields.
(() => {
  const bridge = window.__mockBridge;
  if (!bridge) return;
  const params = new URLSearchParams(location.search);
  const tx = (en, vi) => params.get('lang') === 'vi' ? vi : en;

  // The step each run was last seen on: a failed run keeps it as lastStep (the app's r.Pc once it ended).
  const seen = new Map();
  const runKey = r => `${r.macroId}|${r.accountId}|${r.trial ? 1 : 0}`;
  const withP2 = s => {
    if (!s || typeof s !== 'object') return s;
    const runs = (s.macroRuns || []).map(r => {
      if (r.step != null) seen.set(runKey(r), r.step);
      const ended = !['waiting', 'running', 'paused'].includes(r.status);
      const last = ended ? (r.lastStep ?? seen.get(runKey(r))) : undefined;
      return last != null && ended ? { ...r, lastStep: last } : r;
    });
    // ?nowindows=1: no Roblox window open (every account idle), for the editor's empty picture.
    const accounts = params.has('nowindows') ? (s.accounts || []).map(a => a.state === 'running' ? { ...a, state: 'idle', session: null } : a) : s.accounts;
    // ?p2broken=1: one more macro, with what stops a save on its rows (an If never ended, a click with no point).
    const settings = params.has('p2broken') && s.settings && !(s.settings.macros || []).some(m => m.id === 'm5')
      ? { ...s.settings, macros: [...(s.settings.macros || []), BROKEN()] } : s.settings;
    const playing = (accounts || []).some(a => a.state === 'running');
    const canRun = playing ? (settings?.macros || []).filter(m => m.targets === 'selected' && m.panel?.placeId).map(m => m.id) : [];
    return { ...s, accounts, settings, macroRuns: runs, macroCanRun: canRun };
  };
  const BROKEN = () => ({ id: 'm5', name: tx('Daily reward', 'Quà hằng ngày'), targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: null,
    jitterMs: 40, jitterPx: 3, maxMinutes: 10, onFail: 'stop', options: [], areas: [], panel: { autoShow: false, placeId: null },
    steps: [{ kind: 'key', key: 'E', ms: 60, note: tx('Opens the rewards window', 'Mở cửa sổ phần thưởng') },
      { kind: 'if', test: { kind: 'text', text: 'Claim' } },
      { kind: 'click', button: 'left', double: false },
      { kind: 'addVar', name: 'claims', value: 1 }] });

  const attach = bridge.attach.bind(bridge);
  bridge.attach = fn => attach((event, data) => fn(event, event === 'state' ? withP2(data) : data));
  const call = bridge.call.bind(bridge);
  bridge.call = async (cmd, a) => {
    if (cmd === 'macroTemplates') return TEMPLATES();
    const r = await call(cmd, a);
    return cmd === 'getState' && r?.state ? { ...r, state: withP2(r.state) } : r;
  };

  const base = { targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: null, jitterMs: 40, jitterPx: 3, onFail: 'stop',
    options: [], areas: [], panel: { autoShow: false, placeId: null } };
  // As MacroTemplates.cs builds them (keys, click, cast, rejoin, blank).
  const TEMPLATES = () => ({ macros: [
    { ...base, id: '', name: tx('Press a key every few minutes', 'Bấm phím định kỳ'), icon: 'keyboard', maxMinutes: 240,
      about: tx("Presses a key now and then, so the game doesn't take you for away.", 'Thỉnh thoảng bấm một phím để game không coi bạn là vắng mặt.'),
      options: [{ name: 'o1', label: tx('Every', 'Mỗi'), kind: 'number', value: 300000, default: 300000, min: 1000, max: 1080000, unit: 's' },
        { name: 'o2', label: tx('Key', 'Phím'), kind: 'key', value: 'Space', default: 'Space' }],
      steps: [{ kind: 'loop', times: 0 },
        { kind: 'key', key: '$o2', ms: 60, note: tx('Roblox kicks idle players after 20 minutes; this presses a key before that.', 'Roblox đá người chơi đứng yên sau 20 phút; bước này bấm phím trước lúc đó.') },
        { kind: 'wait', ms: '$o1', note: tx('The player can change how long on the panel', 'Người chơi đổi thời gian này trên bảng') }, { kind: 'endLoop' }] },
    { ...base, id: '', name: tx('Click a button when it shows', 'Bấm nút khi nó hiện'), icon: 'pointer', maxMinutes: 120,
      about: tx('Waits for a button of a colour to show up in an area, clicks it, and waits for the next one.', 'Chờ nút có màu đã chọn hiện trong vùng, bấm vào rồi chờ nút tiếp theo.'),
      options: [{ name: 'o1', label: tx('Button colour', 'Màu nút'), kind: 'colour', value: '#22c55e', default: '#22c55e' },
        { name: 'o2', label: tx('Then wait', 'Rồi chờ'), kind: 'number', value: 500, default: 500, min: 0, max: 60000, unit: 's' }],
      areas: [{ name: 'a1', label: tx('Button area', 'Vùng nút'), colour: '#22c55e', rect: { x: 0.25, y: 0.25, w: 0.5, h: 0.5 } }],
      steps: [{ kind: 'loop', times: 0 },
        { kind: 'waitColour', colour: '$o1', tolerance: 24, region: '@a1', timeoutMs: 600000, todo: ['region', 'colour'], note: tx('Move the box to where the button appears', 'Kéo khung tới chỗ nút hiện ra') },
        { kind: 'clickColour', colour: '$o1', tolerance: 24, region: '@a1', minSize: 6, button: 'left', timeoutMs: 2000, note: tx("Pick the button's color on the game", 'Chọn màu nút trên game') },
        { kind: 'addVar', name: 'clicks', value: 1, note: tx('Counts the clicks, shown on the panel', 'Đếm số lần bấm, hiện trên bảng') },
        { kind: 'wait', ms: '$o2' }, { kind: 'endLoop' }] },
    { ...base, id: '', name: tx('Cast and reel', 'Thả câu và kéo'), icon: 'bar', maxMinutes: 240,
      about: tx('Holds the mouse to cast, waits for the bar, then keeps the marker on the target until the bar goes.', 'Giữ chuột để quăng, chờ thanh hiện ra, rồi giữ vạch trên mục tiêu đến khi thanh biến mất.'),
      options: [{ name: 'o1', label: tx('Hold to cast', 'Giữ để quăng'), kind: 'number', value: 800, default: 800, min: 0, max: 10000, unit: 's' },
        { name: 'o2', label: tx('Target colour', 'Màu mục tiêu'), kind: 'colour', value: '#22c55e', default: '#22c55e' },
        { name: 'o3', label: tx('Marker colour', 'Màu vạch'), kind: 'colour', value: '#ffffff', default: '#ffffff' },
        { name: 'o4', label: tx('Strength', 'Lực'), kind: 'number', value: 1, default: 1, min: 0, max: 10, unit: '' },
        { name: 'o5', label: tx('Wait for the bar', 'Chờ thanh'), kind: 'number', value: 60000, default: 60000, min: 1000, max: 600000, unit: 's' }],
      areas: [{ name: 'a1', label: tx('Bar area', 'Vùng thanh'), colour: '#3b82f6', rect: { x: 0.3, y: 0.75, w: 0.4, h: 0.06 } }],
      steps: [{ kind: 'loop', times: 0 },
        { kind: 'click', x: 0.5, y: 0.5, button: 'left', holdMs: '$o1', todo: ['x'], note: tx('Holds to cast: drag dot 2 to where you cast', 'Giữ để quăng: kéo chấm 2 tới chỗ quăng') },
        { kind: 'follow', region: '@a1', target: '$o2', marker: '$o3', tolerance: 24, button: 'left', mode: 'smooth', kp: '$o4', kd: 0.4, predict: true, fps: 60,
          untilGoneMs: 1000, timeoutMs: '$o5', todo: ['region', 'target', 'marker'], note: tx('Keeps the marker on the target until the bar goes', 'Giữ vạch trên mục tiêu đến khi thanh biến mất') },
        { kind: 'addVar', name: 'catches', value: 1, note: tx('Counts each catch', 'Đếm số lần câu được') },
        { kind: 'wait', ms: 1500, note: tx('A short break before the next cast', 'Nghỉ một chút trước lần quăng sau') }, { kind: 'endLoop' }] },
    { ...base, id: '', name: tx('Rejoin and walk back', 'Vào lại rồi đi về chỗ cũ'), icon: 'rejoin', maxMinutes: 10, targets: 'all', trigger: { kind: 'rejoin' },
      about: tx('After a rejoin, waits for the game to load, walks back and presses E.', 'Sau khi vào lại, chờ game tải, đi về chỗ cũ rồi bấm E.'),
      steps: [{ kind: 'wait', ms: 8000, note: tx('Give the game time to load', 'Chờ game tải xong') },
        { kind: 'key', key: 'W', ms: 2000, note: tx('Walks forward for 2 s: change it to your way back', 'Đi tới 2 giây: đổi theo đường về của bạn') },
        { kind: 'wait', ms: 1000, note: tx('Stops for a moment', 'Dừng một chút') },
        { kind: 'key', key: 'E', ms: 60, note: tx('Uses what is there', 'Dùng thứ ở đó') }] },
    { ...base, id: '', name: tx('New macro', 'Macro mới'), icon: 'blank', maxMinutes: 10, about: tx('Start from nothing and build your own.', 'Bắt đầu từ đầu và tự làm.'), steps: [] },
  ] });
})();
