// What a look at the sound finds: the bands, the bass's curve, a loud part, and the beat. Plain sums on samples, nothing
// of Windows, so a song's file can be played through it as the speakers' sound would be (the beat was measured so, on
// real songs, against a tracker that hears the whole song at once).
//
// The beat is found as beat trackers find it (BTrack, aubio, after Davies and Plumbley's and Ellis's work), not by the
// bass's loudness alone, which a song without a strong kick never moves and a busy mix moves all the time:
// - each look, how much the sound rose in each frequency (the spectral flux, on a log scale and against the loudest
//   of each frequency's neighbours a look before, as SuperFlux does, so a held, wavering note doesn't count);
// - every third of a second, the time between beats: the flux of the last 5.5 s laid on itself shifted (the
//   autocorrelation) and summed at 1 to 4 times each gap (a comb), the gaps near half a second liked best, each
//   step's guess near the last;
// - a running score that is high where a rise comes a beat after another high score, and from it, half a beat after
//   each beat, where the next one will be. The beat is given then, so it lands on the music, not after it.
// In silence, or where the sound has no steady pulse, no beat is given.

/// The bands, low to high (40 Hz to 14 kHz, evenly on a log scale).
pub const BANDS: usize = 32;

/// The points of the bass's curve, 25 to 160 Hz (what the ring round the picture draws, as Trap Nation's videos do).
pub const LOW: usize = 40;

/// What the song does now, each 0 to 1.
#[derive(Clone, Copy)]
pub struct Beat {
    pub bands: [f32; BANDS],
    /// The bass's curve, low to high: it fills its range quickly, so it leaps.
    pub low: [f32; LOW],
    /// How hard the bass hits now (eased so a soft one still shows): what the picture swells and the sparks fly by.
    pub punch: f32,
    pub bass: f32,
    pub mid: f32,
    pub high: f32,
    pub level: f32,
    /// 1 at a beat, falling to 0 in about a third of a second.
    pub kick: f32,
    /// The beats counted so far: a new one starts a burst (and even and odd ones take turns).
    pub beats: u32,
    /// Seconds since the last beat, and the time between beats (half a second until it is known).
    pub since: f32,
    pub gap: f32,
    /// How far the song is in a loud part (a chorus, a drop: what osu! calls kiai), 0 to 1: louder than its last
    /// quarter of a minute, eased in and out.
    pub drop: f32,
}

pub const QUIET: Beat = Beat { bands: [0.; BANDS], low: [0.; LOW], punch: 0., bass: 0., mid: 0., high: 0., level: 0., kick: 0., beats: 0, since: 9., gap: 0.5, drop: 0. };

impl Default for Beat {
    fn default() -> Self {
        QUIET
    }
}

/// How hard the bass's curve hits: its mean eased up (a soft bass still shows), 0 to 1.
pub fn punch(low: &[f32; LOW]) -> f32 {
    let x = low.iter().sum::<f32>() / LOW as f32;
    (5. * (1.2 * x - x.powf(1.2))).clamp(0., 1.).powf(0.8)
}

pub const N: usize = 1024;
/// The samples kept, and the bass's FFT.
pub const RING: usize = 4096;
/// The samples between two looks.
pub const HOP: usize = 512;

/// The bands from a frame of sound: see `look`.
pub struct Ear {
    rate: f32,
    win: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    edges: [(usize, usize); BANDS],
    peak: f32,
    smooth: [f32; BANDS],
    big_win: Vec<f32>,
    big_re: Vec<f32>,
    big_im: Vec<f32>,
    low_peak: f32,
    low_smooth: [f32; LOW],
    loud_now: f32,
    loud_long: f32,
    drop: f32,
    pulse: Pulse,
}

