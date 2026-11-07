// The animated themes: what moves over a theme's still picture (scene-<id>.svg), on one canvas under the page (#live-scene).
// It runs (at 30 frames a second) only while the window is in use: never in the tray or behind Roblox, so it costs nothing
// while accounts play. Windows' "animation effects" setting doesn't stop it: picking one of these themes is asking for it,
// and many players turn that setting off for speed alone. Everything here draws in the picture's own 1600 × 1000 space,
// fitted like the picture's "cover".
(() => {
  const TAU = Math.PI * 2, W = 1600, H = 1000;
  const rng = seed => () => (seed = seed * 16807 % 2147483647) / 2147483647;
  const pick = (r, list) => list[Math.floor(r() * list.length)];
  const near = (a, b, d) => (a.x - b.x) ** 2 + (a.y - b.y) ** 2 < d * d;

  function rippler(colour) {
    const rings = [];
    return {
      add(x, y, size = 90) { if (rings.length < 14) rings.push({ x, y, size, age: 0 }); },
      draw(c, dt) {
        c.strokeStyle = colour;
        for (let i = rings.length - 1; i >= 0; i--) {
          const q = rings[i], k = (q.age += dt) / 2.4;
          if (k >= 1) { rings.splice(i, 1); continue; }
          c.globalAlpha = (1 - k) * .55;
          c.lineWidth = 2.5;
          for (const f of [1, .62]) {
            c.beginPath();
            c.ellipse(q.x, q.y, q.size * k * f + 2, (q.size * k * f + 2) * .62, 0, 0, TAU);
            c.stroke();
          }
        }
        c.globalAlpha = 1;
      },
    };
  }

  // The picture of a photo scene, moved by the graphics card: water that ripples below its horizon, the sway of the sea, a
  // slow drift. It lies under the scene's own canvas; without WebGL the theme's still picture stays, and the rest still runs.
  const PHOTO_SHADER = `precision mediump float;
    varying vec2 v;
    uniform sampler2D img;
    uniform vec2 scale, ptr;
    uniform vec4 still;
    uniform float t, horizon, waves, wobble, drift, dim, grade, on, aspect;
    float edge(float a, float b, float x) { return smoothstep(a - .02, a + .02, x) * (1. - smoothstep(b - .02, b + .02, x)); }
    void main() {
      vec2 uv = (v - .5) * scale + .5, moved = uv;
      if (drift > 0.) moved = (uv - .5) / (1.05 + sin(t * .07) * .02 * drift) + .5 + vec2(sin(t * .05), cos(t * .04)) * .007 * drift;
      if (wobble > 0.) moved += vec2(sin(uv.y * 9. + t * .7) + sin(uv.y * 23. - t * .4) * .3, cos(uv.x * 8. + t * .55)) * .0035 * wobble;
      float k = clamp((uv.y - horizon) / (1. - horizon), 0., 1.);
      if (waves > 0. && k > 0.) {
        float calm = 1. - edge(still.x, still.z, uv.x) * edge(still.y, still.w, uv.y);
        float phase = uv.y * (70. + 90. * (1. - k)) - t * 1.5 + sin(uv.x * 11. + t * .5) * 2.2;
        float wave = sin(phase) + sin(phase * .47 + uv.x * 31. + t) * .6;
        moved += vec2(wave * .0034, wave * .0016) * (.25 + k) * waves * calm;
      }
      if (on > 0. && (waves == 0. || k > 0.) && (waves > 0. || wobble > 0.)) {
        vec2 away = (uv - ptr) * vec2(aspect, 1.);
        float d = length(away);
        moved += away / (d + .001) * sin(d * 90. - t * 5.) * exp(-d * 11.) * .006 * on;
      }
      vec3 c = texture2D(img, moved).rgb * dim;
      c = mix(c, c * vec3(1.12, .82, 1.16) + vec3(.03, 0., .05), grade * (1. - uv.y * .6));
      gl_FragColor = vec4(c, 1.);
    }`;

  function photoLayer(url, o, before) {
    const cv = document.createElement('canvas');
    const gl = cv.getContext('webgl', { alpha: true, antialias: false, depth: false, stencil: false, powerPreference: 'low-power' });
    if (!gl) return null;
    const shader = (type, text) => { const s = gl.createShader(type); gl.shaderSource(s, text); gl.compileShader(s); return s; };
    const prog = gl.createProgram();
    gl.attachShader(prog, shader(gl.VERTEX_SHADER, 'attribute vec2 p; varying vec2 v; void main() { v = vec2(p.x * .5 + .5, .5 - p.y * .5); gl_Position = vec4(p, 0., 1.); }'));
    gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, PHOTO_SHADER));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return null;
    gl.useProgram(prog);
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.bindAttribLocation(prog, 0, 'p');
    gl.linkProgram(prog);
    gl.useProgram(prog);
    gl.vertexAttribPointer(gl.getAttribLocation(prog, 'p'), 2, gl.FLOAT, false, 0, 0);
    const u = name => gl.getUniformLocation(prog, name);
    const [time, scale, ptr, on, aspect] = ['t', 'scale', 'ptr', 'on', 'aspect'].map(u);
    gl.uniform1f(u('horizon'), o.horizon ?? 1);
    gl.uniform1f(u('waves'), o.waves ?? 0);
    gl.uniform1f(u('wobble'), o.wobble ?? 0);
    gl.uniform1f(u('drift'), o.drift ?? 0);
    gl.uniform1f(u('dim'), o.dim ?? 1);
    gl.uniform1f(u('grade'), o.grade ?? 0);
    gl.uniform4fv(u('still'), o.still ?? [0, 0, 0, 0]);
    let ready = false, ease = 0;
    const picture = new Image();
    picture.onload = () => {
      gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, picture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      ready = true;
    };
    picture.src = url;
    cv.id = 'live-photo';
    cv.setAttribute('aria-hidden', 'true');
    before.before(cv);
    cv.addEventListener('webglcontextlost', () => { ready = false; cv.remove(); });
    return {
      size(w, h, res, s) {
        cv.width = Math.round(w * res);
        cv.height = Math.round(h * res);
        gl.viewport(0, 0, cv.width, cv.height);
        gl.uniform2f(scale, w / (W * s), h / (H * s));
        gl.uniform1f(aspect, W / H);
      },
      draw(t, p, dt) {
        if (!ready) return;
        ease += ((p.on ? 1 : 0) - ease) * Math.min(1, dt * 4);
        gl.uniform1f(time, t);
        gl.uniform2f(ptr, p.x / W, p.y / H);
        gl.uniform1f(on, ease);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
      },
      remove() {
        cv.remove();
        gl.getExtension('WEBGL_lose_context')?.loseContext();
      },
    };
  }

  const SCENES = {
    // Koi from above: they scatter from the pointer and come to where you click, as to food.
    koipond() {
      const r = rng(11), rings = rippler('#d9f2ec'), food = [];
      const LOOKS = [['#f7f2e6', '#e8622c'], ['#f0a32b', '#f9dc8c'], ['#f7f2e6', '#c8262c'], ['#22222a', '#f2efe6'], ['#f7f3ea', '#f08a2a'], ['#e8622c', '#f7efe0'], ['#f4c542', '#fff4cc'], ['#f7f2e6', '#30303a']];
      const fish = LOOKS.map(([base, patch], i) => {
        const x = 200 + r() * 1200, y = 150 + r() * 700, size = 15 + r() * 8;
        return { x, y, a: r() * TAU, turn: 0, base, patch, size, seed: i * 1.7, speed: 45 + r() * 30, pts: Array.from({ length: 11 }, () => ({ x, y })) };
      });
      const PADS = [[120, 120, 74, 1], [235, 215, 46, 0], [60, 330, 56, 0], [1490, 110, 80, 1], [1370, 70, 44, 0], [1530, 300, 58, 0], [1440, 860, 76, 1], [1300, 930, 50, 0], [1550, 700, 46, 0], [120, 880, 70, 1], [270, 945, 46, 0], [690, 950, 52, 0], [800, 60, 48, 1], [640, 40, 36, 0], [1040, 520, 40, 0], [470, 470, 36, 1]];
      const bodyPath = (c, f, dx, dy) => {
        const n = f.pts.length, side = [];
        for (let i = 0; i < n; i++) {
          const p = f.pts[i], q = f.pts[Math.min(i + 1, n - 1)], o = f.pts[Math.max(i - 1, 0)];
          const ang = Math.atan2(o.y - q.y, o.x - q.x) + Math.PI / 2, w = f.size * Math.pow(Math.sin(Math.PI * (i + .9) / (n + .6)), .8);
          side.push([p.x + Math.cos(ang) * w + dx, p.y + Math.sin(ang) * w + dy, p.x - Math.cos(ang) * w + dx, p.y - Math.sin(ang) * w + dy]);
        }
        c.beginPath();
        c.moveTo(side[0][0], side[0][1]);
        for (let i = 1; i < n; i++) c.quadraticCurveTo(side[i - 1][0], side[i - 1][1], (side[i - 1][0] + side[i][0]) / 2, (side[i - 1][1] + side[i][1]) / 2);
        c.lineTo(side[n - 1][2], side[n - 1][3]);
        for (let i = n - 2; i >= 0; i--) c.quadraticCurveTo(side[i + 1][2], side[i + 1][3], (side[i + 1][2] + side[i][2]) / 2, (side[i + 1][3] + side[i][3]) / 2);
        c.closePath();
      };
      const fin = (c, p, ang, len, wid) => {
        c.beginPath();
        c.moveTo(p.x, p.y);
        c.quadraticCurveTo(p.x + Math.cos(ang - wid) * len, p.y + Math.sin(ang - wid) * len, p.x + Math.cos(ang) * len * 1.25, p.y + Math.sin(ang) * len * 1.25);
        c.quadraticCurveTo(p.x + Math.cos(ang + wid) * len, p.y + Math.sin(ang + wid) * len, p.x, p.y);
        c.fill();
      };
      return {
        frame(c, t, dt, p) {
          if (r() < dt * .3) rings.add(r() * W, r() * H, 60);
          rings.draw(c, dt);
          for (let i = food.length - 1; i >= 0; i--) {
            if ((food[i].age += dt) > 6) { food.splice(i, 1); continue; }
            c.fillStyle = '#d8b878';
            c.beginPath();
            for (const [dx, dy] of food[i].bits) { c.moveTo(food[i].x + dx, food[i].y + dy); c.arc(food[i].x + dx, food[i].y + dy, 3, 0, TAU); }
            c.fill();
          }
          for (const f of fish) {
            let want = f.a + Math.sin(t * .4 + f.seed) * .9 + Math.sin(t * .13 + f.seed * 3) * .9, boost = 1;
            const goal = food.length ? food.reduce((a, b) => (near(f, b, Math.hypot(f.x - a.x, f.y - a.y)) ? b : a)) : null;
            if (goal) { want = Math.atan2(goal.y - f.y, goal.x - f.x); boost = 1.9; }
            else if (p.on && near(f, p, 170)) { want = Math.atan2(f.y - p.y, f.x - p.x); boost = 3; }
            const m = 110;
            if (f.x < m || f.x > W - m || f.y < m || f.y > H - m) want = Math.atan2(H / 2 - f.y, W / 2 - f.x);
            let d = (want - f.a) % TAU;
            if (d > Math.PI) d -= TAU; else if (d < -Math.PI) d += TAU;
            f.a += Math.max(-1.6, Math.min(1.6, d * 1.6)) * dt;
            const v = f.speed * boost * (1 + .22 * Math.sin(t * 2.4 + f.seed));
            f.x += Math.cos(f.a) * v * dt; f.y += Math.sin(f.a) * v * dt;
            if (goal && near(f, goal, 24)) goal.age += dt * 3;
            f.pts[0].x = f.x; f.pts[0].y = f.y;
            const gap = f.size * .95;
            for (let i = 1; i < f.pts.length; i++) {
              const a = f.pts[i - 1], b = f.pts[i], l = Math.hypot(b.x - a.x, b.y - a.y) || 1;
              b.x = a.x + (b.x - a.x) * gap / l; b.y = a.y + (b.y - a.y) * gap / l;
            }
            c.fillStyle = 'rgba(0, 14, 16, .28)';
            bodyPath(c, f, 9, 13);
            c.fill();
            const tail = f.pts[f.pts.length - 1], back = Math.atan2(tail.y - f.pts[8].y, tail.x - f.pts[8].x), wag = Math.sin(t * 5 + f.seed) * .5;
            c.fillStyle = f.base;
            c.globalAlpha = .85;
            fin(c, tail, back + wag, f.size * 1.9, .55);
            const chest = f.pts[2], head = Math.atan2(f.pts[1].y - chest.y, f.pts[1].x - chest.x);
            fin(c, chest, head + 2.2, f.size * 1.2, .4);
            fin(c, chest, head - 2.2, f.size * 1.2, .4);
            c.globalAlpha = 1;
            bodyPath(c, f, 0, 0);
            c.fill();
            c.fillStyle = f.patch;
            for (const [i, k] of [[1, .55], [3, .78], [5, .62], [7, .5]]) {
              const q = f.pts[i];
              c.beginPath();
              c.ellipse(q.x, q.y, f.size * k, f.size * k * .72, f.seed + i, 0, TAU);
              c.fill();
            }
            c.fillStyle = '#10131a';
            const eye = f.a + Math.PI / 2, e = f.pts[0];
            c.beginPath();
            c.arc(e.x + Math.cos(eye) * f.size * .45, e.y + Math.sin(eye) * f.size * .45, 2.2, 0, TAU);
            c.arc(e.x - Math.cos(eye) * f.size * .45, e.y - Math.sin(eye) * f.size * .45, 2.2, 0, TAU);
            c.fill();
          }
          for (const [i, [x, y, s, bloom]] of PADS.entries()) {
            const a = Math.sin(t * .35 + i) * .06 + i * 1.3;
            c.fillStyle = i % 3 ? '#2f7a4f' : '#3a8c58';
            c.beginPath();
            c.moveTo(x, y);
            c.arc(x, y, s, a + .22, a + TAU - .22);
            c.closePath();
            c.fill();
            c.strokeStyle = 'rgba(190, 235, 190, .3)';
            c.lineWidth = 1.5;
            c.beginPath();
            for (let k = 1; k < 6; k++) { const b = a + k * TAU / 6; c.moveTo(x, y); c.lineTo(x + Math.cos(b) * s * .86, y + Math.sin(b) * s * .86); }
            c.stroke();
            if (bloom) {
              for (const [petals, rad, colour] of [[8, s * .42, '#f4a8bf'], [6, s * .3, '#fbd6df'], [5, s * .16, '#ffe9a8']]) {
                c.fillStyle = colour;
                c.beginPath();
                for (let k = 0; k < petals; k++) {
                  const b = k * TAU / petals + t * .05;
                  c.ellipse(x + Math.cos(b) * rad * .6, y + Math.sin(b) * rad * .6, rad * .55, rad * .28, b, 0, TAU);
                  c.closePath();
                }
                c.fill();
              }
            }
          }
        },
        click(x, y) {
          rings.add(x, y, 150);
          food.push({ x, y, age: 0, bits: Array.from({ length: 7 }, () => [(r() - .5) * 44, (r() - .5) * 44]) });
          if (food.length > 3) food.shift();
        },
      };
    },

    // Fireflies blink and drift, come towards the pointer, and scatter at a click. The grass sways.
    fireflies() {
      const r = rng(23);
      const flies = Array.from({ length: 56 }, () => ({ x: r() * W, y: 300 + r() * 650, vx: 0, vy: 0, ph: r() * TAU, sp: .7 + r() * 1.3, wa: r() * TAU }));
      const blades = Array.from({ length: 150 }, () => ({ x: r() * W, h: 26 + r() * 74, ph: r() * TAU, y: 1004 - r() * 26 }));
      return {
        frame(c, t, dt, p) {
          c.strokeStyle = '#05100f';
          c.lineWidth = 3;
          c.lineCap = 'round';
          c.beginPath();
          for (const b of blades) {
            const sway = Math.sin(t * 1.1 + b.ph + b.x * .004) * b.h * .24;
            c.moveTo(b.x, b.y);
            c.quadraticCurveTo(b.x + sway * .3, b.y - b.h * .6, b.x + sway, b.y - b.h);
          }
          c.stroke();
          for (const f of flies) {
            f.wa += (r() - .5) * 1.8 * dt * 4;
            f.vx += Math.cos(f.wa) * 22 * dt; f.vy += Math.sin(f.wa) * 22 * dt;
            if (p.on && near(f, p, 300)) { f.vx += (p.x - f.x) * .35 * dt; f.vy += (p.y - f.y) * .35 * dt; }
            f.vx *= 1 - .9 * dt; f.vy *= 1 - .9 * dt;
            f.x += f.vx * dt; f.y += f.vy * dt;
            if (f.x < -20) f.x = W + 20; else if (f.x > W + 20) f.x = -20;
            if (f.y < 200) f.vy += 30 * dt; else if (f.y > 960) f.vy -= 30 * dt;
            const k = Math.max(0, Math.sin(t * f.sp + f.ph)) ** 2 * .75 + .25;
            for (const [rad, a, colour] of [[30, .1, '#c9e65a'], [15, .24, '#d6ee66'], [5, 1, '#f6ffc0']]) {
              c.globalAlpha = a * k;
              c.fillStyle = colour;
              c.beginPath();
              c.arc(f.x, f.y, rad, 0, TAU);
              c.fill();
            }
          }
          c.globalAlpha = 1;
        },
        click(x, y) {
          for (const f of flies) {
            const d = Math.hypot(f.x - x, f.y - y);
            if (d < 300) { f.vx += (f.x - x) / (d + 1) * 150; f.vy += (f.y - y) / (d + 1) * 150; f.ph = Math.PI / 2 - f.sp; }
          }
        },
      };
    },

    // Real photographs (photo-<id>.jpg, cut to the scenes' 1600 x 1000; see THEME-PHOTOS.txt): `photo` says how the picture
    // itself moves (photoLayer), and what's drawn here goes over it, in the picture's own places.

    // Ocean Drive at dusk: traffic streaks by, the neon hums, and a click brings the red and blue lights of a patrol car.
    miaminight() {
      const r = rng(61);
      const ROADS = [[0, 1000, 1458, 883], [416, 998, 1250, 868], [1600, 850, 700, 950]];
      const cars = Array.from({ length: 7 }, (_, i) => ({ road: ROADS[i % 3], k: r(), sp: .1 + r() * .16, colour: i % 3 === 2 ? '#fff0c8' : pick(r, ['#ff3b30', '#ff5a3c', '#ffb14d']) }));
      const NEON = [[858, 208, 12, 396, '#7fe6ff'], [956, 240, 10, 364, '#d9f6ff'], [812, 208, 10, 396, '#e8fbff'], [1148, 588, 18, 62, '#58d6ff'], [1148, 642, 18, 40, '#58d6ff'], [1370, 560, 22, 150, '#5ccaff'], [1450, 708, 70, 22, '#ff7a2e']];
      let flash = 0, at = { x: 800, y: 800 };
      return {
        photo: { drift: 0, grade: .55 },
        frame(c, t, dt) {
          c.globalCompositeOperation = 'lighter';
          for (const [i, [x, y, w, h, colour]] of NEON.entries()) {
            const hum = .16 + Math.sin(t * 2.1 + i) * .05 + (Math.sin(t * 31 + i * 7) > .985 ? -.12 : 0);
            c.globalAlpha = Math.max(.02, hum);
            c.fillStyle = colour;
            c.fillRect(x - 8, y - 8, w + 16, h + 16);
            c.globalAlpha = Math.max(.02, hum) * 1.5;
            c.fillRect(x, y, w, h);
          }
          c.lineCap = 'round';
          for (const k of cars) {
            k.k += k.sp * dt;
            if (k.k > 1.25) k.k = -.25;
            const [x0, y0, x1, y1] = k.road;
            const at2 = q => [x0 + (x1 - x0) * q, y0 + (y1 - y0) * q];
            const [hx, hy] = at2(k.k), [tx, ty] = at2(k.k - .16);
            c.strokeStyle = k.colour;
            for (const [w, a] of [[11, .1], [5, .32], [2, .9]]) {
              c.globalAlpha = a;
              c.lineWidth = w;
              c.beginPath();
              c.moveTo(tx, ty);
              c.lineTo(hx, hy);
              c.stroke();
            }
          }
          if (flash > 0) {
            flash = Math.max(0, flash - dt);
            const red = Math.floor(flash * 7) % 2 === 0;
            c.fillStyle = red ? '#ff1f3d' : '#1f6bff';
            c.globalAlpha = .1 * Math.min(1, flash);
            c.fillRect(0, 0, W, H);
            c.globalAlpha = .34 * Math.min(1, flash);
            c.beginPath();
            c.arc(at.x + (red ? -60 : 60), at.y, 150, 0, TAU);
            c.fill();
          }
          c.globalAlpha = 1;
          c.globalCompositeOperation = 'source-over';
        },
        click(x, y) { flash = 2.4; at = { x, y }; },
      };
    },

    // The Thu Bồn at Hội An: the water moves under the boats, paper lanterns drift by, and a click sets a new one down.
    lanternriver() {
      const r = rng(83), TOP = 560;
      const lantern = (x, y) => ({ x, y, sp: 5 + r() * 9, ph: r() * TAU, hue: pick(r, ['#ffb347', '#ff9d5c', '#ffc76b', '#ff7f6b']) });
      const list = Array.from({ length: 11 }, () => lantern(r() * W, TOP + 60 + r() * (H - TOP - 90))).sort((a, b) => a.y - b.y);
      const rings = rippler('#ffd9a0');
      return {
        photo: { horizon: .535, waves: 1, still: [.565, .64, 1, .81] },
        frame(c, t, dt) {
          rings.draw(c, dt);
          for (const l of list) {
            l.x += l.sp * dt;
            if (l.x > W + 40) l.x = -40;
            const s = .3 + (l.y - TOP) / (H - TOP) * .55, flick = .85 + Math.sin(t * 7 + l.ph * 3) * .1 + Math.sin(t * 13 + l.ph) * .05;
            c.fillStyle = l.hue;
            for (let i = 0; i < 5; i++) {
              c.globalAlpha = (.34 - i * .06) * flick;
              c.fillRect(l.x - (15 - i) * s, l.y + 12 * s + i * 11 * s, (30 - i * 2) * s, 4 * s);
            }
            c.globalAlpha = .16 * flick;
            c.beginPath();
            c.arc(l.x, l.y - 8 * s, 34 * s, 0, TAU);
            c.fill();
            c.globalAlpha = 1;
            c.beginPath();
            c.moveTo(l.x - 15 * s, l.y);
            c.lineTo(l.x - 11 * s, l.y - 24 * s);
            c.lineTo(l.x + 11 * s, l.y - 24 * s);
            c.lineTo(l.x + 15 * s, l.y);
            c.closePath();
            c.fill();
            c.fillStyle = '#fff1cf';
            c.globalAlpha = flick;
            c.fillRect(l.x - 5 * s, l.y - 18 * s, 10 * s, 14 * s);
            c.globalAlpha = 1;
          }
        },
        click(x, y) {
          if (y < TOP) return;
          rings.add(x, y);
          list.push(lantern(x, y));
          if (list.length > 24) list.splice(list.reduce((far, l, i) => l.x > list[far].x ? i : far, 0), 1);
          list.sort((a, b) => a.y - b.y);
        },
      };
    },

    // Jellyfish in deep water: the whole picture sways like seen through the sea, specks sink, bubbles rise.
    deepsea() {
      const r = rng(97);
      const specks = Array.from({ length: 90 }, () => ({ x: r() * W, y: r() * H, s: 1 + r() * 2, sp: 4 + r() * 10, ph: r() * TAU }));
      const bubbles = [];
      const bubble = (x, y) => bubbles.push({ x, y, s: 2 + r() * 7, sp: 40 + r() * 70, ph: r() * TAU });
      return {
        photo: { drift: 1, wobble: 1, dim: .82 },
        frame(c, t, dt) {
          c.fillStyle = '#d8f0ff';
          for (const s of specks) {
            s.y += s.sp * dt;
            if (s.y > H) s.y = -4;
            c.globalAlpha = .2 + Math.sin(t + s.ph) * .1;
            c.fillRect(s.x + Math.sin(t * .4 + s.ph) * 12, s.y, s.s, s.s);
          }
          c.globalAlpha = 1;
          if (r() < dt * 1.4) bubble(r() * W, H + 10);
          c.strokeStyle = '#e4f6ff';
          c.lineWidth = 1.5;
          c.globalAlpha = .6;
          for (let i = bubbles.length - 1; i >= 0; i--) {
            const b = bubbles[i];
            b.y -= b.sp * dt;
            if (b.y < -20) { bubbles.splice(i, 1); continue; }
            c.beginPath();
            c.arc(b.x + Math.sin(t * 2 + b.ph) * 8, b.y, b.s, 0, TAU);
            c.stroke();
          }
          c.globalAlpha = 1;
        },
        click(x, y) {
          for (let i = 0; i < 14; i++) bubble(x + (r() - .5) * 46, y + (r() - .5) * 46);
          if (bubbles.length > 90) bubbles.splice(0, bubbles.length - 90);
        },
      };
    },

    // A road through a snowy forest: snow blows on a changing wind, parts round the pointer, and a click sends a gust.
    snowfall() {
      const r = rng(41);
      const flakes = [[130, 1.5, 34, .5], [100, 2.6, 62, .75], [40, 4.4, 104, 1]].flatMap(([n, s, sp, a]) =>
        Array.from({ length: n }, () => ({ x: r() * W, y: r() * H, s: s * (.8 + r() * .4), sp, a, ph: r() * TAU, vx: 0, vy: 0 })));
      return {
        photo: { drift: 1, dim: .6 },
        frame(c, t, dt, p) {
          const wind = Math.sin(t * .17) * 34 + Math.sin(t * .41) * 16;
          c.fillStyle = '#ffffff';
          for (const f of flakes) {
            if (p.on && near(f, p, 130)) {
              const d = Math.hypot(f.x - p.x, f.y - p.y) + 1;
              f.vx += (f.x - p.x) / d * 420 * dt; f.vy += (f.y - p.y) / d * 420 * dt;
            }
            f.vx *= 1 - 1.6 * dt; f.vy *= 1 - 1.6 * dt;
            f.x += (wind * f.a + Math.sin(t * 1.1 + f.ph) * 14 + f.vx) * dt;
            f.y += (f.sp + f.vy) * dt;
            if (f.y > H + 6) { f.y = -6; f.x = r() * W; }
            if (f.x < -8) f.x = W + 8; else if (f.x > W + 8) f.x = -8;
            c.globalAlpha = f.a;
            c.beginPath();
            c.arc(f.x, f.y, f.s, 0, TAU);
            c.fill();
          }
          c.globalAlpha = 1;
        },
        click(x, y) {
          for (const f of flakes) {
            const d = Math.hypot(f.x - x, f.y - y);
            if (d < 360) { const k = (360 - d) / 360 * 620; f.vx += (f.x - x) / (d + 1) * k; f.vy += (f.y - y) / (d + 1) * k; }
          }
        },
      };
    },

    // A harbour at night: the water moves, ferries' lights cross it, the towers' lights blink, a click sets off a firework.
    nightharbour() {
      const r = rng(73);
      const boats = Array.from({ length: 6 }, (_, i) => ({ x: r() * W, y: 700 + i * 22 + r() * 12, sp: (20 + r() * 34) * (i % 2 ? 1 : -1), len: 90 + r() * 170, colour: pick(r, ['#ffd27a', '#ffb45c', '#ffffff', '#ffc9f2']) }));
      const BLINK = [[846, 360], [985, 480], [1100, 550], [1306, 578], [1195, 580], [404, 470], [190, 528]];
      const sparks = [];
      const rings = rippler('#e8ddff');
      return {
        photo: { horizon: .685, waves: 1 },
        frame(c, t, dt) {
          rings.draw(c, dt);
          c.globalCompositeOperation = 'lighter';
          c.lineCap = 'round';
          for (const b of boats) {
            b.x += b.sp * dt;
            if (b.x > W + b.len) b.x = -b.len; else if (b.x < -b.len) b.x = W + b.len;
            c.strokeStyle = b.colour;
            for (const [w, a] of [[7, .12], [2.5, .7]]) {
              c.globalAlpha = a;
              c.lineWidth = w;
              c.beginPath();
              c.moveTo(b.x, b.y);
              c.lineTo(b.x - Math.sign(b.sp) * b.len, b.y + 1);
              c.stroke();
            }
          }
          for (const [i, [x, y]] of BLINK.entries()) {
            c.globalAlpha = Math.max(0, Math.sin(t * 1.5 + i * 1.9)) ** 6 * .9;
            c.fillStyle = '#ff3b3b';
            c.beginPath();
            c.arc(x, y, 5, 0, TAU);
            c.fill();
            c.globalAlpha *= .22;
            c.beginPath();
            c.arc(x, y, 17, 0, TAU);
            c.fill();
          }
          for (let i = sparks.length - 1; i >= 0; i--) {
            const s = sparks[i];
            s.vy += 90 * dt;
            s.vx *= 1 - .8 * dt; s.vy *= 1 - .8 * dt;
            s.x += s.vx * dt; s.y += s.vy * dt;
            if ((s.life -= dt) <= 0) { sparks.splice(i, 1); continue; }
            c.globalAlpha = Math.min(1, s.life * 1.4);
            c.fillStyle = s.colour;
            c.beginPath();
            c.arc(s.x, s.y, 2.6, 0, TAU);
            c.fill();
          }
          c.globalAlpha = 1;
          c.globalCompositeOperation = 'source-over';
        },
        click(x, y) {
          if (y > 690) { rings.add(x, y); return; }
          const colour = pick(r, ['#ff6b8b', '#ffd166', '#7ce8b5', '#7cc8ff', '#d1a6ff']);
          for (let i = 0; i < 70; i++) {
            const a = r() * TAU, v = 50 + r() * 240;
            sparks.push({ x, y, vx: Math.cos(a) * v, vy: Math.sin(a) * v, life: .9 + r() * 1.1, colour: r() < .15 ? '#ffffff' : colour });
          }
          if (sparks.length > 280) sparks.splice(0, sparks.length - 280);
        },
      };
    },

    // Rain on a window: drops gather, slide down, swallow the small ones and leave a trail. The pointer wipes the glass.
    rainglass() {
      const r = rng(31), small = [], runs = [];
      const fall = Array.from({ length: 46 }, () => ({ x: r() * W, y: r() * H, sp: 1100 + r() * 700, l: 30 + r() * 50 }));
      const addRun = () => runs.push({ x: r() * W, y: -20 - r() * 200, s: 7 + r() * 6, v: 40, wait: 0, wob: r() * TAU });
      for (let i = 0; i < 260; i++) small.push({ x: r() * W, y: r() * H, s: 1.4 + r() * 3 });
      return {
        frame(c, t, dt, p) {
          c.strokeStyle = 'rgba(190, 210, 240, .13)';
          c.lineWidth = 1.5;
          c.beginPath();
          for (const f of fall) {
            f.y += f.sp * dt; f.x -= f.sp * dt * .08;
            if (f.y > H + 40) { f.y = -f.l; f.x = r() * (W + 100); }
            c.moveTo(f.x, f.y); c.lineTo(f.x - f.l * .08, f.y + f.l);
          }
          c.stroke();
          for (let i = 0; i < 5 && small.length < 340; i++) if (r() < dt * 7) small.push({ x: r() * W, y: r() * H, s: 1.4 + r() * 3 });
          if (runs.length < 15 && r() < dt * 1.2) addRun();
          for (let i = runs.length - 1; i >= 0; i--) {
            const d = runs[i];
            if ((d.wait -= dt) > 0) continue;
            if (r() < dt * .7) d.wait = r() * 1.1;
            d.v = Math.min(420, d.v + 500 * dt);
            if (d.wait > 0) d.v = 30;
            d.y += d.v * dt;
            d.x += Math.sin(t * 2.2 + d.wob) * 22 * dt;
            for (let k = small.length - 1; k >= 0; k--) {
              const q = small[k];
              if (Math.abs(q.x - d.x) < d.s + q.s && Math.abs(q.y - d.y) < d.s * 1.6 + q.s) { small.splice(k, 1); d.s = Math.min(13, d.s + .03); }
            }
            if (small.length < 340 && r() < dt * 16) small.push({ x: d.x + (r() - .5) * 3, y: d.y - d.s * 2, s: .9 + r() * 1.5 });
            if (d.y > H + 30) runs.splice(i, 1);
          }
          if (p.on) {
            for (let k = small.length - 1; k >= 0; k--) if (near(small[k], p, 60)) small.splice(k, 1);
            for (let k = runs.length - 1; k >= 0; k--) if (near(runs[k], p, 50)) runs.splice(k, 1);
          }
          c.fillStyle = 'rgba(205, 222, 250, .5)';
          c.beginPath();
          for (const q of small) { c.moveTo(q.x + q.s, q.y); c.arc(q.x, q.y, q.s, 0, TAU); }
          c.fill();
          c.fillStyle = 'rgba(20, 30, 52, .55)';
          c.beginPath();
          for (const d of runs) { c.moveTo(d.x + d.s, d.y); c.ellipse(d.x, d.y, d.s, d.s * 1.35, 0, 0, TAU); }
          c.fill();
          c.fillStyle = 'rgba(220, 234, 255, .75)';
          c.beginPath();
          for (const d of runs) { c.moveTo(d.x + d.s * .3, d.y + d.s * .55); c.ellipse(d.x, d.y + d.s * .55, d.s * .55, d.s * .45, 0, 0, TAU); }
          c.fill();
          c.fillStyle = 'rgba(255, 255, 255, .85)';
          c.beginPath();
          for (const d of runs) { c.moveTo(d.x - d.s * .1, d.y - d.s * .55); c.arc(d.x - d.s * .35, d.y - d.s * .55, d.s * .22, 0, TAU); }
          c.fill();
        },
        click(x, y) {
          for (let i = 0; i < 4; i++) runs.push({ x: x + (r() - .5) * 90, y: y + (r() - .5) * 40, s: 5 + r() * 5, v: 60, wait: 0, wob: r() * TAU });
        },
      };
    },

    // A world of blocks: clouds drift, the windmill turns, little block people hop along, and a click drops a block.
    blockworld() {
      const r = rng(41), G = 40;
      const ground = x => 640 - G * Math.round(2.4 * Math.sin(Math.floor(x / G) * .33) + 1.5 * Math.sin(Math.floor(x / G) * .11 + 1));
      const clouds = Array.from({ length: 9 }, (_, i) => ({ x: r() * (W + 400) - 200, y: 40 + (i % 5) * 62 + r() * 26, sp: 9 + r() * 14, parts: Array.from({ length: 3 + Math.floor(r() * 3) }, (_, k) => [k * G * 1.6, Math.floor(r() * 2) * -G * .6, G * (1.6 + Math.floor(r() * 2) * .8)]) }));
      const LOOK = [['#3b82f6', '#22458f'], ['#ef4444', '#8f2222'], ['#22c55e', '#176f38'], ['#a855f7', '#5d2a8f']];
      const folks = LOOK.map(([shirt, legs], i) => ({ x: 340 + i * 330, dir: i % 2 ? -1 : 1, shirt, legs, y: 0, vy: 0, wait: r() * 2, sp: 38 + r() * 26 }));
      const blocks = [], COLOURS = ['#e8b23a', '#d9503f', '#4f8fe0', '#8d5fd3', '#f08a3c', '#35b58a'];
      const cube = (c, x, y, s, face, top, side) => {
        c.fillStyle = face; c.fillRect(x, y, s, s);
        c.fillStyle = top; c.fillRect(x, y, s, s * .22);
        c.fillStyle = side; c.fillRect(x + s * .8, y + s * .22, s * .2, s * .78);
      };
      const stacked = col => blocks.filter(b => b.col === col && b.rest).length;
      return {
        frame(c, t, dt) {
          for (const k of clouds) {
            k.x += k.sp * dt;
            if (k.x > W + 60) k.x = -420;
            for (const [dx, dy, s] of k.parts) {
              c.fillStyle = 'rgba(255, 255, 255, .92)'; c.fillRect(k.x + dx, k.y + dy, s * 1.5, s * .7);
              c.fillStyle = 'rgba(190, 220, 240, .9)'; c.fillRect(k.x + dx, k.y + dy + s * .7, s * 1.5, s * .16);
            }
          }
          const mx = 1218, my = ground(1218) - 200, spin = t * .9;
          for (let i = 0; i < 4; i++) {
            c.save();
            c.translate(mx, my);
            c.rotate(spin + i * TAU / 4);
            c.fillStyle = '#fff6e0'; c.fillRect(10, -14, 118, 28);
            c.fillStyle = '#d9503f'; c.fillRect(0, -5, 124, 10);
            c.restore();
          }
          c.fillStyle = '#6b4a2e'; c.fillRect(mx - 11, my - 11, 22, 22);
          for (const b of blocks) {
            if (!b.rest) {
              b.vy += 1500 * dt; b.y += b.vy * dt;
              const floor = ground(b.col * G) - G * (stacked(b.col) + 1);
              if (b.y >= floor) {
                b.y = floor;
                if (b.vy > 240) b.vy *= -.28; else { b.rest = true; b.age = 0; }
              }
            } else b.age += dt;
          }
          for (let i = blocks.length - 1; i >= 0; i--) {
            const b = blocks[i];
            if (b.rest && b.age > 14) { blocks.splice(i, 1); continue; }
            c.globalAlpha = b.rest && b.age > 13 ? 14 - b.age : 1;
            cube(c, b.col * G, b.y, G, b.colour, 'rgba(255,255,255,.3)', 'rgba(0,0,0,.18)');
            c.globalAlpha = 1;
          }
          for (const f of folks) {
            const floor = ground(f.x) - G * stacked(Math.floor(f.x / G));
            if (f.y === 0) f.y = floor;
            if ((f.wait -= dt) <= 0) {
              f.x += f.dir * f.sp * dt;
              if (f.x < 330 || f.x > W - 60 || r() < dt * .08) f.dir *= -1;
              if (r() < dt * .12) f.wait = .8 + r() * 2.2;
            }
            const next = ground(f.x + f.dir * 12) - G * stacked(Math.floor((f.x + f.dir * 12) / G));
            if (f.y >= floor - 1 && next < floor - 1) f.vy = -Math.sqrt(2 * 1500 * (floor - next + 16));
            f.vy += 1500 * dt; f.y += f.vy * dt;
            if (f.y >= floor) { f.y = floor; f.vy = 0; }
            const step = f.wait > 0 || f.vy ? 0 : Math.sin(t * 11 + f.x) * 4;
            c.fillStyle = f.legs;
            c.fillRect(f.x - 10 + step, f.y - 15, 9, 15);
            c.fillRect(f.x + 1 - step, f.y - 15, 9, 15);
            c.fillStyle = f.shirt;
            c.fillRect(f.x - 12, f.y - 38, 24, 24);
            c.fillStyle = '#f7c948';
            c.fillRect(f.x - 19, f.y - 36, 7, 18);
            c.fillRect(f.x + 12, f.y - 36, 7, 18);
            c.fillRect(f.x - 12, f.y - 60, 24, 22);
            c.fillStyle = '#232323';
            c.fillRect(f.x + f.dir * 3 - 5, f.y - 54, 3, 4);
            c.fillRect(f.x + f.dir * 3 + 2, f.y - 54, 3, 4);
            c.fillRect(f.x + f.dir * 3 - 4, f.y - 46, 8, 2);
          }
        },
        click(x, y) {
          if (blocks.length > 24) blocks.shift();
          blocks.push({ col: Math.floor(x / G), y: Math.min(y, 300) - G, vy: 0, colour: pick(r, COLOURS), rest: false, age: 0 });
        },
      };
    },

    // Peach blossom in the Tết wind: petals tumble in gusts, the pointer stirs them, a click shakes more loose.
    peachfall() {
      const r = rng(53), COLOURS = ['#f7b6c8', '#f49ab4', '#fbd3de', '#ee84a4', '#fde4ea'];
      const petal = () => ({ x: r() * (W + 300) - 300, y: r() * H - H * .1, z: .55 + r() * .9, rot: r() * TAU, spin: (r() - .5) * 2.6, ph: r() * TAU, flip: 1.2 + r() * 2.2, colour: pick(r, COLOURS), push: 0, pushY: 0 });
      const list = Array.from({ length: 80 }, petal);
      return {
        frame(c, t, dt, p) {
          const wind = 46 + 40 * Math.sin(t * .21) + 34 * Math.max(0, Math.sin(t * .67 + 1)) ** 4 * 3;
          for (const q of list) {
            if (p.on && near(q, p, 130)) { q.push += (q.x - p.x) * dt * 6; q.pushY += (q.y - p.y) * dt * 6; }
            q.push *= 1 - 1.6 * dt; q.pushY *= 1 - 1.6 * dt;
            q.x += (wind * q.z + q.push + Math.sin(t + q.ph) * 16) * dt;
            q.y += (34 * q.z + 16 + q.pushY) * dt;
            q.rot += q.spin * dt;
            if (q.y > H + 20 || q.x > W + 30) { q.x = r() * (W + 200) - 400; q.y = -20 - r() * 120; if (list.length > 80 && q.extra) list.splice(list.indexOf(q), 1); }
            const s = q.z * 9, flat = Math.cos(t * q.flip + q.ph), cos = Math.cos(q.rot), sin = Math.sin(q.rot);
            c.setTransform(c.base[0] * cos * flat, c.base[0] * sin * flat, -c.base[0] * sin, c.base[0] * cos, c.base[0] * q.x + c.base[1], c.base[0] * q.y + c.base[2]);
            c.fillStyle = q.colour;
            c.beginPath();
            c.moveTo(0, -s);
            c.bezierCurveTo(s * 1.1, -s * .6, s * .9, s * .7, 0, s);
            c.bezierCurveTo(-s * .9, s * .7, -s * 1.1, -s * .6, 0, -s);
            c.fill();
          }
          c.setTransform(c.base[0], 0, 0, c.base[0], c.base[1], c.base[2]);
        },
        click(x, y) {
          if (list.length > 130) return;
          for (let i = 0; i < 16; i++) {
            const q = petal();
            Object.assign(q, { x: x + (r() - .5) * 60, y: y + (r() - .5) * 40, push: (r() - .5) * 360, pushY: (r() - .7) * 240, extra: true });
            list.push(q);
          }
        },
      };
    },

    // Starry night: the sky's strokes flow round its swirls, the stars breathe, and the pointer stirs a swirl of its own.
    starrynight() {
      const r = rng(61), COLOURS = ['#17357f', '#1f4aa0', '#2a5bb8', '#3a6cc4', '#6b9be0', '#bcd8f2', '#f2e8a0', '#f7d648'];
      const SWIRLS = [[620, 250, 1.5], [930, 330, -1.1], [300, 170, -.8], [1180, 170, .8]];
      const STARS = [[180, 90, 20], [430, 80, 16], [790, 70, 18], [1020, 110, 15], [1270, 330, 18], [1480, 400, 16], [110, 340, 16], [520, 450, 14], [1110, 460, 15]];
      // Yellow and white only round the stars and the moon; blue everywhere else.
      const tone = (x, y) => (STARS.some(([sx, sy, s]) => Math.hypot(x - sx, y - sy) < s * 4.5) || Math.hypot(x - 1400, y - 120) < 130 ? 5 + Math.floor(r() * 3) : r() < .07 ? 5 : Math.floor(r() * 5));
      const dots = Array.from({ length: 900 }, () => { const x = r() * W, y = r() * 660; return { x, y, c: tone(x, y), life: r() * 5 }; });
      const VILLAGE = new Path2D('M0 1000 L0 700 Q200 640 420 690 Q640 730 860 680 Q1120 630 1600 700 L1600 1000 Z');
      const CYPRESS = new Path2D('M430 1000 Q360 800 420 620 Q436 500 468 300 Q470 240 480 210 Q492 250 500 330 Q548 520 556 660 Q588 820 540 1000 Z');
      const flow = (x, y, px, py, on) => {
        let u = 1, v = .22 * Math.sin(x * .006 + y * .012);
        const swirl = (cx, cy, s, rad) => {
          const dx = x - cx, dy = y - cy, d = dx * dx + dy * dy + 900, k = s * rad * rad / (d + rad * rad);
          u += -dy / Math.sqrt(d) * k * 2.4; v += dx / Math.sqrt(d) * k * 2.4;
        };
        for (const [cx, cy, s] of SWIRLS) swirl(cx, cy, s, 170);
        if (on) swirl(px, py, 1.8, 130);
        const l = Math.hypot(u, v) || 1;
        return [u / l, v / l];
      };
      const paths = COLOURS.map(() => null);
      return {
        frame(c, t, dt, p) {
          c.lineCap = 'round';
          c.lineWidth = 7;
          for (let i = 0; i < paths.length; i++) paths[i] = new Path2D();
          for (const d of dots) {
            const [u, v] = flow(d.x, d.y, p.x, p.y, p.on);
            d.x += u * 42 * dt; d.y += v * 42 * dt;
            if ((d.life -= dt) < 0 || d.x > W + 20 || d.x < -20 || d.y < -20 || d.y > 700) { d.x = r() * W; d.y = r() * 660; d.c = tone(d.x, d.y); d.life = 3 + r() * 5; }
            const path = paths[d.c];
            path.moveTo(d.x - u * 26, d.y - v * 26);
            path.lineTo(d.x, d.y);
          }
          c.globalAlpha = .82;
          paths.forEach((path, i) => { c.strokeStyle = COLOURS[i]; c.stroke(path); });
          c.globalAlpha = 1;
          for (const [i, [x, y, s]] of STARS.entries()) {
            const k = 1 + .14 * Math.sin(t * 1.3 + i * 1.7);
            for (const [f, colour] of [[3.1, '#6f93c4'], [2.3, '#d8c85e'], [1.5, '#f4e27a']]) {
              c.strokeStyle = colour;
              c.lineWidth = 5;
              c.setLineDash([11, 8]);
              c.lineDashOffset = -t * 12 * f;
              c.beginPath();
              c.arc(x, y, s * f * k, 0, TAU);
              c.stroke();
            }
            c.setLineDash([]);
            c.fillStyle = '#fff6c2';
            c.beginPath();
            c.arc(x, y, s * .7, 0, TAU);
            c.fill();
          }
          c.fillStyle = '#f7d648';
          c.beginPath();
          c.arc(1400, 120, 62, 0, TAU);
          c.fill();
          c.fillStyle = '#17357f';
          c.beginPath();
          c.arc(1376, 104, 54, 0, TAU);
          c.fill();
          c.fillStyle = '#16264a';
          c.fill(VILLAGE);
          c.fillStyle = '#10201c';
          c.fill(CYPRESS);
        },
        click(x, y) {
          SWIRLS.push([x, y, r() < .5 ? 1.2 : -1.2]);
          if (SWIRLS.length > 7) SWIRLS.splice(4, 1);
        },
      };
    },

    // The great wave, cut in wood: spray flies off the crest, the swell rolls, boats ride it and clouds pass.
    greatwave() {
      const r = rng(71), spray = [];
      const CREST = [[640, 112], [720, 118], [800, 150], [860, 190], [905, 244], [560, 150], [470, 210]];
      const boats = [[1050, 840, 1], [1320, 900, .85], [760, 925, .9]];
      const clouds = [[200, 90, 300], [900, 60, 240], [1300, 180, 330]];
      const band = (c, y, amp, len, sp, t, colour, crest) => {
        c.fillStyle = colour;
        c.beginPath();
        c.moveTo(-10, H + 10);
        for (let x = -10; x <= W + 20; x += 30) c.lineTo(x, y + Math.sin(x / len + t * sp) * amp + Math.sin(x / (len * .43) - t * sp * 1.4) * amp * .35);
        c.lineTo(W + 20, H + 10);
        c.fill();
        if (crest) {
          c.strokeStyle = '#f4efe0';
          c.lineWidth = 3.5;
          c.beginPath();
          for (let x = -10; x <= W + 20; x += 30) {
            const yy = y + Math.sin(x / len + t * sp) * amp + Math.sin(x / (len * .43) - t * sp * 1.4) * amp * .35;
            if (x <= -10) c.moveTo(x, yy); else c.lineTo(x, yy);
          }
          c.stroke();
        }
      };
      return {
        frame(c, t, dt) {
          c.fillStyle = '#f3e9d0';
          for (const q of clouds) {
            const x = (q[0] + t * 11) % (W + 500) - 300;
            c.fillRect(x, q[1], q[2], 13);
            c.fillRect(x + 60, q[1] - 13, q[2] * .6, 13);
            c.fillRect(x + 110, q[1] + 13, q[2] * .55, 9);
          }
          if (spray.length < 130) for (const [x, y] of CREST) if (r() < dt * 5) spray.push({ x: x + (r() - .5) * 40, y: y + (r() - .5) * 20, vx: 40 + r() * 120, vy: -60 - r() * 120, s: 2 + r() * 5 });
          c.fillStyle = '#fbf8ee';
          c.beginPath();
          for (let i = spray.length - 1; i >= 0; i--) {
            const q = spray[i];
            q.vy += 200 * dt; q.x += q.vx * dt; q.y += q.vy * dt;
            if (q.y > 760) { spray.splice(i, 1); continue; }
            c.moveTo(q.x + q.s, q.y);
            c.arc(q.x, q.y, q.s, 0, TAU);
          }
          c.fill();
          band(c, 800, 16, 190, .55, t, '#2a6a9c', true);
          for (const [i, [x, y, s]] of boats.entries()) {
            const bob = Math.sin(t * .9 + i * 2) * 13, tilt = Math.sin(t * .7 + i) * .07;
            c.save();
            c.translate(x, y - 50 + bob);
            c.rotate(tilt);
            c.scale(s, s);
            c.fillStyle = '#e8dcb8';
            c.beginPath();
            c.moveTo(-120, -6); c.quadraticCurveTo(0, 22, 130, -14); c.quadraticCurveTo(0, 42, -120, -6);
            c.fill();
            c.fillStyle = '#1b3350';
            for (let k = 0; k < 7; k++) c.fillRect(-84 + k * 26, -8 + Math.abs(k - 3) * -1, 9, 10);
            c.restore();
          }
          band(c, 860, 20, 150, .75, t + 2, '#174a7e', true);
          band(c, 930, 13, 120, .95, t + 5, '#0f2f57', false);
        },
        click(x, y) {
          for (let i = 0; i < 26; i++) { const a = r() * TAU, v = 80 + r() * 230; spray.push({ x, y, vx: Math.cos(a) * v, vy: Math.sin(a) * v - 80, s: 2 + r() * 5 }); }
        },
      };
    },

    // Water lilies in dabs of paint: the pond shimmers and sways, and a click sends rings across it.
    waterlilies() {
      const r = rng(83), rings = rippler('rgba(245, 245, 255, .9)');
      const COLOURS = ['#6e97b8', '#8cb2c8', '#a79ac8', '#5e8f90', '#b9d0d6', '#7fa88e', '#c8b4d8', '#4c7d8e'];
      const dabs = COLOURS.map(() => Array.from({ length: 80 }, () => ({ x: r() * W, y: r() * H, w: 22 + r() * 36, h: 4 + r() * 5, ph: r() * TAU })));
      const LILIES = [[420, 300, 34, '#f6c2d2'], [1020, 240, 30, '#fbf3ea'], [1300, 600, 38, '#f4a8bf'], [640, 690, 36, '#fbf3ea'], [240, 640, 28, '#f4a8bf'], [860, 500, 26, '#f9d5df'], [1500, 330, 28, '#fbf3ea']];
      return {
        frame(c, t, dt) {
          dabs.forEach((group, g) => {
            c.fillStyle = COLOURS[g];
            c.globalAlpha = .42 + .3 * Math.sin(t * .9 + g * .8);
            c.beginPath();
            for (const d of group) {
              const x = d.x + Math.sin(t * .6 + d.ph) * 14, y = d.y + Math.sin(t * .4 + d.ph * 2) * 3;
              c.moveTo(x + d.w, y);
              c.ellipse(x, y, d.w, d.h, 0, 0, TAU);
            }
            c.fill();
          });
          c.globalAlpha = 1;
          rings.draw(c, dt);
          for (const [i, [x, y, s, colour]] of LILIES.entries()) {
            const b = y + Math.sin(t * .7 + i) * 3;
            c.fillStyle = colour;
            c.beginPath();
            for (let k = 0; k < 9; k++) {
              const a = k * TAU / 9 + Math.sin(t * .3 + i) * .05;
              c.moveTo(x + Math.cos(a) * s, b + Math.sin(a) * s * .55);
              c.ellipse(x + Math.cos(a) * s * .55, b + Math.sin(a) * s * .32, s * .55, s * .2, a, 0, TAU);
            }
            c.fill();
            c.fillStyle = '#f2cf63';
            c.beginPath();
            c.ellipse(x, b - s * .08, s * .24, s * .15, 0, 0, TAU);
            c.fill();
          }
        },
        click(x, y) { rings.add(x, y, 220); },
      };
    },

    // Cubism, an original picture: its planes catch and lose the light, and turn bright under the pointer.
    cubism() {
      const r = rng(97), NX = 15, NY = 10, pts = [];
      for (let j = 0; j <= NY; j++) for (let i = 0; i <= NX; i++) pts.push([i * W / NX + (r() - .5) * 66, j * H / NY + (r() - .5) * 66]);
      const tris = [];
      for (let j = 0; j < NY; j++) for (let i = 0; i < NX; i++) {
        const a = j * (NX + 1) + i, b = a + 1, d = a + NX + 1, e = d + 1;
        for (const t of (i + j) % 2 ? [[a, b, e], [a, e, d]] : [[a, b, d], [b, e, d]])
          tris.push({ p: t.map(k => pts[k]), x: (pts[t[0]][0] + pts[t[1]][0] + pts[t[2]][0]) / 3, y: (pts[t[0]][1] + pts[t[1]][1] + pts[t[2]][1]) / 3, ph: r() * TAU, sp: .25 + r() * .5, lit: 0 });
      }
      return {
        frame(c, t, dt, p) {
          for (const q of tris) {
            q.lit += ((p.on && near(q, p, 190) ? 1 : 0) - q.lit) * Math.min(1, dt * 5);
            const k = Math.sin(t * q.sp + q.ph) * .17 + q.lit * .3;
            c.fillStyle = k > 0 ? `rgba(255, 236, 190, ${k.toFixed(3)})` : `rgba(14, 10, 8, ${(-k).toFixed(3)})`;
            c.beginPath();
            c.moveTo(q.p[0][0], q.p[0][1]);
            c.lineTo(q.p[1][0], q.p[1][1]);
            c.lineTo(q.p[2][0], q.p[2][1]);
            c.fill();
          }
        },
        click(x, y) { for (const q of tris) if (Math.hypot(q.x - x, q.y - y) < 340) q.lit = 1.6 - Math.hypot(q.x - x, q.y - y) / 340; },
      };
    },

    // Colour blocks and black lines: small squares run along the lines, and the blocks change colour by themselves or at a click.
    blocksart() {
      const r = rng(103), XS = [120, 340, 620, 980, 1210, 1430], YS = [110, 330, 560, 760, 900], COLOURS = ['#d42a20', '#1f4fa8', '#f2c81e', '#f6f1e3'];
      const runners = Array.from({ length: 34 }, () => {
        const flat = r() < .5;
        return { flat, i: Math.floor(r() * (flat ? YS : XS).length), at: r() * (flat ? W : H), dir: r() < .5 ? 1 : -1, colour: pick(r, COLOURS), sp: 70 + r() * 110, next: 0 };
      });
      const cells = [];
      return {
        frame(c, t, dt) {
          if (r() < dt * .35) cells.push({ i: Math.floor(r() * (XS.length - 1)), j: Math.floor(r() * (YS.length - 1)), colour: pick(r, COLOURS), age: 0 });
          for (let n = cells.length - 1; n >= 0; n--) {
            const q = cells[n];
            if ((q.age += dt) > 7) { cells.splice(n, 1); continue; }
            c.globalAlpha = Math.min(1, q.age * 1.2, (7 - q.age) * 1.2);
            c.fillStyle = q.colour;
            c.fillRect(XS[q.i] + 5, YS[q.j] + 5, XS[q.i + 1] - XS[q.i] - 10, YS[q.j + 1] - YS[q.j] - 10);
          }
          c.globalAlpha = 1;
          for (const q of runners) {
            q.at += q.dir * q.sp * dt;
            const cross = q.flat ? XS : YS, end = q.flat ? W : H;
            if (q.at < 0) q.at = end; else if (q.at > end) q.at = 0;
            q.next -= dt;
            if (q.next <= 0 && r() < .5) for (const [k, v] of cross.entries()) if (Math.abs(v - q.at) < 6) {
              const line = q.flat ? YS[q.i] : XS[q.i];
              q.flat = !q.flat; q.i = k; q.at = line; q.next = 1.2;
              q.dir = r() < .5 ? 1 : -1;
              break;
            }
            const line = (q.flat ? YS : XS)[q.i];
            c.fillStyle = q.colour === '#f6f1e3' ? '#ffffff' : q.colour;
            c.fillRect((q.flat ? q.at : line) - 8, (q.flat ? line : q.at) - 8, 16, 16);
          }
        },
        click(x, y) {
          const i = XS.findIndex((v, k) => k < XS.length - 1 && x >= v && x < XS[k + 1]), j = YS.findIndex((v, k) => k < YS.length - 1 && y >= v && y < YS[k + 1]);
          if (i >= 0 && j >= 0) cells.push({ i, j, colour: pick(r, COLOURS), age: 0 });
        },
      };
    },

    // Gold leaf: a sheen sweeps the tiles, they catch the pointer's light, and the spirals turn.
    goldleaf() {
      const r = rng(109), tiles = [], S = 34;
      for (let y = 0; y < H; y += S) for (let x = 0; x < W; x += S) if (r() < .42) tiles.push({ x: x + 2, y: y + 2, s: S - 4 - Math.floor(r() * 3) * 4, ph: r() * TAU, sp: .6 + r() * 1.2 });
      const SPIRALS = [[420, 240, 96], [790, 640, 130], [1180, 280, 110], [1420, 720, 84], [180, 760, 76], [980, 860, 66], [620, 90, 54]];
      const bins = [new Path2D(), new Path2D(), new Path2D()];
      let flash = null;
      return {
        frame(c, t, dt, p) {
          const sweep = (t * 170) % (W + H + 900) - 450;
          for (let i = 0; i < 3; i++) bins[i] = new Path2D();
          for (const q of tiles) {
            let k = (Math.sin(t * q.sp + q.ph) + 1) * .22;
            const band = Math.abs(q.x + q.y - sweep);
            if (band < 240) k += (1 - band / 240) * .8;
            if (p.on && near(q, p, 170)) k += .7;
            if (flash) { const d = Math.abs(Math.hypot(q.x - flash.x, q.y - flash.y) - flash.r); if (d < 70) k += 1 - d / 70; }
            if (k > .12) bins[k > .9 ? 2 : k > .45 ? 1 : 0].rect(q.x, q.y, q.s, q.s);
          }
          if (flash && (flash.r += 620 * dt) > 900) flash = null;
          [['#a97c1f', .4], ['#e0b13a', .62], ['#ffe68a', .9]].forEach(([colour, a], i) => { c.globalAlpha = a; c.fillStyle = colour; c.fill(bins[i]); });
          c.globalAlpha = 1;
          c.lineCap = 'round';
          for (const [i, [x, y, s]] of SPIRALS.entries()) {
            const turn = t * (i % 2 ? -.28 : .28);
            c.fillStyle = '#1d150b';
            c.beginPath();
            c.arc(x, y, s, 0, TAU);
            c.fill();
            c.strokeStyle = '#f0c552';
            c.lineWidth = 5.5;
            c.beginPath();
            for (let a = 0; a < 20; a += .25) {
              const rad = s * .95 * a / 20, sx = x + Math.cos(a + turn) * rad, sy = y + Math.sin(a + turn) * rad;
              if (a === 0) c.moveTo(sx, sy); else c.lineTo(sx, sy);
            }
            c.stroke();
          }
        },
        click(x, y) { flash = { x, y, r: 0 }; },
      };
    },
  };

  const pointer = { x: 0, y: 0, on: false };
  let canvas = null, ctx = null, photo = null, scene = null, raf = 0, last = 0, time = 0, focused = true, wokenUntil = 0;
  let fit = { s: 1, x: 0, y: 0 };

  function size() {
    if (!canvas) return;
    const w = innerWidth, h = innerHeight, res = Math.min(devicePixelRatio || 1, 1.25), s = Math.max(w / W, h / H);
    canvas.width = Math.round(w * res);
    canvas.height = Math.round(h * res);
    fit = { s, x: (w - W * s) / 2, y: (h - H * s) / 2 };
    ctx.base = [s * res, fit.x * res, fit.y * res];
    photo?.size(w, h, res, s);
    wake();
  }

  /** The lock screen paints over the page; behind it there's nothing to draw for. */
  const covered = () => !!document.querySelector('.lock, .boot');
  const mayRun = () => !!scene && !document.hidden && (focused || performance.now() < wokenUntil);

  function draw(dt) {
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.setTransform(ctx.base[0], 0, 0, ctx.base[0], ctx.base[1], ctx.base[2]);
    time += dt;
    photo?.draw(time, pointer, dt);
    scene.frame(ctx, time, dt, pointer);
  }

  function tick(now) {
    raf = 0;
    if (!mayRun()) return;
    raf = requestAnimationFrame(tick);
    if (now - last < 32) return;
    const dt = Math.min(.1, (now - last) / 1000);
    last = now;
    if (!covered()) draw(dt);
  }

  function wake() {
    if (!raf && mayRun()) { last = performance.now(); raf = requestAnimationFrame(tick); }
  }

  function setTheme() {
    const id = document.documentElement.dataset.theme, make = Object.hasOwn(SCENES, id) ? SCENES[id] : null;
    if (make === scene?.make) return;
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    photo?.remove();
    photo = null;
    if (!make) {
      scene = null;
      canvas?.remove();
      canvas = ctx = null;
      return;
    }
    if (!canvas) {
      canvas = document.createElement('canvas');
      canvas.id = 'live-scene';
      canvas.setAttribute('aria-hidden', 'true');
      document.body.prepend(canvas);
      ctx = canvas.getContext('2d');
    }
    scene = { ...make(), make };
    time = 0;
    // The picture's address as the page resolved it (the theme's --backdrop, whichever folder the page is in).
    if (scene.photo) {
      // On the app's page the picture is on body::before (its own layer, app.css), elsewhere on the body itself.
      const url = /url\("([^"]*photo-[^"]*)"\)/.exec(getComputedStyle(document.body, '::before').backgroundImage)?.[1]
        || /url\("([^"]*photo-[^"]*)"\)/.exec(getComputedStyle(document.body).backgroundImage)?.[1];
      if (url) photo = photoLayer(url, scene.photo, canvas);
    }
    size();
  }

  function toScene(e) {
    pointer.x = (e.clientX - fit.x) / fit.s;
    pointer.y = (e.clientY - fit.y) / fit.s;
  }
  addEventListener('pointermove', e => {
    if (!scene) return;
    toScene(e);
    pointer.on = true;
    // Moved over while another window is in use: it comes alive for a moment.
    if (!focused) wokenUntil = performance.now() + 2500;
    wake();
  }, { passive: true });
  document.documentElement.addEventListener('pointerleave', () => { pointer.on = false; });
  addEventListener('pointerdown', e => {
    if (!scene) return;
    toScene(e);
    scene.click?.(pointer.x, pointer.y);
  }, { passive: true });
  addEventListener('blur', () => { focused = false; pointer.on = false; });
  addEventListener('focus', () => { focused = true; wake(); });
  addEventListener('resize', size);
  document.addEventListener('visibilitychange', wake);
  new MutationObserver(setTheme).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  setTheme();
  window.liveScene = { running: () => !!raf };
})();