impl Ear {
    pub fn new(rate: f32) -> Ear {
        let win = (0..N).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (N - 1) as f32).cos()).collect();
        let bin = rate / N as f32;
        let mut edges = [(1, 2); BANDS];
        let (lo, hi) = (40f32, 14_000f32.min(rate / 2. - bin));
        for (b, e) in edges.iter_mut().enumerate() {
            let f0 = lo * (hi / lo).powf(b as f32 / BANDS as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / BANDS as f32);
            let a = ((f0 / bin).floor() as usize).max(1);
            *e = (a, ((f1 / bin).ceil() as usize).max(a + 1).min(N / 2));
        }
        // (The bass's window leans to the newest sound, at its fullest 0.85 of the way in: an even one would weigh a
        // kick only once it reached the middle, 40 ms on, and the ring would swell after the bars.)
        let lean = 0.5f32.ln() / 0.85f32.ln();
        let big_win = (0..RING).map(|i| (std::f32::consts::PI * (i as f32 / (RING - 1) as f32).powf(lean)).sin().powi(2)).collect();
        Ear {
            rate,
            win,
            re: vec![0.; N],
            im: vec![0.; N],
            edges,
            peak: 1e-4,
            smooth: [0.; BANDS],
            big_win,
            big_re: vec![0.; RING],
            big_im: vec![0.; RING],
            low_peak: 1e-4,
            low_smooth: [0.; LOW],
            loud_now: -99.,
            loud_long: -99.,
            drop: 0.,
            pulse: Pulse::new(rate, bin),
        }
    }

    /// One look at the samples (`ring`, RING of them, the oldest at `at`), made each HOP new samples: the bands from
    /// the last N, the bass's curve from all, and whether a beat falls now. `gap` is the time between beats when the
    /// song has a steady one, else 0.
    pub fn look(&mut self, ring: &[f32], at: usize) -> (Beat, bool) {
        for i in 0..N {
            self.re[i] = ring[(at + RING - N + i) % RING] * self.win[i];
            self.im[i] = 0.;
        }
        fft(&mut self.re, &mut self.im);
        let mag = |i: usize, re: &[f32], im: &[f32]| (re[i] * re[i] + im[i] * im[i]).sqrt();
        let mut raw = [0f32; BANDS];
        for (b, &(a, z)) in self.edges.iter().enumerate() {
            raw[b] = (a..z).map(|i| mag(i, &self.re, &self.im)).fold(0., f32::max);
        }
        let top = raw.iter().copied().fold(0., f32::max);
        // A loud part: the song's loudness (before it is made to fill the range below) over a second, against the
        // last 15 s; 2.5 dB more is a chorus or a drop. Looks come about 90 times a second.
        let loud = 20. * raw.iter().sum::<f32>().max(1e-9).log10();
        if self.loud_long < -98. {
            (self.loud_now, self.loud_long) = (loud, loud);
        }
        self.loud_now += (loud - self.loud_now) * 0.02;
        self.loud_long += (loud - self.loud_long) * 0.0008;
        let up = if self.loud_now > self.loud_long + 2.5 && top > 1e-3 { 1. } else { 0. };
        self.drop += (up - self.drop) * if up > self.drop { 0.03 } else { 0.008 };
        // (Loud or quiet, the song fills the range: the loudest band of the last few seconds is the top.)
        self.peak = (self.peak * 0.997).max(top).max(1e-4);
        let mut out = Beat::default();
        for b in 0..BANDS {
            let db = 20. * (raw[b] / self.peak).max(1e-6).log10();
            let v = ((db + 48.) / 48.).clamp(0., 1.);
            let s = &mut self.smooth[b];
            *s += (v - *s) * if v > *s { 0.6 } else { 0.14 };
            out.bands[b] = *s;
        }
        let beat = self.pulse.step(&self.re, &self.im);
        out.gap = self.pulse.gap();
        // The bass's curve: 25 to 160 Hz, read between the big FFT's steps, measured against its own loudest of late
        // over a 15 dB window (it fills quickly and leaps, as Trap Nation's does), quick to rise and to fall.
        for i in 0..RING {
            self.big_re[i] = ring[(at + i) % RING] * self.big_win[i];
            self.big_im[i] = 0.;
        }
        fft(&mut self.big_re, &mut self.big_im);
        let step = self.rate / RING as f32;
        let mut raw = [0f32; LOW];
        for (p, r) in raw.iter_mut().enumerate() {
            let at_bin = 25. * (160f32 / 25.).powf(p as f32 / (LOW - 1) as f32) / step;
            let (b0, u) = (at_bin.floor() as usize, at_bin.fract());
            *r = mag(b0, &self.big_re, &self.big_im) * (1. - u) + mag(b0 + 1, &self.big_re, &self.big_im) * u;
        }
        let top = raw.iter().copied().fold(0., f32::max);
        self.low_peak = (self.low_peak * 0.998).max(top).max(1e-4);
        for p in 0..LOW {
            let db = 20. * (raw[p] / self.low_peak).max(1e-6).log10();
            let v = ((db + 15.) / 15.).clamp(0., 1.);
            let s = &mut self.low_smooth[p];
            *s += (v - *s) * if v > *s { 0.75 } else { 0.3 };
        }
        for p in 0..LOW {
            let (a, b) = (p.saturating_sub(1), (p + 1).min(LOW - 1));
            out.low[p] = (self.low_smooth[a] + self.low_smooth[p] + self.low_smooth[b]) / 3.;
        }
        out.punch = punch(&out.low);
        out.drop = self.drop;
        let avg = |r: std::ops::Range<usize>| out.bands[r.clone()].iter().sum::<f32>() / r.len() as f32;
        out.bass = avg(0..6);
        out.mid = avg(6..19);
        out.high = avg(19..BANDS);
        out.level = avg(0..BANDS);
        (out, beat)
    }
}

/// The flux kept for the time between beats: 512 looks, about 5.5 s.
const KEPT: usize = 512;
/// How often the time between beats is found again, in looks (about a third of a second).
const TEMPO_EVERY: u64 = 32;
/// The looks heard before the first guess (about 1.4 s: a beat is given from then).
const WARM: u64 = 128;
/// The times between beats looked at, in beats a minute.
const SLOWEST: f32 = 60.;
const FASTEST: f32 = 190.;
/// The score's weight on what came before (BTrack's 0.9), and how sharply it wants exactly a beat between highs.
const ALPHA: f32 = 0.9;
const TIGHT: f32 = 5.;

/// The beat: see the top of the file. Counted in looks (`n`); the times between beats in looks too.
struct Pulse {
    /// Looks a second.
    fps: f32,
    /// The frequencies read (bins 1 to 16 kHz), and the last look's, on a log scale, each the loudest of it and
    /// its two neighbours.
    bins: usize,
    last: Vec<f32>,
    cur: Vec<f32>,
    /// The flux and the running score of the last KEPT looks (look `n` at `n % KEPT`).
    flux: Vec<f32>,
    score: Vec<f32>,
    n: u64,
    /// The gaps tried (in looks, shortest first), how likely each is now, and the one taken.
    gaps: Vec<usize>,
    belief: Vec<f32>,
    gap: usize,
    /// How plainly the song has a pulse (the comb's best against its mean), and how loud its rises are of late
    /// against the last half minute (silence and a soft passage give no beat).
    sure: f32,
    rise_now: f32,
    rise_long: f32,
    /// The score's weights for the gap taken, from half a gap to two gaps back.
    weights: Vec<f32>,
    /// When the next beat falls, and when it is next looked for.
    next: Option<u64>,
    seek_at: u64,
    /// Scratch.
    line: Vec<f32>,
    acf: Vec<f32>,
    ahead: Vec<f32>,
}

impl Pulse {
    fn new(rate: f32, bin: f32) -> Pulse {
        let fps = rate / HOP as f32;
        let bins = ((16_000f32.min(rate / 2.) / bin) as usize).min(N / 2 - 1);
        let shortest = (fps * 60. / FASTEST).floor() as usize;
        let longest = (fps * 60. / SLOWEST).ceil() as usize;
        let gaps: Vec<usize> = (shortest..=longest).collect();
        let belief = vec![1. / gaps.len() as f32; gaps.len()];
        let gap = (fps * 0.5).round() as usize;
        let mut p = Pulse {
            fps,
            bins,
            last: vec![0.; bins + 2],
            cur: vec![0.; bins + 2],
            flux: vec![0.; KEPT],
            score: vec![0.; KEPT],
            n: 0,
            gaps,
            belief,
            gap,
            sure: 0.,
            rise_now: 0.,
            rise_long: 0.,
            weights: Vec::new(),
            next: None,
            seek_at: 0,
            line: vec![0.; KEPT],
            acf: vec![0.; KEPT],
            ahead: Vec::new(),
        };
        p.weigh();
        p
    }

    /// The time between beats in seconds while the song has a steady pulse, else 0.
    fn gap(&self) -> f32 {
        if self.steady() { self.gap as f32 / self.fps } else { 0. }
    }

    fn steady(&self) -> bool {
        self.sure > 1.6 && self.rise_now > 0.25 * self.rise_long && self.rise_now > 1e-3
    }

    fn weigh(&mut self) {
        let g = self.gap as f32;
        self.weights = ((g / 2.).round() as usize..=(2. * g).round() as usize).map(|v| (-0.5 * (TIGHT * (v as f32 / g).ln()).powi(2)).exp()).collect();
    }

    /// The best of the score a beat back from look `n`, weighed by how near a gap it is (`score_at` gives the
    /// score of an earlier look).
    fn back(&self, n: u64, score_at: impl Fn(u64) -> f32) -> f32 {
        let first = (self.gap as f32 / 2.).round() as u64;
        let mut best = 0f32;
        for (i, w) in self.weights.iter().enumerate() {
            let v = first + i as u64;
            if v <= n && v < KEPT as u64 {
                best = best.max(w * score_at(n - v));
            }
        }
        best
    }

    /// A look's spectrum (`re`, `im`, the N-point FFT): whether a beat falls now.
    fn step(&mut self, re: &[f32], im: &[f32]) -> bool {
        // The rise: each frequency on a log scale against the loudest of it and its neighbours a look before.
        for k in 1..=self.bins {
            self.cur[k] = (re[k] * re[k] + im[k] * im[k]).sqrt().ln_1p();
        }
        let mut rise = 0f32;
        for k in 1..=self.bins {
            let before = self.last[k - 1].max(self.last[k]).max(self.last[k + 1]);
            rise += (self.cur[k] - before).max(0.);
        }
        rise /= self.bins as f32;
        for k in 1..=self.bins {
            self.last[k] = self.cur[k];
        }
        let n = self.n;
        self.n += 1;
        self.flux[(n % KEPT as u64) as usize] = rise;
        self.rise_now += (rise - self.rise_now) * (1. / self.fps);
        self.rise_long += (rise - self.rise_long) * (1. / (30. * self.fps));
        let score = (1. - ALPHA) * rise + ALPHA * self.back(n, |m| self.score[(m % KEPT as u64) as usize]);
        self.score[(n % KEPT as u64) as usize] = score;
        if n >= WARM && n % TEMPO_EVERY == 0 {
            self.find_gap();
        }
        if n >= self.seek_at && n >= WARM {
            self.seek(n);
        }
        let beat = self.next == Some(n);
        if beat {
            self.next = None;
            self.seek_at = n + (self.gap as u64 / 2).max(1);
        }
        beat && self.steady()
    }

    /// Finds the time between beats again from the flux kept.
    fn find_gap(&mut self) {
        let len = KEPT.min(self.n as usize);
        // The flux oldest first, each look less the mean of the 15 around it (only the rises above their
        // surroundings count).
        let start = self.n - len as u64;
        for i in 0..len {
            self.line[i] = self.flux[((start + i as u64) % KEPT as u64) as usize];
        }
        let mut lifted = vec![0f32; len];
        for (i, l) in lifted.iter_mut().enumerate() {
            let (a, z) = (i.saturating_sub(8), (i + 8).min(len));
            let mean = self.line[a..z].iter().sum::<f32>() / (z - a) as f32;
            *l = (self.line[i] - mean).max(0.);
        }
        let longest = *self.gaps.last().unwrap_or(&1);
        let lags = (4 * longest + 4).min(len - 1);
        for lag in 0..=lags {
            let mut s = 0f32;
            for i in lag..len {
                s += lifted[i] * lifted[i - lag];
            }
            self.acf[lag] = s / (len - lag) as f32;
        }
        // The comb: each gap's autocorrelation at 1 to 4 times it (spread a little at the longer ones), the gaps near
        // half a second liked best (a Rayleigh curve, as BTrack's).
        let like = self.fps * 0.5;
        let mut seen: Vec<f32> = self
            .gaps
            .iter()
            .map(|&g| {
                let mut c = 0f32;
                for a in 1..=4usize {
                    for b in 0..(2 * a - 1) {
                        let at = a * g + b + 1 - a;
                        if at <= lags {
                            c += self.acf[at] / (2 * a - 1) as f32;
                        }
                    }
                }
                let gf = g as f32;
                c * gf / (like * like) * (-gf * gf / (2. * like * like)).exp()
            })
            .collect();
        let mean = seen.iter().sum::<f32>() / seen.len() as f32;
        let top = seen.iter().copied().fold(0., f32::max);
        self.sure = if mean > 0. { top / mean } else { 0. };
        if top <= 0. {
            return;
        }
        for s in seen.iter_mut() {
            *s /= top;
        }
        // Each step's guess stays near the last one: the chance of moving from gap a to gap b falls off with how far
        // apart they are (a twentieth of the gap is likely, a fifth is not).
        let before = self.belief.clone();
        let mut sum = 0f32;
        for (j, &gb) in self.gaps.iter().enumerate() {
            let mut best = 0f32;
            for (i, &ga) in self.gaps.iter().enumerate() {
                let d = (gb as f32 / ga as f32).ln() / 0.06;
                best = best.max(before[i] * (-0.5 * d * d).exp());
            }
            self.belief[j] = best * (0.05 + seen[j]);
            sum += self.belief[j];
        }
        if sum > 0. {
            for b in self.belief.iter_mut() {
                *b /= sum;
            }
        }
        let (i, _) = self.belief.iter().enumerate().fold((0, 0f32), |m, (i, &b)| if b > m.1 { (i, b) } else { m });
        if self.gaps[i] != self.gap {
            self.gap = self.gaps[i];
            self.weigh();
        }
    }

    /// Where the next beat falls: the score carried on a beat ahead with no new rises, the highest of it near one
    /// gap after the last beat.
    fn seek(&mut self, n: u64) {
        let g = self.gap;
        self.ahead.clear();
        self.ahead.resize(g + 1, 0.);
        for k in 1..=g {
            let at = n + k as u64;
            let v = {
                let ahead = &self.ahead;
                let score = &self.score;
                self.back(at, |m| if m > n { ahead[(m - n) as usize] } else { score[(m % KEPT as u64) as usize] })
            };
            self.ahead[k] = v;
        }
        // (Looked for half a gap after a beat, so the next one is expected half a gap from now.)
        let mid = g as f32 / 2.;
        let mut best = (1usize, f32::MIN);
        for k in 1..=g {
            let d = (k as f32 - mid) / mid;
            let v = self.ahead[k] * (-0.5 * d * d).exp();
            if v > best.1 {
                best = (k, v);
            }
        }
        self.next = Some(n + best.0 as u64);
        self.seek_at = n + best.0 as u64 + (g as u64 / 2).max(1);
    }
}

/// A radix-2 FFT in place (N a power of two).
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        let (ws, wc) = ang.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1f32, 0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                (cr, ci) = (cr * wc - ci * ws, cr * ws + ci * wc);
            }
        }
        len <<= 1;
    }
}
