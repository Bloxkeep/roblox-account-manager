using System.Drawing;
using System.Drawing.Imaging;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Macros (Beta): the checks on a macro, the player on a made-up desktop (its own clock, windows that are only records:
/// nothing is ever sent, focused or launched), the turn every window input waits for, the pictures' maths (finding a
/// picture, colours, follow's decisions and its PD control), TinyTask files, settings ("$o1") and areas ("@a1"), the
/// panel's values and when it shows by itself, the dots and boxes drawn on the game (their maths only: no window is
/// opened), the templates and "What it sees".
/// </summary>
static class MacroSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        InputTurn.Reset();
        Validation(c);
        Geometry(c);
        Vision(c);
        TinyTask(c);
        Values(c);
        FollowMaths(c);
        await Turns(c);
        await Player(c);
        await Features(c);
        await Bounds(c);
        Panels(c);
        Templates(c);
        Watch(c);
        InputTurn.Reset();
    }

    // ───────────────────────── helpers ─────────────────────────

    static string? Refused(Action a)
    {
        try
        {
            a();
            return null;
        }
        catch (UserError e)
        {
            return e.Message;
        }
    }

    static Macro Read(string steps, string extra = "")
    {
        using var doc = JsonDocument.Parse($$"""{"name":"Test","jitterMs":0,"jitterPx":0{{extra}},"steps":{{steps}}}""");
        return MacroRules.Read(doc.RootElement);
    }

    /// <summary>A checked macro with an id, ready to queue.</summary>
    internal static Macro M(string steps, string extra = "")
    {
        var m = Read(steps, extra);
        MacroRules.Check(m, new List<Guid>(), []);
        m.Id = Guid.NewGuid();
        return m;
    }

    static string? CheckOf(string steps, string extra = "") => Refused(() => MacroRules.Check(Read(steps, extra), new List<Guid>(), []));

    static JsonElement J(object v) => JsonSerializer.SerializeToElement(v);

    /// <summary>A black picture with coloured columns (x, width, colour).</summary>
    static Bitmap Bars(int w, int h, params (int X, int W, Color Colour)[] bars)
    {
        var b = new Bitmap(w, h, PixelFormat.Format32bppArgb);
        using var g = Graphics.FromImage(b);
        g.Clear(Color.Black);
        foreach (var (x, bw, colour) in bars)
        {
            using var brush = new SolidBrush(colour);
            g.FillRectangle(brush, x, 0, bw, h);
        }
        return b;
    }

    /// <summary>Text as Windows would read it: one line each, its words side by side.</summary>
    static List<MacroOcr.Line> Said(params string[] lines) => lines.Select((line, k) => new MacroOcr.Line(
        line.Split(' ').Select((word, i) => new MacroOcr.Word(word, new RectangleF(i * 60, k * 20, 50, 16))).ToList())).ToList();

    static Bitmap Patches(int w, int h, params Rectangle[] patches)
    {
        var b = new Bitmap(w, h, PixelFormat.Format32bppArgb);
        using var g = Graphics.FromImage(b);
        g.Clear(Color.Black);
        foreach (var r in patches)
            g.FillRectangle(Brushes.Lime, r);
        return b;
    }

    // ───────────────────────── a made-up desktop ─────────────────────────

    sealed class FakeOutside : IMacroOutside
    {
        public string Clip = "";
        public Dictionary<string, string> Files = new();
        public Dictionary<string, string> Web = new();
        public List<string> Posts = new();
        public Task<string?> ClipboardAsync() => Task.FromResult<string?>(Clip);
        public Task CopyAsync(string text) { Clip = text; return Task.CompletedTask; }
        public Task<string?> ReadFileAsync(string name) => Task.FromResult(Files.TryGetValue(name, out var f) ? f.Split('\n').Last(l => l.Trim().Length > 0) : null);
        public Task AppendFileAsync(string name, string line) { Files[name] = Files.TryGetValue(name, out var f) ? f + "\n" + line : line; return Task.CompletedTask; }
        public Task<string?> WebAsync(string url, string? post, CancellationToken ct)
        {
            if (post is not null)
                Posts.Add(url + " " + post);
            return Task.FromResult(post is null ? Web.GetValueOrDefault(url) : "ok");
        }
    }

    internal sealed class FakeDesk : IMacroDesk
    {
        public DateTime Clock = new(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
        public readonly DateTime Start = new(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
        public readonly List<string> Log = new();
        public readonly Dictionary<Guid, MacroWindow> Windows = new();
        public Func<string?> HoldNow = () => null;
        public Action? OnDelay;
        public Func<Point, bool> ClearAt = _ => true;
        public Func<Bitmap?> Picture = () => null;
        public Func<Rectangle, Bitmap?>? Area;
        /// <summary>When the person last touched the PC (ms on this clock); null: not since the start.</summary>
        public Func<int?> PersonAt = () => null;
        /// <summary>A wait asked while this is true never ends: the app stopped with a run halfway (Bloxkeep quitting).</summary>
        public Func<bool>? SuspendWhen;
        public readonly TaskCompletionSource Never = new();
        /// <summary>Giving the window back fails (a turn's end throwing).</summary>
        public bool ThrowOnEnd;
        public int Ms => (int)(Clock - Start).TotalMilliseconds;

        public DateTime Now => Clock;

        public Task Delay(int ms, CancellationToken ct)
        {
            if (SuspendWhen?.Invoke() == true)
                return Never.Task;
            Clock = Clock.AddMilliseconds(Math.Max(0, ms));
            OnDelay?.Invoke();
            ct.ThrowIfCancellationRequested();
            return Task.CompletedTask;
        }

        public string? Hold() => HoldNow();
        public MacroWindow? Window(Guid accountId) => Windows.GetValueOrDefault(accountId);

        public Task<IMacroTurn?> BeginTurnAsync(MacroWindow window, CancellationToken ct)
        {
            Log.Add($"turn {window.Pid}");
            return Task.FromResult<IMacroTurn?>(new FakeTurn(this, window));
        }

        public Task<Bitmap?> PictureAsync(MacroWindow window, IMacroTurn? turn) => Task.FromResult(Picture());
        public bool ImeOn() => false;
        public Func<List<List<MacroOcr.Line>>?> Lines = () => null;
        public Task<List<List<MacroOcr.Line>>?> ReadAsync(Bitmap picture, Rectangle area) => Task.FromResult(Lines());
        public Task<Bitmap?> AreaAsync(MacroWindow window, IMacroTurn turn, Rectangle area) => Task.FromResult(Area?.Invoke(area));
    }

    sealed class FakeTurn(FakeDesk desk, MacroWindow w) : IMacroTurn
    {
        Point _at;
        readonly int _started = desk.Ms;
        /// <summary>What is down now, as the real turn keeps it to let go of all at once.</summary>
        readonly List<(string Name, bool Key)> _down = new();
        public Rectangle Client => w.Client;
        public bool StillFront() => true;
        public bool PersonInput() => desk.PersonAt() is { } at && at > _started && at <= desk.Ms;
        public void Key(MacroKey key, bool up)
        {
            Note(key.Name, true, up);
            desk.Log.Add($"{w.Pid} key {key.Name} {(up ? "up" : "down")} @{desk.Ms}");
        }
        public void Char(char c, bool up) => desk.Log.Add($"{w.Pid} char {c} {(up ? "up" : "down")}");
        public void MoveTo(Point screen)
        {
            _at = screen;
            desk.Log.Add($"{w.Pid} move {screen.X},{screen.Y}");
        }
        public void MoveBy(int dx, int dy) => desk.Log.Add($"{w.Pid} by {dx},{dy}");
        public void Button(string button, bool up)
        {
            Note(button, false, up);
            desk.Log.Add($"{w.Pid} {button} {(up ? "up" : "down")} @{desk.Ms}");
        }
        public void Wheel(int notches) => desk.Log.Add($"{w.Pid} wheel {notches}");
        public bool Clear(Point screen) => desk.ClearAt(screen);
        public Point CursorAt() => _at;

        void Note(string name, bool key, bool up)
        {
            _down.Remove((name, key));
            if (!up)
                _down.Add((name, key));
        }

        /// <summary>Everything let go and the PC given back at once, with no wait (Bloxkeep closing).</summary>
        public void Abort()
        {
            foreach (var (name, key) in _down.ToList())
                desk.Log.Add(key ? $"{w.Pid} key {name} up @{desk.Ms}" : $"{w.Pid} {name} up @{desk.Ms}");
            _down.Clear();
            desk.Log.Add($"abort {w.Pid}");
        }

        public ValueTask DisposeAsync()
        {
            if (desk.ThrowOnEnd)
                throw new InvalidOperationException("The window couldn't be given back.");
            desk.Log.Add($"end {w.Pid}");
            return ValueTask.CompletedTask;
        }
    }

    internal static (MacroPlayer Player, FakeDesk Desk, Guid[] Accounts) Setup(int accounts = 1)
    {
        var desk = new FakeDesk();
        var ids = Enumerable.Range(0, accounts).Select(_ => Guid.NewGuid()).ToArray();
        for (int i = 0; i < accounts; i++)
            desk.Windows[ids[i]] = new MacroWindow(ids[i], i + 1, IntPtr.Zero, new Rectangle(100, 100, 800, 600), 60, true, false);
        return (new MacroPlayer(desk) { Random = new Random(1) }, desk, ids);
    }

    static async Task Done(MacroPlayer p) => await Task.WhenAny(p.Idle, Task.Delay(10_000));

    // ───────────────────────── checks on a macro ─────────────────────────

    static void Validation(Checks c)
    {
        c.Check("a macro needs a name", Refused(() => MacroRules.Check(new Macro { Steps = [new MacroStep { Kind = "wait", Ms = 1 }] }, new List<Guid>(), [])) is not null);
        c.Check("a macro needs a step", CheckOf("[]") is not null);
        c.Check("an unknown step is refused", CheckOf("""[{"kind":"jump"}]""") is not null);
        c.Check("a loop without its end is refused", CheckOf("""[{"kind":"loop","times":2},{"kind":"key","key":"A"}]""")?.Contains("End repeat") == true);
        c.Check("a key held and never released is refused", CheckOf("""[{"kind":"down","key":"W"}]""") is not null);
        var key = Read("""[{"kind":"key","key":"E"}]""");
        MacroRules.Check(key, new List<Guid>(), []);
        c.Check("a key press is held 60 ms unless told", key.Steps[0].Ms == 60, key.Steps[0].Ms);
        c.Check("a macro starting by itself on 'selected' is refused", CheckOf("""[{"kind":"wait","ms":10}]""", ""","trigger":{"kind":"join"}""") is not null);
        c.Check("a mouse hold of 0..10000 ms is taken", CheckOf("""[{"kind":"click","x":0.5,"y":0.5,"holdMs":800}]""") is null);
        c.Check("a mouse hold over 10 s is refused", CheckOf("""[{"kind":"click","x":0.5,"y":0.5,"holdMs":20000}]""") is not null);
        c.Check("Hold and Release take the mouse buttons", CheckOf("""[{"kind":"down","key":"MouseLeft"},{"kind":"up","key":"MouseLeft"}]""") is null);
        c.Check("follow needs its area", CheckOf("""[{"kind":"follow","target":"#00ff00","marker":"#ffffff"}]""") is not null);
        var follow = Read("""[{"kind":"follow","region":{"x":0,"y":0.5,"w":1,"h":0.1},"target":"#00ff00","marker":{"left":"#ff0000","right":"$edge"}}]""");
        MacroRules.Check(follow, new List<Guid>(), []);
        var f = follow.Steps[0];
        c.Check("follow takes box edges and fills its defaults (onOff, 1.0/0.4, 60 fps)", f.Mode == "onOff" && f.Kp == 1.0 && f.Kd == 0.4 && f.Fps == 60 && f.Button == "left",
            JsonSerializer.Serialize(f));
        c.Check("follow at 300 fps is refused", CheckOf("""[{"kind":"follow","region":{"x":0,"y":0,"w":1,"h":1},"target":"#00ff00","marker":"#ffffff","fps":300}]""") is not null);
        c.Check("follow's marker must be a colour", CheckOf("""[{"kind":"follow","region":{"x":0,"y":0,"w":1,"h":1},"target":"#00ff00","marker":{"left":"red"}}]""") is not null);
        var cc = Read("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1}}]""");
        MacroRules.Check(cc, new List<Guid>(), []);
        c.Check("clickColour ignores specks under 4 px unless told", cc.Steps[0].MinSize == 4 && cc.Steps[0].Button == "left");
        c.Check("waitColour searches an area too", CheckOf("""[{"kind":"waitColour","colour":"#00ff00","region":{"x":0,"y":0,"w":0.5,"h":0.5}}]""") is null);
        c.Check("an If stuck part is taken at the top level", CheckOf("""[{"kind":"key","key":"E"},{"kind":"recover"},{"kind":"key","key":"Escape"}]""", ""","onFail":"restart","stuckMinutes":5""") is null);
        c.Check("an If stuck part inside a loop is refused", CheckOf("""[{"kind":"loop","times":0},{"kind":"recover"},{"kind":"endLoop"}]""") is not null);
        c.Check("a second If stuck part is refused", CheckOf("""[{"kind":"recover"},{"kind":"key","key":"E"},{"kind":"recover"}]""") is not null);
        c.Check("a key still held where the If stuck part begins is refused",
            CheckOf("""[{"kind":"down","key":"W"},{"kind":"recover"},{"kind":"up","key":"W"}]""") is not null);
        c.Check("stuck after over an hour is refused", CheckOf("""[{"kind":"key","key":"E"}]""", ""","stuckMinutes":90""") is not null);
        var jumps = MacroRules.Jumps(Read("""[{"kind":"loop","times":2},{"kind":"if","test":{"kind":"var","name":"n","op":"=","value":1}},{"kind":"break"},{"kind":"else"},{"kind":"key","key":"A"},{"kind":"endIf"},{"kind":"endLoop"}]""").Steps);
        c.Check("jumps: loop↔endLoop, if→else→endIf, break→endLoop", string.Join(",", jumps) == "6,3,6,5,-1,-1,0", string.Join(",", jumps));
        c.Check("scan codes: letters by place, arrows extended, the pad apart",
            MacroKeys.All["A"].Scan == 0x1E && MacroKeys.All["Up"] is { Scan: 0x48, Extended: true } && MacroKeys.FromScan(0x48, true)?.Name == "Up"
            && MacroKeys.FromScan(0x48, false)?.Name == "Numpad8");
        c.Check("a press lasts two frames at least, never under 60 ms", MacroPlayer.HoldFor(10, 30) == 67 && MacroPlayer.HoldFor(10, 240) == 60 && MacroPlayer.HoldFor(500, 60) == 500);
    }

    static void Geometry(Checks c)
    {
        var left = new Rectangle(-1920, 0, 1920, 1080);
        c.Check("a place in a window on a screen left of the main one", MacroGeometry.ClientPoint(left, 0.5, 0.5) == new Point(-960, 540)
            && MacroGeometry.ClientPoint(left, 1, 1) == new Point(-1, 1079));
        var desktop = new Rectangle(-1920, 0, 3840, 1080);
        c.Check("absolute input covers the whole desktop from its left edge",
            MacroGeometry.Absolute(new Point(-1920, 0), desktop) == (0, 0) && MacroGeometry.Absolute(new Point(1919, 1079), desktop) == (65535, 65535),
            MacroGeometry.Absolute(new Point(1919, 1079), desktop));
        var pieces = MacroGeometry.Pieces(100, -7, 6).ToList();
        c.Check("a camera move's pieces add up to it", pieces.Sum(p => p.X) == 100 && pieces.Sum(p => p.Y) == -7);
    }

    // ───────────────────────── pictures ─────────────────────────

    static void Vision(Checks c)
    {
        // Blocks of grey, so a picture scaled up still looks alike.
        var rnd = new Random(7);
        using var picture = new Bitmap(240, 160);
        using (var g = Graphics.FromImage(picture))
        {
            for (int y = 0; y < 160; y += 8)
                for (int x = 0; x < 240; x += 8)
                {
                    int v = rnd.Next(256);
                    using var b = new SolidBrush(Color.FromArgb(v, v, v));
                    g.FillRectangle(b, x, y, 8, 8);
                }
        }
        using var cut = picture.Clone(new Rectangle(120, 40, 32, 32), PixelFormat.Format32bppArgb);
        using var png = new MemoryStream();
        cut.Save(png, ImageFormat.Png);
        string base64 = Convert.ToBase64String(png.ToArray());
        var gray = MacroVision.ToGray(picture);
        var found = MacroVision.Find(gray, MacroVision.Template(base64, 1)!, new Rectangle(0, 0, gray.W, gray.H));
        c.Check("a picture is found where it was cut", found is { Score: > 0.95 } f && Math.Abs(f.Found.X - 120) <= 1 && Math.Abs(f.Found.Y - 40) <= 1, found);
        using var big = new Bitmap(360, 240);
        using (var g = Graphics.FromImage(big))
        {
            g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.NearestNeighbor;
            g.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.Half;
            g.DrawImage(picture, new Rectangle(0, 0, 360, 240));
        }
        var bigGray = MacroVision.ToGray(big);
        var scaled = MacroVision.Find(bigGray, MacroVision.Template(base64, 1.5)!, new Rectangle(0, 0, bigGray.W, bigGray.H));
        c.Check("in a window 1.5× as big, the cut is scaled and still found", scaled is { Score: > 0.85 } s && Math.Abs(s.Found.X - 180) <= 3 && Math.Abs(s.Found.Y - 60) <= 3, scaled);
        var area = new Rectangle(0, 0, gray.W, gray.H);
        var same = MacroVision.Difference(MacroVision.Signature(gray, area), MacroVision.Signature(MacroVision.ToGray(picture), area));
        using var other = Bars(240, 160, (0, 120, Color.White));
        var moved = MacroVision.Difference(MacroVision.Signature(gray, area), MacroVision.Signature(MacroVision.ToGray(other), area));
        c.Check("waitStill: the same picture is still, another isn't", same <= MacroVision.StillDifference && moved > MacroVision.StillDifference, (same, moved));
        using var dot = Bars(10, 10, (5, 1, Color.FromArgb(200, 20, 20)));
        c.Check("a colour at a place, within its tolerance", MacroVision.ColourAt(dot, 0.5, 0.5, "#c81414", 0) && MacroVision.ColourAt(dot, 0.5, 0.5, "#d01010", 10)
            && !MacroVision.ColourAt(dot, 0.1, 0.5, "#c81414", 24));
    }

    static void TinyTask(Checks c)
    {
        static byte[] Rec(params (uint Message, uint L, uint H, uint Time)[] events)
        {
            var bytes = new List<byte>();
            foreach (var (message, l, h, time) in events)
            {
                bytes.AddRange(BitConverter.GetBytes(message));
                bytes.AddRange(BitConverter.GetBytes(l));
                bytes.AddRange(BitConverter.GetBytes(h));
                bytes.AddRange(BitConverter.GetBytes(time));
                bytes.AddRange(BitConverter.GetBytes(0u));
            }
            return bytes.ToArray();
        }
        uint e = 0x45 | (0x12 << 8);
        var file = Rec((0x100, e, 0, 1000), (0x101, e, 0, 1120), (0x201, 960, 540, 1500), (0x202, 960, 540, 1560));
        var m = MacroRecording.FromTinyTask(file, "rec");
        var keyStep = m.Steps.FirstOrDefault(s => s.Kind == "key");
        var click = m.Steps.FirstOrDefault(s => s.Kind == "click");
        c.Check("a TinyTask file: a key held 120 ms, then a click in the middle of a 1920×1080 screen",
            keyStep is { Key: "E", Ms: 120 } && click is { X: 0.5, Y: 0.5 } && m.RecordedSize is { W: 1920, H: 1080 },
            JsonSerializer.Serialize(m.Steps, MacroRules.PageJson));
        c.Check("a file that isn't one is refused", Refused(() => MacroRecording.ReadTinyTask(new byte[19])) is not null
            && Refused(() => MacroRecording.ReadTinyTask(Rec((0x9999, 0, 0, 0)))) is not null);
        var steps = MacroRecording.ToSteps([new MacroEvent(0, "keyDown", "W"), new MacroEvent(50, "keyDown", "Space"), new MacroEvent(80, "keyUp", "Space"),
            new MacroEvent(900, "keyUp", "W")], _ => (0.5, 0.5));
        c.Check("a key held across another is a Hold and a Release around it",
            steps.Any(s => s is { Kind: "down", Key: "W" }) && steps.Any(s => s is { Kind: "up", Key: "W" }) && steps.Any(s => s is { Kind: "key", Key: "Space" }),
            string.Join(" ", steps.Select(s => s.Kind + ":" + s.Key)));
        // A walk held alone as long as it was (2026-11-13: one of 8 s was cut to 5 s and stopped meters short); past a key
        // step's 10 s, Hold, the wait and Release.
        var walk = MacroRecording.ToSteps([new MacroEvent(0, "keyDown", "W"), new MacroEvent(8000, "keyUp", "W")], _ => (0.5, 0.5));
        c.Check("a key held alone 8 s plays 8 s, not 5", walk is [{ Kind: "key", Key: "W", Ms: 8000 }], JsonSerializer.Serialize(walk, MacroRules.PageJson));
        var longWalk = MacroRecording.ToSteps([new MacroEvent(0, "keyDown", "W"), new MacroEvent(25_000, "keyUp", "W"), new MacroEvent(25_500, "keyDown", "E"), new MacroEvent(25_600, "keyUp", "E")], _ => (0.5, 0.5));
        c.Check("…one held 25 s: Hold, a 25 s wait, Release, then what came after",
            longWalk is [{ Kind: "down", Key: "W" }, { Kind: "wait", Ms: 25_000 }, { Kind: "up", Key: "W" }, { Kind: "wait", Ms: 500 }, { Kind: "key", Key: "E", Ms: 100 }],
            JsonSerializer.Serialize(longWalk, MacroRules.PageJson));
        var made = new Macro { Name = "Walk", Steps = longWalk.Concat(walk).ToList() };
        c.Check("…and both pass a macro's checks", Refused(() => MacroRules.Check(made, new List<Guid>(), [])) is null);
    }

    // ───────────────────────── settings and areas in steps ─────────────────────────

    static void Values(Checks c)
    {
        var m = Read("""[{"kind":"wait","ms":"$o1"},{"kind":"waitColour","colour":"$o2","region":"@a1","tolerance":"$o3"},{"kind":"key","key":"$o4"}]""");
        c.Check("\"$o1\" is kept apart from the number it stands for", m.Steps[0].Ms is null && MacroValues.RefOf(m.Steps[0].Extra, "ms") == "o1");
        c.Check("\"@a1\" names an area, in an area's field only", MacroValues.RefOf(m.Steps[1].Extra, "region") == "@a1" && m.Steps[1].Region is null
            && MacroValues.RefOf(new() { ["ms"] = J("@a1") }, "ms") is null);
        c.Check("a macro naming settings and areas passes its checks", Refused(() => MacroRules.Check(m, new List<Guid>(), [])) is null);
        c.Check("its names are listed", string.Join(",", MacroValues.Names(m).Order(StringComparer.Ordinal)) == "@a1,o1,o2,o3,o4", string.Join(",", MacroValues.Names(m)));
        string page = JsonSerializer.Serialize(m, MacroRules.PageJson);
        c.Check("\"$o1\" and \"@a1\" go back to the page as written", page.Contains("\"ms\":\"$o1\"") && page.Contains("\"region\":\"@a1\"") && page.Contains("\"key\":\"$o4\""), page);
        var vault = JsonSerializer.Deserialize<Macro>(JsonSerializer.Serialize(m))!;
        c.Check("and survive the vault", MacroValues.RefOf(vault.Steps[0].Extra, "ms") == "o1" && vault.Steps[0].Ms is null);
        c.Check("a bad name is refused", CheckOf("""[{"kind":"wait","ms":"$9lives"}]""") is not null && CheckOf("""[{"kind":"move","x":0.5,"y":0.5},{"kind":"waitColour","colour":"#000000","region":"@-"}]""") is not null);
        var values = new Dictionary<string, JsonElement>
        {
            ["o1"] = J(250), ["o2"] = J("#00ff00"), ["@a1"] = J(new { x = 0.1, y = 0.2, w = 0.3, h = 0.4 }), ["o3"] = J(500), ["o4"] = J("q"),
        };
        JsonElement? V(string n) => values.TryGetValue(n, out var v) ? v : null;
        var wait = MacroValues.Resolve(m.Steps[0], V);
        var colour = MacroValues.Resolve(m.Steps[1], V);
        var key = MacroValues.Resolve(m.Steps[2], V);
        c.Check("values are put in as it plays (a tolerance kept to 255, a key spelled as the list has it)", wait.Ms == 250 && colour.Colour == "#00ff00"
            && colour.Region is { X: 0.1, W: 0.3 } && colour.Tolerance == 255 && key.Key == "Q", JsonSerializer.Serialize(colour, MacroRules.PageJson));
        c.Check("the step itself is left as it was", m.Steps[0].Ms is null);
        values.Remove("o1");
        c.Check("a missing value stops it (missingValue)", Throws<MacroValueMissing>(() => MacroValues.Resolve(m.Steps[0], V)));
        values["o1"] = J("soon");
        values["o4"] = J("NoSuchKey");
        c.Check("a value of the wrong kind too", Throws<MacroValueMissing>(() => MacroValues.Resolve(m.Steps[0], V)) && Throws<MacroValueMissing>(() => MacroValues.Resolve(m.Steps[2], V)));
        var varTest = new MacroTest { Kind = "var", Name = "o5", Op = "=", Extra = new() { ["value"] = J("Gold") } };
        c.Check("a var check compares a choice's word", MacroValues.VarTest(varTest, n => n == "o5" ? J("gold") : null, false)
            && !MacroValues.VarTest(varTest, n => n == "o5" ? J("Wood") : null, false));
        var count = new MacroTest { Kind = "var", Name = "n", Op = "<", Value = 3 };
        c.Check("a variable the macro sets counts as 0 before it is set", MacroValues.VarTest(count, _ => null, setByMacro: true)
            && Throws<MacroValueMissing>(() => MacroValues.VarTest(count, _ => null, setByMacro: false)));
        // What a run reads: its variables first, then the macro's settings; an area by "@".
        var own = Read("""[{"kind":"wait","ms":1}]""", ""","options":[{"name":"o1","label":"Every","kind":"number","value":5,"default":1}],"areas":[{"name":"a1","label":"Bar","rect":{"x":0.1,"y":0.1,"w":0.2,"h":0.2} }]""");
        c.Check("a run reads a variable, else the macro's setting, else an area", MacroPlayer.Value(own, null, "o1")?.GetDouble() == 5
            && MacroPlayer.Value(own, new() { ["o1"] = 9 }, "o1")?.GetDouble() == 9 && MacroPlayer.Value(own, null, "@a1")?.GetProperty("w").GetDouble() == 0.2
            && MacroPlayer.Value(own, null, "@a2") is null && MacroPlayer.Value(own, null, "o2") is null);

        // Formulas (MacroExpr.cs): what AutoHotkey's expressions do, in a Set variable step.
        double E(string f, Dictionary<string, double>? v = null) => MacroExpr.Eval(f, n => v is not null && v.TryGetValue(n, out double d) ? d : throw new MacroValueMissing(n));
        var vs = new Dictionary<string, double> { ["coins"] = 250, ["n"] = 7 };
        c.Check("a formula: order, brackets, names, {name}, % never below 0", E("2 + 3 * 4") == 14 && E("(2 + 3) * 4") == 20 && E("coins / 2 + {n}", vs) == 132
            && E("-1 % 5") == 4 && E("2 ^ 3 ^ 2") == 512 && E("-2 ^ 2") == -4 && E("10 - 2 - 3") == 5);
        c.Check("a formula's functions and comparisons", E("round(2.5)") == 3 && E("round(3.14159, 2)") == 3.14 && E("floor(2.9) + ceil(2.1)") == 5
            && E("max(1, n, 3)", vs) == 7 && E("min(4, 2)") == 2 && E("abs(-3)") == 3 && E("coins >= 100", vs) == 1 && E("n = 8 or n < 0", vs) == 0
            && E("not (n > 1) + 2", vs) == 2 && E("sqrt(9)") == 3);
        var dice = Enumerable.Range(0, 300).Select(_ => E("random(1, 6)")).ToList();
        c.Check("random(1, 6) is a whole number from 1 to 6, each seen", dice.All(d => d == Math.Floor(d) && d is >= 1 and <= 6) && dice.Distinct().Count() == 6);
        c.Check("a formula's mistakes show where",
            MacroExpr.Problem("2 +") is { } a1 && a1.Contains("stops") && MacroExpr.Problem("2 + * 3") is { } a2 && a2.Contains("* 3")
            && MacroExpr.Problem("foo(1)") is { } a3 && a3.Contains("foo") && MacroExpr.Problem("round()") is not null && MacroExpr.Problem("(1 + 2") is not null
            && MacroExpr.Problem("coins * 2 + random(1, 3)") is null, $"{MacroExpr.Problem("2 +")} / {MacroExpr.Problem("2 + * 3")} / {MacroExpr.Problem("foo(1)")}");
        c.Check("a formula that divides by 0 fails as it plays", Throws<MacroExprError>(() => E("1 / (n - 7)", vs)) && Throws<MacroValueMissing>(() => E("nope + 1")));
        c.Check("numbers read off the screen, as games write them",
            MacroExpr.NumberIn("Coins: 1,234") == 1234 && MacroExpr.NumberIn("1.234.567") == 1234567 && MacroExpr.NumberIn("12.5") == 12.5
            && MacroExpr.NumberIn("3.2K") == 3200 && MacroExpr.NumberIn("1.5M coins") == 1_500_000 && MacroExpr.NumberIn("2B") == 2e9
            && MacroExpr.NumberIn("x 12,5") == 12.5 && MacroExpr.NumberIn("1,234.56") == 1234.56 && MacroExpr.NumberIn("Level 7 of 10") == 7
            && MacroExpr.NumberIn("-15 HP") == -15 && MacroExpr.NumberIn("no number") is null && MacroExpr.NumberIn("Lv5") == 5 && MacroExpr.NumberIn("Wave 3 10 left") == 3,
            $"{MacroExpr.NumberIn("3.2K")} {MacroExpr.NumberIn("x 12,5")} {MacroExpr.NumberIn("Lv5")}");
        var has = new MacroTest { Kind = "var", Name = "said", Op = "has", Extra = new() { ["value"] = J("reached") } };
        c.Check("a var check: has (any case) and doesn't have", MacroValues.VarTest(has, _ => J("Level 12 REACHED"), false)
            && !MacroValues.VarTest(has, _ => J("Level 12"), false)
            && MacroValues.VarTest(new MacroTest { Kind = "var", Name = "said", Op = "!has", Extra = new() { ["value"] = J("lost") } }, _ => J("You won"), false));
    }

    static bool Throws<T>(Action a) where T : Exception
    {
        try
        {
            a();
            return false;
        }
        catch (T)
        {
            return true;
        }
    }

    // ───────────────────────── follow's maths ─────────────────────────

    static void FollowMaths(Checks c)
    {
        c.Check("on/off: hold while behind the target, let go past it, keep as is on it",
            MacroFollow.OnOff(20, 50) == true && MacroFollow.OnOff(80, 50) == false && MacroFollow.OnOff(51, 50) is null);
        using var bar = Bars(100, 10, (49, 3, Color.Lime), (19, 3, Color.White));
        var px = MacroFollow.Pixels.Of(bar);
        var sight = MacroFollow.Look(px, "#00ff00", new MacroMarker("#ffffff", null, null), 24);
        c.Check("the target line and the marker are found by colour", sight.Target is 50 && sight.Marker is 20, sight);
        using var box = Bars(100, 10, (49, 2, Color.Lime), (10, 2, Color.Red), (30, 2, Color.Blue));
        var edges = MacroFollow.Look(MacroFollow.Pixels.Of(box), "#00ff00", new MacroMarker(null, "#ff0000", "#0000ff"), 24);
        c.Check("a box's marker is the middle between its edges", edges.Marker is 20.5 && edges.Left is 10.5 && edges.Right is 30.5, edges);
        var none = MacroFollow.Look(MacroFollow.Pixels.Of(box), "#ff00ff", new MacroMarker("#ffffff", null, null), 24);
        c.Check("a colour not there is null", none.Target is null && none.Marker is null);

        var pd = new MacroFollow.Pd(1, 0, 100);
        c.Check("PD: the press grows with the distance, none once past, a whole frame at most",
            Math.Abs(pd.PressMs(60, 50, 16) - 6.4) < 1e-9 && pd.PressMs(40, 50, 16) == 0 && pd.PressMs(90, 10, 16) == 16);
        var damped = new MacroFollow.Pd(1, 0.4, 100);
        damped.PressMs(80, 50, 16); // error 0.3
        double closingFast = damped.PressMs(60, 50, 16); // error 0.1, closing by 0.2 a frame
        var plain = new MacroFollow.Pd(1, 0, 100);
        plain.PressMs(80, 50, 16);
        double undamped = plain.PressMs(60, 50, 16);
        c.Check("PD: closing in fast, damping presses less", closingFast < undamped && Math.Abs(closingFast - 1.28) < 1e-9 && Math.Abs(undamped - 6.4) < 1e-9,
            (closingFast, undamped));
        var predictor = new MacroFollow.Predictor();
        double ahead = 0;
        foreach (int x in new[] { 0, 4, 8, 12, 16, 20 })
            ahead = predictor.Ahead(x);
        c.Check("the predictor learns the speed and leads by two frames", Math.Abs(predictor.Speed - 3.875) < 1e-9 && Math.Abs(ahead - (20 + 2 * 3.875)) < 1e-9,
            (predictor.Speed, ahead));
        using var spots = Patches(100, 100, new Rectangle(10, 10, 20, 20), new Rectangle(60, 60, 2, 2), new Rectangle(70, 20, 10, 10));
        var blobs = MacroFollow.Blobs(MacroFollow.Pixels.Of(spots), "#00ff00", 24, 4);
        c.Check("patches of a colour, largest first, specks left out", blobs.Count == 2 && blobs[0] == new Rectangle(10, 10, 20, 20) && blobs[1] == new Rectangle(70, 20, 10, 10),
            string.Join(" ", blobs));
    }

    // ───────────────────────── the turn ─────────────────────────

    static async Task Turns(Checks c)
    {
        InputTurn.Reset();
        var a = InputTurn.TryEnter("afk")!;
        c.Check("a turn taken is busy, a second one waits", InputTurn.Busy && InputTurn.TryEnter("picture") is null);
        var b = InputTurn.EnterAsync("macro");
        var d = InputTurn.EnterAsync("arrange");
        c.Check("those waiting wait", !b.IsCompleted && !d.IsCompleted);
        a.Dispose();
        await Task.WhenAny(b, Task.Delay(2000));
        c.Check("the first to ask goes next, alone", b.IsCompletedSuccessfully && !d.IsCompleted && InputTurn.Holder == "macro");
        (await b).Dispose();
        await Task.WhenAny(d, Task.Delay(2000));
        c.Check("then the next", d.IsCompletedSuccessfully && InputTurn.Holder == "arrange");
        (await d).Dispose();
        c.Check("then it is free", !InputTurn.Busy);
        var held = InputTurn.TryEnter("afk")!;
        bool ran = false;
        InputTurn.RunWhenFree("hotkey", () => ran = true);
        c.Check("a hotkey waits while anti-AFK has a window", !ran);
        held.Dispose();
        // Ran on the pool: the turn is handed back just after the action, not with it.
        for (int i = 0; i < 100 && !(ran && !InputTurn.Busy); i++)
            await Task.Delay(10);
        c.Check("and runs once it is free", ran && !InputTurn.Busy);

        // A macro asked while anti-AFK has a window waits ("busy"), then plays.
        var (player, desk, accounts) = Setup();
        var afk = InputTurn.TryEnter("afk")!;
        var macro = M("""[{"kind":"key","key":"E"}]""");
        player.Queue(macro, accounts);
        var run = player.Runs[0];
        c.Check("a run waits for anti-AFK's turn", run is { Status: "paused", Reason: "busy" } && !desk.Log.Any(l => l.Contains("key")), $"{run.Status} {run.Reason}");
        afk.Dispose();
        await Done(player);
        c.Check("then plays", run.Status == "done" && desk.Log.Any(l => l.Contains("key E down")), $"{run.Status} {string.Join(" | ", desk.Log)}");
        InputTurn.Reset();
    }

    // ───────────────────────── the player ─────────────────────────

    static async Task Player(Checks c)
    {
        {
            var (player, desk, accounts) = Setup(2);
            var m = M("""[{"kind":"loop","times":3},{"kind":"key","key":"A"},{"kind":"wait","ms":2000},{"kind":"endLoop"}]""");
            player.Queue(m, accounts);
            await Done(player);
            string turns = string.Join(",", desk.Log.Where(l => l.StartsWith("turn")).Select(l => l[5..]));
            c.Check("windows take turns round the queue", turns == "1,2,1,2,1,2" && player.Runs.All(r => r.Status == "done"), turns);
            c.Check("a turn gives the window back as it ends", desk.Log.Count(l => l.StartsWith("end")) == 6);
        }
        {
            var (player, desk, accounts) = Setup();
            var seen = new List<string>();
            desk.HoldNow = () => desk.Clock < desk.Start.AddSeconds(2) ? "personActive" : null;
            player.Changed = () => seen.Add($"{player.Runs.FirstOrDefault()?.Status}/{player.Runs.FirstOrDefault()?.Reason}");
            player.Queue(M("""[{"kind":"key","key":"E"}]"""), accounts);
            await Done(player);
            int pressedAt = int.Parse(desk.Log.First(l => l.Contains("key E down")).Split('@')[1]);
            c.Check("a person at the PC pauses runs, which go on once they stop", seen.Contains("paused/personActive") && player.Runs[0].Status == "done"
                && pressedAt >= 2000, $"{pressedAt} {string.Join(" ", seen)}");
        }
        {
            var (player, desk, accounts) = Setup();
            bool stopped = false;
            desk.OnDelay = () =>
            {
                if (!stopped && desk.Log.Any(l => l.Contains("key W down")) && desk.Ms > 300)
                {
                    stopped = true;
                    player.Stop(null, "byUser");
                }
            };
            player.Queue(M("""[{"kind":"down","key":"W"},{"kind":"wait","ms":1000},{"kind":"key","key":"E"},{"kind":"up","key":"W"}]"""), accounts);
            await Done(player);
            int down = desk.Log.FindIndex(l => l.Contains("key W down")), up = desk.Log.FindIndex(l => l.Contains("key W up"));
            c.Check("stopped while a key is held: it is let go, nothing after", player.Runs[0] is { Status: "stopped", Reason: "byUser" } && down >= 0 && up > down
                && !desk.Log.Any(l => l.Contains("key E")) && player.Runs[0].Held.Count == 0, string.Join(" | ", desk.Log));
        }
        {
            // "Stop trying" in the editor: only Try it's run goes, the saved macro (same id) goes on on its other account.
            var (player, desk, accounts) = Setup(2);
            var m = M("""[{"kind":"wait","ms":5000}]""");
            int stopped = -1;
            desk.OnDelay = () =>
            {
                if (stopped < 0 && player.Runs.Count == 2 && desk.Ms > 1000)
                    stopped = player.Stop(m.Id, "byUser", accounts[1], trialOnly: true);
            };
            player.Queue(m, [accounts[0]]);
            player.Queue(m, [accounts[1]], trial: true);
            await Done(player);
            var real = player.Runs.First(r => !r.Trial);
            c.Check("stopping Try it leaves the saved macro's run going", stopped == 1 && real.Status == "done" && player.Runs.First(r => r.Trial).Status == "stopped",
                $"{stopped} {real.Status}");
        }
        {
            // Restart: a wait that runs out goes through the If stuck part, then from the top; it gives up after five.
            var (player, desk, accounts) = Setup();
            desk.Picture = () => new Bitmap(10, 10);
            player.Queue(M("""[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"E"},{"kind":"recover"},{"kind":"key","key":"Q"}]""",
                ""","onFail":"restart" """), accounts);
            await Done(player);
            int q = desk.Log.Count(l => l.Contains("key Q down"));
            c.Check("a wait run out: the If stuck part, then from the top; it gives up after 5 in a row", player.Runs[0] is { Status: "failed", Reason: "stuck" }
                && q == MacroPlayer.MaxRestarts && !desk.Log.Any(l => l.Contains("key E")), $"{player.Runs[0].Status}/{player.Runs[0].Reason} Q×{q}");
        }
        {
            // Stuck: waits passed over ("skip") still count; time held back while the person uses the PC doesn't.
            var (player, desk, accounts) = Setup();
            desk.Picture = () => new Bitmap(10, 10);
            desk.HoldNow = () => desk.Clock >= desk.Start.AddSeconds(20) && desk.Clock < desk.Start.AddMinutes(3) ? "personActive" : null;
            player.Queue(M("""[{"kind":"loop","times":0},{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"E"},{"kind":"endLoop"},{"kind":"recover"},{"kind":"key","key":"Q"}]""",
                ""","onFail":"skip","stuckMinutes":1,"maxMinutes":30 """), accounts);
            await Done(player);
            string? first = desk.Log.FirstOrDefault(l => l.Contains("key Q down"));
            int at = first is null ? -1 : int.Parse(first.Split('@')[1]);
            c.Check("finding nothing for its stuck time runs the If stuck part, time held back left out", at >= 220_000 && at < 240_000
                && desk.Log.Any(l => l.Contains("key E")), at);
            c.Check("still stuck after five restarts: it gives up", player.Runs[0] is { Status: "failed", Reason: "stuck" }, $"{player.Runs[0].Status}/{player.Runs[0].Reason}");
        }
        {
            // A question answered "no" isn't stuck: "if the pop-up shows, close it" can say no for hours.
            var (player, desk, accounts) = Setup();
            desk.Picture = () => new Bitmap(10, 10);
            player.Queue(M("""[{"kind":"loop","times":0},{"kind":"if","test":{"kind":"colour","x":0.5,"y":0.5,"colour":"#ff0000"}},{"kind":"key","key":"Escape"},{"kind":"endIf"},{"kind":"wait","ms":30000},{"kind":"endLoop"}]""",
                ""","onFail":"restart","stuckMinutes":1,"maxMinutes":10 """), accounts);
            await Done(player);
            c.Check("an If answered no for a long time isn't stuck", player.Runs[0] is { Status: "stopped", Reason: "maxTime" }, $"{player.Runs[0].Status}/{player.Runs[0].Reason}");
        }
        {
            var (player, desk, accounts) = Setup();
            desk.Picture = () => new Bitmap(10, 10);
            player.Queue(M("""[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"K"}]""", ""","onFail":"skip" """), accounts);
            await Done(player);
            c.Check("onFail skip: past a wait that timed out to the next step", player.Runs[0].Status == "done" && desk.Log.Any(l => l.Contains("key K down")));
            var (player2, desk2, accounts2) = Setup();
            desk2.Picture = () => new Bitmap(10, 10);
            player2.Queue(M("""[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"K"}]"""), accounts2);
            await Done(player2);
            c.Check("onFail stop: the run fails (timeout)", player2.Runs[0] is { Status: "failed", Reason: "timeout" } && !desk2.Log.Any(l => l.Contains("key K")));
        }
        {
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"loop","times":3},{"kind":"addVar","name":"n","value":1},{"kind":"endLoop"},{"kind":"if","test":{"kind":"var","name":"n","op":"=","value":3}},{"kind":"key","key":"Y"},{"kind":"else"},{"kind":"key","key":"N"},{"kind":"endIf"}]"""), accounts);
            await Done(player);
            c.Check("loops and variables: three passes, then the if's yes", player.Vars[accounts[0]]["n"] == 3 && desk.Log.Any(l => l.Contains("key Y down"))
                && !desk.Log.Any(l => l.Contains("key N")));
        }
        {
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"wait","ms":"$o1"},{"kind":"key","key":"$o2"},{"kind":"key","key":"E","ms":"$o9"}]""",
                ""","options":[{"name":"o1","label":"Every","kind":"number","value":250,"default":1000},{"name":"o2","label":"Key","kind":"key","value":"Q","default":"E"}]"""), accounts);
            await Done(player);
            c.Check("a setting is read as it plays (a number, a key); a missing one fails the run (missingValue)",
                player.Runs[0] is { Status: "failed", Reason: "missingValue" } && desk.Ms >= 250 && desk.Log.Any(l => l.Contains("key Q down")) && !desk.Log.Any(l => l.Contains("key E")),
                $"{player.Runs[0].Status} {player.Runs[0].Reason} {string.Join(" | ", desk.Log)}");
        }
        {
            var (player, desk, accounts) = Setup();
            desk.Picture = () => Patches(800, 600, new Rectangle(500, 400, 20, 20), new Rectangle(100, 100, 20, 20));
            player.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":"@a1"}]""",
                ""","areas":[{"name":"a1","label":"Button area","rect":{"x":0.5,"y":0.5,"w":0.5,"h":0.5} }]"""), accounts);
            await Done(player);
            c.Check("a step looks in its area (\"@a1\"): the patch inside it is clicked, not the one outside", desk.Log.Contains("1 move 610,510") && player.Runs[0].Status == "done",
                string.Join(" | ", desk.Log));
            var (player2, _, accounts2) = Setup();
            player2.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":"@a2"}]"""), accounts2);
            await Done(player2);
            c.Check("an area the macro doesn't have fails the run (missingValue)", player2.Runs[0] is { Status: "failed", Reason: "missingValue" });
        }
        {
            var (player, desk, accounts) = Setup();
            int asked = 0;
            desk.ClearAt = _ => ++asked > 1;
            var seen = new List<string>();
            player.Changed = () => seen.Add($"{player.Runs[0].Status}/{player.Runs[0].Reason}");
            player.Queue(M("""[{"kind":"click","x":0.5,"y":0.5,"holdMs":500}]"""), accounts);
            await Done(player);
            var down = desk.Log.FirstOrDefault(l => l.Contains("left down"));
            var up = desk.Log.FirstOrDefault(l => l.Contains("left up"));
            int held = down is null || up is null ? -1 : int.Parse(up.Split('@')[1]) - int.Parse(down.Split('@')[1]);
            c.Check("another app's window at the click: no click, 'covered', tried again", seen.Contains("paused/covered") && asked >= 2 && player.Runs[0].Status == "done",
                string.Join(" ", seen));
            c.Check("a click held 500 ms, in the middle of the window", held == 500 && desk.Log.Contains("1 move 500,400"), string.Join(" | ", desk.Log));
        }
        {
            var (player, desk, accounts) = Setup();
            // A bar 800 px wide: the marker behind, then past, then the bar goes.
            int frame = 0;
            desk.Area = area =>
            {
                frame++;
                return frame switch
                {
                    <= 3 => Bars(area.Width, area.Height, (400, 3, Color.Lime), (100, 3, Color.White)),
                    <= 6 => Bars(area.Width, area.Height, (400, 3, Color.Lime), (600, 3, Color.White)),
                    _ => Bars(area.Width, area.Height),
                };
            };
            player.Queue(M("""[{"kind":"follow","region":{"x":0,"y":0.5,"w":1,"h":0.1},"target":"#00ff00","marker":"#ffffff","untilGoneMs":100,"timeoutMs":5000},{"kind":"key","key":"Z"}]"""), accounts);
            await Done(player);
            var presses = desk.Log.Where(l => l.Contains("left ")).Select(l => l.Split(' ')[2]).ToList();
            c.Check("follow (on/off): holds while the marker is behind, lets go past, ends when the bar goes",
                string.Join(",", presses) == "down,up" && desk.Log.Any(l => l.Contains("key Z down")) && player.Runs[0].Status == "done", string.Join(" | ", desk.Log));
            c.Check("follow points into the bar's area first", desk.Log.Contains("1 move 500,430"), string.Join(" | ", desk.Log));
        }
        {
            var (player, desk, accounts) = Setup();
            desk.Area = area => Bars(area.Width, area.Height, (400, 3, Color.Lime), (100, 3, Color.White));
            player.Queue(M("""[{"kind":"follow","region":{"x":0,"y":0.5,"w":1,"h":0.1},"target":"#00ff00","marker":"#ffffff","mode":"smooth","timeoutMs":200}]"""), accounts);
            await Done(player);
            int downs = desk.Log.Count(l => l.Contains("left down")), ups = desk.Log.Count(l => l.Contains("left up"));
            c.Check("follow (smooth): short pulses, every one let go; the timeout is the macro's onFail", downs > 2 && downs == ups && player.Runs[0] is { Status: "failed", Reason: "timeout" },
                $"{downs}/{ups} {player.Runs[0].Status} {player.Runs[0].Reason}");
        }
        {
            var (player, desk, accounts) = Setup();
            desk.Picture = () => Patches(800, 600, new Rectangle(400, 300, 20, 20));
            player.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1}}]"""), accounts);
            await Done(player);
            c.Check("clickColour clicks the middle of the patch", desk.Log.Contains("1 move 510,410") && desk.Log.Any(l => l.Contains("left down")) && player.Runs[0].Status == "done",
                string.Join(" | ", desk.Log));
        }
        {
            // Each patch once, when it has shown 300 ms: one there 200 ms only isn't clicked.
            var (player, desk, accounts) = Setup();
            desk.Picture = () => desk.Ms switch
            {
                < 1000 => Patches(800, 600, new Rectangle(100, 100, 20, 20), new Rectangle(400, 300, 20, 20)),
                < 1500 => new Bitmap(800, 600),
                < 1700 => Patches(800, 600, new Rectangle(600, 100, 20, 20)),
                _ => new Bitmap(800, 600),
            };
            int firstAt = -1;
            desk.OnDelay = () => { if (firstAt < 0 && desk.Log.Any(l => l.Contains("left down"))) firstAt = desk.Ms; };
            player.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"eachAfterMs":300,"repeatUntilGoneMs":1500}]"""), accounts);
            await Done(player);
            c.Check("click each patch: every one once, after it has shown that long; one gone sooner isn't clicked",
                desk.Log.Count(l => l.EndsWith("move 210,210")) == 1 && desk.Log.Count(l => l.EndsWith("move 510,410")) == 1
                && !desk.Log.Any(l => l.EndsWith("move 710,210")) && desk.Log.Count(l => l.Contains("left down")) == 2
                && firstAt >= 300 && player.Runs[0].Status == "done", $"{firstAt} {player.Runs[0].Status} {string.Join(" | ", desk.Log)}");
        }
        {
            // Text read from a list the player ticks: a letter read wrong is forgiven; a longer name isn't a shorter one.
            var (player, desk, accounts) = Setup();
            int reads = 0;
            desk.Picture = () => new Bitmap(800, 600);
            desk.Lines = () => [++reads <= 2 ? Said("Enchant", "Blessed Song") : Said("Enchant", "Abysal")];
            player.Queue(M("""[{"kind":"waitText","words":"$o1","region":{"x":0,"y":0,"w":1,"h":1}},{"kind":"key","key":"Y"}]""",
                ""","options":[{"name":"o1","label":"Want","kind":"list","choices":["Abyssal","Blessed","Blessed Song"],"value":["abyssal","Blessed"]}]"""), accounts);
            await Done(player);
            c.Check("wait for any ticked word: Abysal read counts as Abyssal, Blessed Song doesn't count as Blessed",
                reads == 3 && desk.Log.Any(l => l.Contains("key Y down")) && player.Runs[0].Status == "done", $"{reads} {player.Runs[0].Status} {player.Runs[0].Reason}");
        }
        {
            var (player, desk, accounts) = Setup();
            desk.Picture = () => new Bitmap(800, 600);
            desk.Lines = () => [Said("Abyssal")];
            player.Queue(M("""[{"kind":"if","test":{"kind":"text","words":"$o1"}},{"kind":"key","key":"Y"},{"kind":"else"},{"kind":"key","key":"N"},{"kind":"endIf"}]""",
                ""","options":[{"name":"o1","label":"Want","kind":"list","choices":["Abyssal","Blessed"],"value":[]}]"""), accounts);
            await Done(player);
            c.Check("…none ticked: nothing counts", desk.Log.Any(l => l.Contains("key N down")) && !desk.Log.Any(l => l.Contains("key Y")), string.Join(" | ", desk.Log));
        }
        {
            var (player, desk, accounts) = Setup(2);
            int shots = 0;
            desk.Picture = () => ++shots <= 4 ? Patches(800, 600, new Rectangle(100, 100, 20, 20)) : new Bitmap(800, 600);
            var states = new List<string>();
            player.Changed = () =>
            {
                if (player.Runs.Count == 2 && player.Runs[0].Status == "running")
                    states.Add(player.Runs[1].Reason ?? "");
            };
            player.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"repeatUntilGoneMs":300}]"""), [accounts[0]]);
            player.Queue(M("""[{"kind":"key","key":"Q"}]"""), [accounts[1]]);
            await Done(player);
            int clicks = desk.Log.Count(l => l.StartsWith("1 left down"));
            int lastClick = desk.Log.FindLastIndex(l => l.StartsWith("1 left up")), q = desk.Log.FindIndex(l => l.Contains("key Q"));
            c.Check("a repeating clickColour clicks each time it shows, holding the turn: the other window waits", clicks == 4 && q > lastClick,
                $"{clicks} {string.Join(" | ", desk.Log)}");
        }
        {
            var (player, desk, accounts) = Setup();
            string? said = null;
            player.Alert = (_, _, text, _) => said = text;
            player.Queue(M("""[{"kind":"setVar","name":"caught","value":3},{"kind":"alert","text":"Caught {caught} with {o1} {nope}"}]""",
                ""","options":[{"name":"o1","label":"Rod","kind":"choice","choices":["Wood","Gold"],"value":"gold"}]"""), accounts);
            await Done(player);
            c.Check("an alert's text takes {name}: counters and settings", said == "Caught 3 with Gold {nope}", said);
        }
        {
            // Read a number into a variable, work it out in a formula, keep a text, type it.
            var (player, desk, accounts) = Setup();
            int reads = 0;
            desk.Picture = () => new Bitmap(800, 600);
            desk.Lines = () => [++reads <= 1 ? Said("Coins:") : Said("Coins: 1,250")];
            string? said = null;
            player.Alert = (_, _, text, _) => said = text;
            player.Queue(M("""[{"kind":"read","name":"coins","region":{"x":0,"y":0,"w":0.5,"h":0.2}},{"kind":"setVar","name":"left","expr":"max(0, 2000 - coins)"},{"kind":"addVar","name":"left","expr":"round(coins / 100)"},{"kind":"setVar","name":"note","text":"need {left} more"},{"kind":"if","test":{"kind":"var","name":"note","op":"has","value":"763"}},{"kind":"alert","text":"{note}"},{"kind":"endIf"}]"""), accounts);
            await Done(player);
            c.Check("read a number (tried again until one shows), formulas, a text variable, has", reads == 2 && player.Vars[accounts[0]]["coins"] == 1250
                && player.Vars[accounts[0]]["left"] == 763 && player.Texts[accounts[0]]["note"] == "need 763 more" && said == "need 763 more" && player.Runs[0].Status == "done",
                $"{reads} {said} {player.Runs[0].Status} {player.Runs[0].Reason}");
        }
        {
            var (player, desk, accounts) = Setup();
            desk.Picture = () => new Bitmap(800, 600);
            desk.Lines = () => [Said("nothing here")];
            player.Queue(M("""[{"kind":"read","name":"coins","region":{"x":0,"y":0,"w":0.5,"h":0.2},"timeoutMs":1000},{"kind":"key","key":"K"}]"""), accounts);
            await Done(player);
            var (player2, _, accounts2) = Setup();
            player2.Queue(M("""[{"kind":"setVar","name":"n","value":0},{"kind":"setVar","name":"x","expr":"5 / n"},{"kind":"key","key":"K"}]"""), accounts2);
            await Done(player2);
            c.Check("no number read in time fails as a wait does; a division by 0 fails the run (badFormula)",
                player.Runs[0] is { Status: "failed", Reason: "timeout" } && player2.Runs[0] is { Status: "failed", Reason: "badFormula" },
                $"{player.Runs[0].Reason} {player2.Runs[0].Reason}");
        }
        {
            // Run a macro: the other's steps play here, its settings read as it starts; its If stuck part doesn't.
            var (player, desk, accounts) = Setup();
            var inner = M("""[{"kind":"key","key":"$o1"},{"kind":"addVar","name":"n","value":1},{"kind":"recover"},{"kind":"key","key":"Z"}]""",
                ""","options":[{"name":"o1","label":"Key","kind":"key","value":"Q","default":"E"}]""");
            var outer = M($$"""[{"kind":"loop","times":2},{"kind":"call","macroId":"{{inner.Id}}"},{"kind":"endLoop"},{"kind":"key","key":"Y"}]""");
            player.FindMacro = id => id == inner.Id ? MacroRules.Copy(inner) : null;
            player.Queue(outer, accounts);
            await Done(player);
            c.Check("run a macro: its steps play in place, twice in a repeat, with its own setting; not its If stuck part",
                desk.Log.Count(l => l.Contains("key Q down")) == 2 && player.Vars[accounts[0]]["n"] == 2 && desk.Log.Any(l => l.Contains("key Y down"))
                && !desk.Log.Any(l => l.Contains("key Z")) && player.Runs[0].Status == "done", $"{player.Runs[0].Status} {player.Runs[0].Reason} {string.Join(" | ", desk.Log)}");
            var (player2, desk2, accounts2) = Setup();
            player2.FindMacro = _ => null;
            player2.Queue(M($$"""[{"kind":"key","key":"A"},{"kind":"call","macroId":"{{Guid.NewGuid()}}"},{"kind":"key","key":"B"}]"""), accounts2);
            await Done(player2);
            c.Check("a macro it runs that's gone fails the run there (missingMacro), shown at the Run a macro step",
                player2.Runs[0] is { Status: "failed", Reason: "missingMacro", ShownPc: 1 } && desk2.Log.Any(l => l.Contains("key A down")) && !desk2.Log.Any(l => l.Contains("key B")),
                $"{player2.Runs[0].Reason} {player2.Runs[0].ShownPc}");
            var self = M("""[{"kind":"key","key":"A"}]""");
            var other = M($$"""[{"kind":"call","macroId":"{{self.Id}}"}]""");
            self.Steps.Add(new MacroStep { Kind = "call", MacroId = other.Id });
            string? Calls(Macro m, params Macro[] all) => Refused(() => MacroRules.CheckCalls(m, all));
            c.Check("saving: the macro to run is saved, not itself, and doesn't run this one back",
                Calls(self, self, other) is { } back && back.Contains("back") && Calls(other, other) is { } gone && gone.Contains("isn't there")
                && Calls(M($$"""[{"kind":"call","macroId":"{{Guid.NewGuid()}}"}]""")) is not null && Calls(other, other, M("""[{"kind":"key","key":"A"}]""")) is not null
                && CheckOf("""[{"kind":"call"}]""") is not null, $"{Calls(self, self, other)} / {Calls(other, other)}");
        }
        {
            // Read from the clipboard, a file, the web; write to them. A fake outside: nothing on this PC is touched.
            var (player, desk, accounts) = Setup();
            var outside = new FakeOutside { Clip = "Gems 3.5K", Files = { ["coins log"] = "12\n40" }, Web = { ["https://x.test/n"] = "{\"count\": 7}" } };
            player.Outside = outside;
            player.Queue(M("""[{"kind":"read","from":"clipboard","name":"gems"},{"kind":"read","from":"file","fileName":"coins log","name":"coins"},{"kind":"read","from":"web","url":"https://x.test/n","name":"n"},{"kind":"write","to":"file","fileName":"out","text":"gems {gems} coins {coins} n {n}"},{"kind":"write","to":"clipboard","text":"{coins}"},{"kind":"write","to":"web","url":"https://x.test/hook","text":"done {n}"}]"""), accounts);
            await Done(player);
            var v = player.Vars[accounts[0]];
            c.Check("read from the clipboard, a file's last line, a web answer; write a line, the clipboard, a web post",
                v["gems"] == 3500 && v["coins"] == 40 && v["n"] == 7 && outside.Files["out"] == "gems 3500 coins 40 n 7" && outside.Clip == "40"
                && outside.Posts.SequenceEqual(["https://x.test/hook done 7"]) && player.Runs[0].Status == "done",
                $"{player.Runs[0].Status} {player.Runs[0].Reason} {string.Join(",", v.Select(x => x.Key + "=" + x.Value))} {outside.Files.GetValueOrDefault("out")}");
            c.Check("saving: a file is a plain name, a web address https",
                CheckOf("""[{"kind":"write","to":"file","fileName":"../x","text":"a"}]""") is not null && CheckOf("""[{"kind":"write","to":"web","url":"http://x.test","text":"a"}]""") is not null
                && CheckOf("""[{"kind":"read","from":"file","fileName":"coins log","name":"c"}]""") is null && CheckOf("""[{"kind":"read","name":"c"}]""") is not null);
            c.Check("web steps go only to public addresses", !MacroOutside.Public(System.Net.IPAddress.Parse("127.0.0.1")) && !MacroOutside.Public(System.Net.IPAddress.Parse("192.168.1.5"))
                && !MacroOutside.Public(System.Net.IPAddress.Parse("10.0.0.1")) && !MacroOutside.Public(System.Net.IPAddress.Parse("::1")) && !MacroOutside.Public(System.Net.IPAddress.Parse("169.254.169.254"))
                && MacroOutside.Public(System.Net.IPAddress.Parse("162.159.135.232")));
        }
        {
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"if","test":{"kind":"var","name":"o1","op":"=","value":1}},{"kind":"key","key":"Y"},{"kind":"else"},{"kind":"key","key":"N"},{"kind":"endIf"}]""",
                ""","options":[{"name":"o1","label":"Auto sell","kind":"check","value":true,"default":false}]"""), accounts);
            await Done(player);
            c.Check("an if on a switch the player sets", desk.Log.Any(l => l.Contains("key Y down")) && !desk.Log.Any(l => l.Contains("key N")), string.Join(" | ", desk.Log));
        }
        {
            // A point the player sets on the panel: the click's x and y both name it.
            var (player, desk, accounts) = Setup();
            var m = M("""[{"kind":"click","x":"$o1","y":"$o1","button":"left"}]""",
                ""","options":[{"name":"o1","label":"Water","kind":"point","value":{"x":0.25,"y":0.75}}]""");
            player.Queue(m, accounts);
            await Done(player);
            c.Check("a click at a point the player set (x and y from one setting)", desk.Log.Any(l => l.EndsWith("move 300,550")) && player.Runs[0].Status == "done",
                string.Join(" | ", desk.Log));
            c.Check("…and that point shows on the game as the step's dot", AreaGeometry.Dots(m) is [{ Step: 0, X: 0.25, Y: 0.75 }]);
        }
        {
            var (player, desk, accounts) = Setup();
            desk.Windows.Clear();
            player.Queue(M("""[{"kind":"key","key":"E"}]"""), accounts);
            await Done(player);
            c.Check("a window gone fails the run", player.Runs[0] is { Status: "failed", Reason: "windowGone" });
        }
        {
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"loop","times":0},{"kind":"key","key":"E"},{"kind":"endLoop"}]""", ""","maxMinutes":1"""), accounts);
            await Done(player);
            c.Check("a run ends at its longest time (maxTime)", player.Runs[0] is { Status: "stopped", Reason: "maxTime" } && desk.Ms >= 60_000 && desk.Ms < 70_000, desk.Ms);
        }
    }

    // ───────────────────────── how long a turn keeps the PC ─────────────────────────

    /// <summary>The time on the desk's clock a log line has ("… @1234").</summary>
    static int At(string line) => int.Parse(line.Split('@')[1]);

    static string State(MacroRun run) => $"{run.Status}/{run.Reason}";

    /// <summary>
    /// A turn holds InputTurn, and anti-AFK for every other window waits behind it: no step may keep it past the run's
    /// longest time, while the person uses the PC, or long after anti-AFK asked. Each case has a guard far past where it
    /// should end, so a regression fails instead of running forever.
    /// </summary>
    // ───────────────────────── features (a switch with its own steps) ─────────────────────────

    /// <summary>A macro with two features, Sell (presses S) and Eat (presses E), both switched on unless told; extra
    /// replaces their steps or adds fields.</summary>
    static string Feats(string sell = """[{"kind":"key","key":"S"}]""", string eat = """[{"kind":"key","key":"E"}]""", string sellMore = "", int sellOn = 1, int eatOn = 1) =>
        $$""","options":[{"name":"o1","label":"Sell","kind":"check","value":{{sellOn}}},{"name":"o2","label":"Eat","kind":"check","value":{{eatOn}}},{"name":"o3","label":"Key","kind":"key","value":"K"}],"features":[{"id":"f1","option":"o1"{{sellMore}},"steps":{{sell}}},{"id":"f2","option":"o2","steps":{{eat}}}]""";

    /// <summary>The keys pressed, in order (one letter each).</summary>
    static string Keys(FakeDesk desk) => string.Concat(desk.Log.Where(l => l.Contains(" key ") && l.Contains(" down")).Select(l => l.Split(' ')[2]));

    static async Task Features(Checks c)
    {
        c.Check("a feature is taken with its own steps", CheckOf("""[{"kind":"key","key":"A"}]""", Feats()) is null, CheckOf("""[{"kind":"key","key":"A"}]""", Feats()));
        c.Check("a feature whose switch is gone is refused",
            CheckOf("""[{"kind":"key","key":"A"}]""", Feats().Replace("\"option\":\"o2\"", "\"option\":\"o9\"")) is not null);
        c.Check("a feature on a setting that isn't a switch is refused",
            CheckOf("""[{"kind":"key","key":"A"}]""", Feats().Replace("\"option\":\"o2\"", "\"option\":\"o3\"")) is not null);
        c.Check("a repeat never ended in a feature is refused, naming it",
            CheckOf("""[{"kind":"key","key":"A"}]""", Feats(sell: """[{"kind":"loop","times":2}]"""))?.StartsWith("Sell:") == true);
        c.Check("a key a feature holds and never lets go is refused",
            CheckOf("""[{"kind":"key","key":"A"}]""", Feats(sell: """[{"kind":"down","key":"W"}]""")) is not null);
        c.Check("If stuck in a feature is refused", CheckOf("""[{"kind":"key","key":"A"}]""", Feats(sell: """[{"kind":"recover"}]""")) is not null);
        c.Check("the steps of the macro and its features count together (500)",
            CheckOf("[" + string.Join(",", Enumerable.Repeat("""{"kind":"key","key":"A"}""", 499)) + "]", Feats()) is not null);
        c.Check("the main steps' safe points: the end of each pass of a repeat inside no other block",
            string.Join(",", MacroPlayer.SafeEnds(Read("""[{"kind":"loop","times":0},{"kind":"if","test":{"kind":"var","name":"n","op":"=","value":1}},{"kind":"loop","times":2},{"kind":"endLoop"},{"kind":"endIf"},{"kind":"endLoop"},{"kind":"loop","times":2},{"kind":"endLoop"}]""").Steps)) == "5,7");
        {
            // Two features due at every pass: each plays whole, in the list's order, then the repeat goes on (its passes kept).
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"loop","times":3},{"kind":"key","key":"A"},{"kind":"endLoop"}]""", Feats()), accounts);
            await Done(player);
            c.Check("features play at each pass's end and the end, in order, never inside other steps", Keys(desk) == "ASEASEASESE" && player.Runs[0].Status == "done",
                Keys(desk));
        }
        {
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"loop","times":2},{"kind":"key","key":"A"},{"kind":"endLoop"}]""", Feats(eatOn: 0)), accounts);
            await Done(player);
            c.Check("a feature switched off doesn't play", Keys(desk) == "ASASS", Keys(desk));
        }
        {
            // Sell every 10 s over a minute of a 1 s round: about six times, the first at the first safe point.
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"loop","times":0},{"kind":"key","key":"A"},{"kind":"wait","ms":1000},{"kind":"endLoop"}]""",
                ""","maxMinutes":1""" + Feats(sellMore: ""","everyMs":10000""", eatOn: 0)), accounts);
            await Done(player);
            string keys = Keys(desk);
            int sells = keys.Count(k => k == 'S');
            c.Check("a feature every 10 s plays every 10 s, not every round", sells is >= 5 and <= 7 && keys.StartsWith("AS") && keys.Count(k => k == 'A') > 50,
                $"{sells} {keys}");
        }
        {
            // Switched off on the panel while it plays: it stops at its next step, the main steps go on.
            var (player, desk, accounts) = Setup();
            var m = M("""[{"kind":"loop","times":2},{"kind":"key","key":"A"},{"kind":"endLoop"}]""",
                Feats(sell: """[{"kind":"key","key":"S"},{"kind":"wait","ms":1000},{"kind":"key","key":"T"}]""", eatOn: 0));
            desk.OnDelay = () =>
            {
                if (desk.Log.Any(l => l.Contains("key S down")))
                    m.Options[0].Value = J(0);
            };
            player.Queue(m, accounts);
            await Done(player);
            c.Check("a feature switched off while it plays stops there; the rest goes on", Keys(desk) == "ASA" && player.Runs[0].Status == "done", Keys(desk));
        }
        {
            // W held through the repeat: no feature at its passes (they'd play with W down); at the end, once let go.
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"down","key":"W"},{"kind":"loop","times":2},{"kind":"key","key":"A"},{"kind":"endLoop"},{"kind":"up","key":"W"}]""", Feats(eatOn: 0)), accounts);
            await Done(player);
            int up = desk.Log.FindIndex(l => l.Contains("key W up")), sell = desk.Log.FindIndex(l => l.Contains("key S down"));
            c.Check("no feature plays while a key is held", Keys(desk) == "WAAS" && sell > up, Keys(desk));
        }
        {
            // Its picture never shows: skipped this time (said on the panel), the main steps go on, the run isn't failed.
            var (player, desk, accounts) = Setup();
            desk.Picture = () => new Bitmap(10, 10);
            player.Queue(M("""[{"kind":"key","key":"A"}]""",
                Feats(sell: """[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":500},{"kind":"key","key":"T"}]""")), accounts);
            await Done(player);
            var record = player.Features.GetValueOrDefault((player.Runs[0].Macro.Id, accounts[0], "f1"));
            c.Check("a feature that finds nothing is skipped, the next plays, the run goes on", Keys(desk) == "AE" && player.Runs[0].Status == "done"
                && record is { Failed: "timeout", FailedStep: 0 }, $"{Keys(desk)} {player.Runs[0].Status} {record?.Failed}");
        }
        {
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"key","key":"A"}]""", Feats(sell: """[{"kind":"key","key":"$o3"}]""", eatOn: 0)), accounts);
            await Done(player);
            c.Check("a feature's step reads the panel's settings", Keys(desk) == "AK", Keys(desk));
        }
        {
            // One macro on a window at a time.
            var (player, desk, accounts) = Setup();
            var first = M("""[{"kind":"wait","ms":3000}]""");
            int two = -1;
            Macro? busy = null;
            desk.OnDelay = () =>
            {
                // Queued while the first plays (the made-up desk plays a queue through at once).
                if (two < 0 && desk.Ms > 1000)
                {
                    two = player.Queue(M("""[{"kind":"key","key":"A"}]"""), accounts);
                    busy = player.BusyWith(accounts[0], Guid.NewGuid());
                }
            };
            int one = player.Queue(first, accounts);
            await Done(player);
            c.Check("a second macro on a window running one isn't queued, and which is running there is said", one == 1 && two == 0
                && !Keys(desk).Contains('A') && busy == first, $"{one} {two}");
        }
    }

    static async Task Bounds(Checks c)
    {
        InputTurn.Reset();
        {
            // A colour that never goes (it also matches a permanent button): clicked again and again.
            var (player, desk, accounts) = Setup();
            desk.Picture = () => Patches(200, 150, new Rectangle(100, 75, 10, 10));
            desk.OnDelay = () => { if (desk.Ms > 300_000) player.Stop(null, "guard"); };
            player.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"repeatUntilGoneMs":300}]""", ""","maxMinutes":1"""), accounts);
            await Done(player);
            c.Check("a repeating clickColour whose colour never goes ends at the run's longest time, the turn given back",
                player.Runs[0] is { Status: "stopped", Reason: "maxTime" } && desk.Ms < 70_000 && !InputTurn.Busy, $"{State(player.Runs[0])} {desk.Ms} busy={InputTurn.Busy}");
        }
        {
            // Within a long longest time: the step itself gives up after 10 minutes (its timeout if longer): the macro's onFail.
            var (player, desk, accounts) = Setup();
            desk.Picture = () => Patches(40, 30, new Rectangle(20, 15, 6, 6));
            desk.OnDelay = () => { if (desk.Ms > 1_500_000) player.Stop(null, "guard"); };
            player.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"repeatUntilGoneMs":300}]""", ""","maxMinutes":60"""), accounts);
            await Done(player);
            c.Check("…and within a longer one, the step gives up after 10 minutes (the macro's onFail)",
                player.Runs[0] is { Status: "failed", Reason: "timeout" } && desk.Ms is >= 600_000 and < 620_000 && !InputTurn.Busy, $"{State(player.Runs[0])} {desk.Ms}");
        }
        {
            // Click each patch: a patch somewhere new every look, so "nothing new for a while" never comes.
            var (player, desk, accounts) = Setup();
            int look = 0;
            desk.Picture = () => { int k = look++ / 4; return Patches(200, 150, new Rectangle(k * 37 % 180, k * 23 % 130, 8, 8)); };
            desk.OnDelay = () => { if (desk.Ms > 300_000) player.Stop(null, "guard"); };
            player.Queue(M("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"eachAfterMs":50,"repeatUntilGoneMs":1000}]""", ""","maxMinutes":1"""), accounts);
            await Done(player);
            c.Check("click each patch with patches that keep coming ends at the run's longest time",
                player.Runs[0] is { Status: "stopped", Reason: "maxTime" } && desk.Ms < 70_000 && !InputTurn.Busy, $"{State(player.Runs[0])} {desk.Ms}");
        }
        {
            // follow on a bar that never goes, with a 10-minute timeout.
            var (player, desk, accounts) = Setup();
            desk.Area = area => Bars(area.Width, area.Height, (area.Width / 2, 3, Color.Lime), (2, 3, Color.White));
            desk.OnDelay = () => { if (desk.Ms > 300_000) player.Stop(null, "guard"); };
            player.Queue(M("""[{"kind":"follow","region":{"x":0,"y":0.5,"w":0.1,"h":0.1},"target":"#00ff00","marker":"#ffffff","timeoutMs":600000}]""", ""","maxMinutes":1"""), accounts);
            await Done(player);
            c.Check("follow ends at the run's longest time too, everything let go",
                player.Runs[0] is { Status: "stopped", Reason: "maxTime" } && desk.Ms < 70_000 && !InputTurn.Busy
                && desk.Log.Count(l => l.Contains("left down")) == desk.Log.Count(l => l.Contains("left up")), $"{State(player.Runs[0])} {desk.Ms}");
        }
        {
            // Anti-AFK asks for the PC while a follow waits on its bar for minutes.
            var (player, desk, accounts) = Setup();
            desk.Area = area => Bars(area.Width, area.Height, (area.Width / 2, 3, Color.Lime), (2, 3, Color.White));
            Task<IDisposable>? afk = null;
            int grantedAt = -1;
            desk.OnDelay = () =>
            {
                if (afk is null && desk.Ms >= 5000)
                    afk = InputTurn.EnterAsync("afk");
                if (grantedAt < 0 && afk is { IsCompletedSuccessfully: true })
                {
                    grantedAt = desk.Ms;
                    afk.Result.Dispose(); // the nudge done
                }
                if (desk.Ms > 300_000)
                    player.Stop(null, "guard");
            };
            player.Queue(M("""[{"kind":"follow","region":{"x":0,"y":0.5,"w":0.1,"h":0.1},"target":"#00ff00","marker":"#ffffff","timeoutMs":120000}]""", ""","maxMinutes":30"""), accounts);
            await Done(player);
            c.Check("anti-AFK asking during a long follow gets the PC within the turn's budget (30 s)", grantedAt is >= 5000 and < 40_000, grantedAt);
            InputTurn.Reset();
        }
        {
            // A key held for a minute (walking), and the person touches the PC a second in.
            var (player, desk, accounts) = Setup();
            desk.PersonAt = () => desk.Ms >= 1000 ? 1000 : null;
            desk.HoldNow = () => desk.Ms is >= 1000 and < 6000 ? "personActive" : null;
            var seen = new List<string>();
            bool busyWhilePaused = false;
            player.Changed = () => seen.Add(State(player.Runs[0]));
            desk.OnDelay = () =>
            {
                busyWhilePaused |= desk.Ms is > 2500 and < 5500 && InputTurn.Busy;
                if (desk.Ms > 4_000_000) player.Stop(null, "guard");
            };
            player.Queue(M("""[{"kind":"down","key":"W"},{"kind":"wait","ms":60000},{"kind":"up","key":"W"}]""", ""","maxMinutes":5"""), accounts);
            await Done(player);
            var downs = desk.Log.Where(l => l.Contains("key W down")).Select(At).ToList();
            var ups = desk.Log.Where(l => l.Contains("key W up")).Select(At).ToList();
            c.Check("a person touching the PC during a held wait: the key let go at once, the run paused, the PC free",
                ups.Count > 0 && ups[0] < 2000 && seen.Contains("paused/personActive") && !busyWhilePaused, $"{string.Join(" ", seen.Distinct())} busy={busyWhilePaused} | {string.Join(" | ", desk.Log)}");
            c.Check("…once they stop, the key is held again for the rest of the wait only",
                downs.Count == 2 && downs[1] >= 6000 && ups.Count == 2 && ups[1] - downs[1] is >= 58_000 and < 62_000 && player.Runs[0].Status == "done",
                $"{State(player.Runs[0])} | {string.Join(" | ", desk.Log)}");
        }
        {
            // A held wait at its longest time: an hour's wait in a 1-minute run.
            var (player, desk, accounts) = Setup();
            desk.OnDelay = () => { if (desk.Ms > 4_000_000) player.Stop(null, "guard"); };
            player.Queue(M("""[{"kind":"down","key":"W"},{"kind":"wait","ms":3600000},{"kind":"up","key":"W"}]""", ""","maxMinutes":1"""), accounts);
            await Done(player);
            var up = desk.Log.FirstOrDefault(l => l.Contains("key W up"));
            c.Check("a held wait stops at the run's longest time, the key let go then", player.Runs[0] is { Status: "stopped", Reason: "maxTime" }
                && up is not null && At(up) < 61_000 && !InputTurn.Busy, $"{State(player.Runs[0])} {up}");
        }
        {
            // Paused between holding a key and the wait: the next turn holds it again before the wait counts.
            var (player, desk, accounts) = Setup();
            desk.PersonAt = () => desk.Ms >= 10 ? 10 : null;
            desk.HoldNow = () => desk.Ms is >= 10 and < 5010 ? "personActive" : null;
            desk.OnDelay = () => { if (desk.Ms > 4_000_000) player.Stop(null, "guard"); };
            player.Queue(M("""[{"kind":"down","key":"W"},{"kind":"wait","ms":3000},{"kind":"up","key":"W"}]""", ""","maxMinutes":5"""), accounts);
            await Done(player);
            var downs = desk.Log.Where(l => l.Contains("key W down")).Select(At).ToList();
            var ups = desk.Log.Where(l => l.Contains("key W up")).Select(At).ToList();
            c.Check("paused right after a key is held: it is held again when the run goes on, for the whole wait",
                downs.Count == 2 && downs[1] >= 5010 && ups.Count == 2 && ups[1] - downs[1] >= 3000 && player.Runs[0].Status == "done", string.Join(" | ", desk.Log));
        }
        {
            // The key setting changed on the panel while its key is held.
            var (player, desk, accounts) = Setup();
            bool changed = false;
            desk.OnDelay = () =>
            {
                if (!changed && desk.Log.Any(l => l.Contains("key W down")))
                {
                    changed = true;
                    player.Runs[0].Macro.Options[0].Value = J("E");
                }
            };
            player.Queue(M("""[{"kind":"down","key":"$o1"},{"kind":"wait","ms":100},{"kind":"up","key":"$o1"},{"kind":"wait","ms":5000},{"kind":"key","key":"Q"}]""",
                ""","options":[{"name":"o1","label":"Key","kind":"key","value":"W"}]"""), accounts);
            await Done(player);
            int wUp = desk.Log.FindIndex(l => l.Contains("key W up")), q = desk.Log.FindIndex(l => l.Contains("key Q down"));
            c.Check("a key setting changed while its key is held: Release lets go of the key its Hold pressed", wUp >= 0 && wUp < q
                && !desk.Log.Any(l => l.Contains("key E")) && player.Runs[0].Status == "done", string.Join(" | ", desk.Log));
            c.Check("Hold one setting's key and Release another's is refused", CheckOf("""[{"kind":"down","key":"$o1"},{"kind":"up","key":"$o2"}]""",
                ""","options":[{"name":"o1","label":"A","kind":"key","value":"W"},{"name":"o2","label":"B","kind":"key","value":"W"}]""") is not null);
            c.Check("…the same setting's key held and let go passes", CheckOf("""[{"kind":"down","key":"$o1"},{"kind":"up","key":"$o1"}]""",
                ""","options":[{"name":"o1","label":"A","kind":"key","value":"W"}]""") is null);
        }
        {
            // A Press step whose key setting holds a mouse button.
            var (player, desk, accounts) = Setup();
            player.Queue(M("""[{"kind":"key","key":"$o1"}]""", ""","options":[{"name":"o1","label":"Key","kind":"key","value":"MouseLeft"}]"""), accounts);
            await Done(player);
            c.Check("a Press step given a mouse button stops with a reason (missingValue)", player.Runs[0] is { Status: "failed", Reason: "missingValue" }, State(player.Runs[0]));
        }
        {
            // Giving the window back fails: the PC's turn is handed on all the same.
            var (player, desk, accounts) = Setup();
            desk.ThrowOnEnd = true;
            player.Queue(M("""[{"kind":"key","key":"E"}]"""), accounts);
            await Done(player);
            c.Check("a turn whose end fails still hands the PC on", !InputTurn.Busy && player.Runs[0].Finished, $"{State(player.Runs[0])} holder={InputTurn.Holder}");
            InputTurn.Reset();
        }
        {
            // The account's window replaced (a rejoin, a crash) between two turns.
            var (player, desk, accounts) = Setup();
            desk.OnDelay = () =>
            {
                if (desk.Ms > 1000 && desk.Windows[accounts[0]].Pid == 1)
                    desk.Windows[accounts[0]] = desk.Windows[accounts[0]] with { Pid = 7 };
            };
            player.Queue(M("""[{"kind":"key","key":"A"},{"kind":"wait","ms":3000},{"kind":"key","key":"B"}]"""), accounts);
            await Done(player);
            c.Check("a run whose window was replaced starts over from the top in the new one", desk.Log.Any(l => l.StartsWith("7 key A down"))
                && player.Runs[0].Status == "done", string.Join(" | ", desk.Log));
        }
        {
            // Run (the saved macro) while the editor's Try it plays it on the same account.
            var (player, desk, accounts) = Setup();
            var m = M("""[{"kind":"wait","ms":5000}]""");
            var hold = InputTurn.TryEnter("afk")!;
            player.Queue(m, accounts, trial: true);
            var tried = player.Runs[0];
            int queued = player.Queue(m, accounts);
            hold.Dispose();
            await Done(player);
            var real = player.Runs.FirstOrDefault(r => !r.Trial);
            c.Check("Run while Try it plays the macro there: the saved macro runs, Try it stops", queued == 1 && real?.Status == "done"
                && tried.Status == "stopped" && player.Runs.Count(r => r.Macro.Id == m.Id) == 1, $"{queued} {real?.Status} {tried.Status}");
            InputTurn.Reset();
        }
        {
            // Bloxkeep closing while a key is held and another pressed: nothing will run again on this thread.
            var (player, desk, accounts) = Setup();
            desk.SuspendWhen = () => desk.Log.Any(l => l.Contains("key E down"));
            player.Queue(M("""[{"kind":"down","key":"W"},{"kind":"wait","ms":500},{"kind":"key","key":"E","ms":5000},{"kind":"up","key":"W"}]"""), accounts);
            player.ReleaseAll("closing");
            c.Check("closing mid-turn: every key held or pressed is let go at once, the PC given back", desk.Log.Any(l => l.Contains("key W up"))
                && desk.Log.Any(l => l.Contains("key E up")) && desk.Log.Any(l => l.StartsWith("abort")) && player.Runs[0] is { Status: "stopped", Reason: "closing" },
                $"{State(player.Runs[0])} | {string.Join(" | ", desk.Log)}");
            InputTurn.Reset();
        }
        {
            // The real turn's own list of what is down: let go, forgotten; still down, owed at the close, newest first.
            var held = new HeldInputs<string>();
            held.Note("key W", up: false, "W up");
            held.Note("button left", up: false, "left up");
            held.Note("key E", up: false, "E up");
            held.Note("key E", up: true, "E up");
            held.Note("key W", up: false, "W up");
            var ups = held.Ups();
            c.Check("closing: the real turn lets go of what is still down, once each", string.Join(",", ups) == "W up,left up" && held.Ups().Length == 0,
                string.Join(",", ups));
        }
        {
            // Without working raw input, a turn still notices the person (input Windows got well after our own).
            var t0 = new DateTime(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
            c.Check("raw input not working: a person's input is seen from Windows' last input",
                MacroDesk.PersonSince(t0, () => null, () => false, lastInputTick: 10_000, lastSent: 5_000)
                && !MacroDesk.PersonSince(t0, () => null, () => false, lastInputTick: 5_050, lastSent: 5_000));
            c.Check("raw input working: only a device's input counts", !MacroDesk.PersonSince(t0, () => null, () => true, 10_000, 5_000)
                && MacroDesk.PersonSince(t0, () => t0.AddSeconds(1), () => true, 5_000, 5_000) && !MacroDesk.PersonSince(t0, () => t0.AddSeconds(-1), () => true, 5_000, 5_000));
        }
        InputTurn.Reset();
    }

    // ───────────────────────── the panel: settings, areas, on the game ─────────────────────────

    static void Panels(Checks c)
    {
        const string Step = """[{"kind":"wait","ms":"$o1"}]""";
        string With(string options, string areas = "[]", string panel = """{"autoShow":false,"placeId":null}""") =>
            $$""","options":{{options}},"areas":{{areas}},"panel":{{panel}}""";
        var good = Read(Step, With("""
            [
              {"name":"o1","label":"  Hold time ","kind":"number","value":99999,"default":500,"min":0,"max":10000,"unit":"s"},
              {"name":"o2","label":"Bar colour","kind":"colour","value":"#ABCDEF","default":"nope"},
              {"name":"o3","label":"Auto sell","kind":"check","value":true},
              {"name":"o4","label":"Key","kind":"key","value":"space","default":"E"},
              {"name":"o5","label":"Rod","kind":"choice","choices":["Wood","Gold"],"value":"gold","default":"Iron"},
              {"name":"o6","label":"Strength","kind":"number","value":"2.5","min":5,"max":0,"unit":"furlongs"}
            ]
            """, """[{"name":"a1","label":"Fish area","colour":"#3B82F6","rect":{"x":0.3,"y":0.7,"w":0.4,"h":0.1} }]""",
            """{"autoShow":true,"placeId":-5}"""));
        string? error = Refused(() => MacroRules.Check(good, new List<Guid>(), []));
        c.Check("a macro with every kind of setting passes", error is null, error);
        var o = good.Options;
        c.Check("values are kept to their kind (a number to its range, a colour in small letters, a check as 1, a key and a choice as the list spells them)",
            o[0].Value?.GetDouble() == 10000 && o[0].Label == "Hold time" && o[1].Value?.GetString() == "#abcdef" && o[2].Value?.GetDouble() == 1
            && o[3].Value?.GetString() == "Space" && o[4].Value?.GetString() == "Gold", JsonSerializer.Serialize(o, MacroRules.PageJson));
        c.Check("a default that doesn't fit becomes a plain one (white, the first choice)", o[1].Default?.GetString() == "#ffffff" && o[4].Default?.GetString() == "Wood"
            && o[2].Default?.GetDouble() == 0);
        c.Check("a range given backwards is turned round; an unknown unit dropped", o[5].Min == 0 && o[5].Max == 5 && o[5].Unit == "" && o[5].Value?.GetDouble() == 2.5);
        c.Check("only a number keeps a range and unit, only a choice its choices", o[1].Min is null && o[1].Unit is null && o[0].Choices is null);
        c.Check("an area inside the window is kept, colour in small letters; a place below 0 is no game", good.Areas[0].Colour == "#3b82f6" && good.Panel.PlaceId is null && good.Panel.AutoShow);
        string? Opt(string options, string areas = "[]") => CheckOf(Step, With(options, areas));
        var tabbed = Read(Step, With("""
            [
              {"name":"o1","label":"Wait","kind":"number","value":5,"tab":"fishing"},
              {"name":"o2","label":"Water","kind":"point","value":{"x":1.5,"y":"x"}},
              {"name":"o3","label":"Key","kind":"key","value":"E","tab":"gone"}
            ]
            """, "[]", """{"autoShow":false,"placeId":null,"tabs":[" Casting ","Fishing"]}"""));
        error = Refused(() => MacroRules.Check(tabbed, new List<Guid>(), []));
        c.Check("tabs: names trimmed, a setting's tab spelled as the panel's; an unknown one goes to the first", error is null
            && tabbed.Panel.Tabs.SequenceEqual(["Casting", "Fishing"]) && tabbed.Options[0].Tab == "Fishing" && tabbed.Options[2].Tab is null, error);
        c.Check("a point that isn't one is put in the middle", tabbed.Options[1].Value is { } pt && pt.GetProperty("x").GetDouble() == 0.5
            && MacroOptions.Show(tabbed.Options[1], pt) == "50%, 50%", tabbed.Options[1].Value?.GetRawText());
        var oneTab = Read(Step, With("""[{"name":"o1","label":"Wait","kind":"number","value":5,"tab":"Only"}]""", "[]", """{"autoShow":false,"placeId":null,"tabs":["Only"]}"""));
        MacroRules.Check(oneTab, new List<Guid>(), []);
        var headed = Read(Step, With("""[{"name":"o1","label":"Bait","kind":"heading","value":5}]"""));
        MacroRules.Check(headed, new List<Guid>(), []);
        c.Check("a heading has no value", headed.Options[0] is { Value: null, Default: null });
        var listed = Read(Step, With("""[{"name":"o1","label":"Want","kind":"list","choices":["Abyssal","Blessed"],"value":["blessed","nope","abyssal"]}]"""));
        MacroRules.Check(listed, new List<Guid>(), []);
        c.Check("a list keeps its ticked words, spelled and ordered as the list", listed.Options[0].Value?.GetRawText() == """["Abyssal","Blessed"]"""
            && MacroOptions.Show(listed.Options[0], listed.Options[0].Value!.Value) == "2 of 2", listed.Options[0].Value?.GetRawText());
        c.Check("a list needs its words", Opt("""[{"name":"o1","label":"x","kind":"list","choices":[]}]""") is not null);
        c.Check("click each: after 0 to 10 s", CheckOf("""[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"eachAfterMs":20000}]""") is not null);
        c.Check("one tab is no tabs",oneTab.Panel.Tabs.Count == 0 && oneTab.Options[0].Tab is null);
        c.Check("two tabs of the same name are refused", Opt("[]") is null
            && CheckOf(Step, With("[]", "[]", """{"autoShow":false,"placeId":null,"tabs":["A","a"]}""")) is not null);
        c.Check("a setting needs a label of 1 to 30 characters", Opt("""[{"name":"o1","label":" ","kind":"number"}]""") is not null
            && Opt($$"""[{"name":"o1","label":"{{new string('a', 31)}}","kind":"number"}]""") is not null);
        c.Check("an unknown kind of setting is refused", Opt("""[{"name":"o1","label":"x","kind":"slider"}]""") is not null);
        c.Check("two settings of one name are refused", Opt("""[{"name":"o1","label":"x","kind":"check"},{"name":"o1","label":"y","kind":"check"}]""") is not null);
        c.Check("a choice needs its choices, each once", Opt("""[{"name":"o1","label":"x","kind":"choice","choices":[]}]""") is not null
            && Opt("""[{"name":"o1","label":"x","kind":"choice","choices":["a","A"]}]""") is not null);
        string many = "[" + string.Join(",", Enumerable.Range(1, 31).Select(i => $$"""{"name":"o{{i}}","label":"x","kind":"check"}""")) + "]";
        c.Check("30 settings at most, 8 areas at most", Opt(many) is not null
            && Opt("[]", "[" + string.Join(",", Enumerable.Range(1, 9).Select(i => $$"""{"name":"a{{i}}","label":"x","rect":{"x":0,"y":0,"w":0.1,"h":0.1} }""")) + "]") is not null);
        c.Check("an area outside the window is refused", Opt("[]", """[{"name":"a1","label":"x","rect":{"x":0.9,"y":0,"w":0.5,"h":0.1} }]""") is not null);
        c.Check("a new macro comes with an empty id", Read("""[{"kind":"wait","ms":1}]""", ""","id":"" """).Id == Guid.Empty);
        var old = JsonSerializer.Deserialize<Macro>("""{"Name":"Old","Steps":[{"Kind":"wait","Ms":5}]}""")!;
        c.Check("a macro saved before settings existed has none, and a panel that doesn't show by itself", old.Options.Count == 0 && old.Areas.Count == 0 && !old.Panel.AutoShow);

        // As the panel shows and takes them.
        var hold = o[0];
        c.Check("seconds are shown in seconds and typed in seconds", MacroOptions.Show(hold, J(1500)) == "1.5 s" && MacroOptions.TypedNumber(hold, "2,5 s") == 2500
            && MacroOptions.TypedNumber(hold, "lots") is null, MacroOptions.Show(hold, J(1500)));
        c.Check("− and + move a number by a step, kept in its range", MacroOptions.Nudge(hold, 1).GetDouble() == 10000 && MacroOptions.Step(hold) == 100
            && MacroOptions.Nudge(new MacroOption { Kind = "number", Value = J(0.3), Min = 0, Max = 10 }, -1).GetDouble() == 0.2);
        var part = new MacroOption { Kind = "number", Min = 0.5, Max = 1, Unit = "%" };
        c.Check("a part of 1 shown as percent", MacroOptions.Show(part, J(0.9)) == "90%" && MacroOptions.TypedNumber(part, "80%") == 0.8);
        c.Check("a switch shows On or Off", MacroOptions.Show(o[2], J(1)) == L.T("On") && MacroOptions.Show(o[2], J(0)) == L.T("Off"));
        var language = Prefs.Language;
        try
        {
            Prefs.Language = "vi";
            c.Check("in Vietnamese a number has a comma and its unit in words, and is typed either way",
                MacroOptions.Show(hold, J(1500)) == "1,5 giây" && MacroOptions.TypedNumber(hold, "2,5 giây") == 2500
                && MacroOptions.TypedNumber(hold, "0.8") == 800, MacroOptions.Show(hold, J(1500)));
        }
        finally { Prefs.Language = language; }
        c.Check("a setting changed from the panel or the page is kept to its kind", MacroOptions.Normalize(o[4], J("WOOD"))?.GetString() == "Wood"
            && MacroOptions.Normalize(o[4], J("Iron")) is null && MacroOptions.Normalize(o[2], J(false))?.GetDouble() == 0 && MacroOptions.Normalize(o[1], J("red")) is null);
        var counted = Read("""[{"kind":"setVar","name":"fish","value":0},{"kind":"addVar","name":"fish","value":1},{"kind":"addVar","name":"coins","value":5}]""");
        c.Check("its counters: every variable it sets or adds to, once", string.Join(",", MacroOptions.Counters(counted)) == "fish,coins");

        // A file carries the settings and areas.
        var file = JsonSerializer.Serialize(MacroRules.Copy(good), new JsonSerializerOptions(MacroRules.PageJson) { WriteIndented = true });
        using (var doc = JsonDocument.Parse(file))
        {
            var back = MacroRules.Read(doc.RootElement);
            c.Check("a .bloxkeepmacro file carries its settings, areas and panel", back.Options.Count == 6 && back.Options[4].Value?.GetString() == "Gold"
                && back.Areas[0].Label == "Fish area" && back.Panel.AutoShow && Refused(() => MacroRules.Check(back, new List<Guid>(), [])) is null);
        }

        // On the game: dots and boxes.
        var client = new Rectangle(100, 50, 800, 600);
        var r = new MacroRect { X = 0.25, Y = 0.25, W = 0.5, H = 0.5 };
        var screen = AreaGeometry.ToScreen(r, client);
        var back2 = AreaGeometry.ToFraction(screen, client);
        c.Check("an area in screen pixels and back", screen == new Rectangle(300, 200, 400, 300) && back2.X == 0.25 && back2.W == 0.5);
        c.Check("a press takes a corner, the inside, or nothing", AreaGeometry.HitTest(screen, new Point(303, 197), 8) == AreaGeometry.Grip.TopLeft
            && AreaGeometry.HitTest(screen, new Point(698, 502), 8) == AreaGeometry.Grip.BottomRight && AreaGeometry.HitTest(screen, new Point(500, 300), 8) == AreaGeometry.Grip.Move
            && AreaGeometry.HitTest(screen, new Point(50, 50), 8) == AreaGeometry.Grip.None);
        c.Check("moved, it stays inside the window", AreaGeometry.Drag(screen, AreaGeometry.Grip.Move, 1000, -1000, client, 12) == new Rectangle(500, 50, 400, 300));
        c.Check("sized, it keeps a few pixels", AreaGeometry.Drag(screen, AreaGeometry.Grip.BottomRight, -1000, -1000, client, 12) == new Rectangle(300, 200, 12, 12)
            && AreaGeometry.Drag(screen, AreaGeometry.Grip.TopLeft, -1000, 0, client, 12) == Rectangle.FromLTRB(100, 200, 700, 500));
        c.Check("an area outside the window falls back to all of it", AreaGeometry.ToFraction(new Rectangle(0, 0, 10, 10), client) is { X: 0, W: 1 });
        var pointed = Read("""
            [{"kind":"click","x":0.5,"y":0.5,"holdMs":500},{"kind":"key","key":"E"},{"kind":"move","x":0.1,"y":0.9},
             {"kind":"waitColour","x":0.2,"y":0.2,"colour":"#000000"},{"kind":"waitColour","colour":"#000000","region":"@a1"},{"kind":"click","x":"$o1","y":0.5}]
            """);
        var dots = AreaGeometry.Dots(pointed);
        c.Check("the dots: click, hold, move and a colour at a place, each with its step; not an area's step nor a setting's place",
            string.Join(" ", dots.Select(d => $"{d.Step + 1}@{d.X},{d.Y}")) == "1@0.5,0.5 3@0.1,0.9 4@0.2,0.2", string.Join(" ", dots));
        var at = AreaGeometry.ToScreen(0.5, 0.5, client);
        c.Check("a dot on the screen, taken by a press near it, and back", at == new Point(500, 350) && AreaGeometry.HitDot(dots, new Point(507, 356), client, 12) == 0
            && AreaGeometry.HitDot(dots, new Point(530, 350), client, 12) == -1 && AreaGeometry.ToFraction(new Point(140, 590), client) == (0.05, 0.9)
            && AreaGeometry.ToFraction(new Point(5000, -20), client) == (1, 0));

        // Shown by itself while its game is played.
        var fish = new Macro { Id = Guid.NewGuid(), Name = "f", Panel = new MacroPanel { AutoShow = true, PlaceId = 123 } };
        var never = new Macro { Id = Guid.NewGuid(), Name = "n", Panel = new MacroPanel { AutoShow = false, PlaceId = 123 } };
        var auto = new HashSet<Guid>();
        var closed = new HashSet<Guid>();
        var visible = new HashSet<Guid>();
        (List<Guid> Show, List<Guid> Hide) Tick(params long[] places)
        {
            var result = MacroOptions.AutoShow([fish, never], places.ToHashSet(), auto, closed, visible.Contains);
            foreach (var id in result.Show)
                visible.Add(id);
            foreach (var id in result.Hide)
                visible.Remove(id);
            return result;
        }
        var t1 = Tick(123);
        var t2 = Tick(123);
        c.Check("a panel shows as its game starts, once; one without autoShow never", t1.Show.SequenceEqual([fish.Id]) && t2.Show.Count == 0);
        var t3 = Tick();
        c.Check("and hides as the game ends", t3.Hide.SequenceEqual([fish.Id]));
        Tick(123);
        visible.Remove(fish.Id);
        auto.Remove(fish.Id);
        closed.Add(fish.Id); // the person closed it
        var t4 = Tick(123);
        c.Check("closed by the person, it stays closed while the game goes on", t4.Show.Count == 0);
        Tick();
        var t5 = Tick(123);
        c.Check("and shows again next time the game is played", t5.Show.SequenceEqual([fish.Id]));

        // "Any game" (no place): only a macro with a window in the game played, and never more than three at once.
        var anyGames = Enumerable.Range(0, 5).Select(i => new Macro { Id = Guid.NewGuid(), Name = $"a{i}", Panel = new MacroPanel { AutoShow = true, PlaceId = null } }).ToList();
        var mine = anyGames.Take(4).Select(m => m.Id).ToHashSet();
        var auto2 = new HashSet<Guid>();
        var none = MacroOptions.AutoShow(anyGames, new HashSet<long>(), auto2, new HashSet<Guid>(), _ => false, m => mine.Contains(m.Id), 3);
        var some = MacroOptions.AutoShow(anyGames, new HashSet<long> { 99 }, auto2, new HashSet<Guid>(), _ => false, m => mine.Contains(m.Id), 3);
        c.Check("a panel for any game shows only while a game is played in one of its accounts, three at most",
            none.Show.Count == 0 && some.Show.Count == 3 && some.Show.All(mine.Contains) && !some.Show.Contains(anyGames[4].Id), string.Join(",", some.Show));
        var gone = MacroOptions.AutoShow(anyGames, new HashSet<long>(), auto2, new HashSet<Guid>(), _ => true, m => mine.Contains(m.Id), 3);
        c.Check("and they hide as the games end", gone.Hide.Count == 3 && auto2.Count == 0);
    }

    // ───────────────────────── templates and "What it sees" ─────────────────────────

    static void Templates(Checks c)
    {
        var all = MacroTemplates.Build();
        c.Check("five templates: keys, click, cast, rejoin, blank", string.Join(",", all.Select(t => t.Id)) == "keys,click,cast,rejoin,blank");
        c.Check("a blank template has no steps", all[4].Macro.Steps.Count == 0);
        foreach (var t in all.Take(4))
        {
            var m = MacroRules.Copy(t.Macro);
            string? error = Refused(() => MacroRules.Check(m, new List<Guid>(), []));
            var own = m.Options.Select(o => o.Name).Concat(m.Areas.Select(a => "@" + a.Name)).ToHashSet();
            var missing = MacroValues.Names(m).Where(n => !own.Contains(n)).ToList();
            c.Check($"template {t.Id}: it passes, and every setting and area it names is its own", error is null && missing.Count == 0, error ?? string.Join(",", missing));
        }
        string json = JsonSerializer.Serialize(MacroTemplates.All(), new JsonSerializerOptions(JsonSerializerDefaults.Web));
        using var doc = JsonDocument.Parse(json);
        var page = doc.RootElement;
        c.Check("macroTemplates: { macros: Macro & {about, icon} } and nothing else",
            page.EnumerateObject().Select(p => p.Name).SequenceEqual(["macros"]) && page.GetProperty("macros").GetArrayLength() == 5
            && page.GetProperty("macros")[0].GetProperty("about").GetString()!.Length > 0 && page.GetProperty("macros")[0].GetProperty("icon").GetString() == "keyboard"
            && page.GetProperty("macros")[0].GetProperty("id").GetString() == "", json[..Math.Min(400, json.Length)]);
        c.Check("each comes with its settings and areas", page.GetProperty("macros")[2].GetProperty("options").GetArrayLength() == 5
            && page.GetProperty("macros")[2].GetProperty("areas")[0].GetProperty("label").GetString()!.Length > 0
            && page.GetProperty("macros")[1].GetProperty("steps")[1].GetProperty("region").GetString() == "@a1");
        // As the page uses one: read back from the page and saved.
        string? flow = null;
        foreach (var t in page.GetProperty("macros").EnumerateArray().Take(4))
        {
            var m = MacroRules.Read(t);
            flow ??= Refused(() => MacroRules.Check(m, new List<Guid>(), []));
        }
        c.Check("a template read back from the page passes", flow is null, flow);
        var cast = MacroRules.Read(page.GetProperty("macros")[2]);
        MacroRules.Check(cast, new List<Guid>(), []);
        c.Check("a step's to-do comes with it and stays once saved", cast.Id == Guid.Empty
            && JsonSerializer.Serialize(cast, MacroRules.PageJson).Contains("\"todo\":["), JsonSerializer.Serialize(cast.Steps[2], MacroRules.PageJson));
        c.Check("its point to set on the game is a dot (step 2)", AreaGeometry.Dots(cast).Select(d => d.Step).SequenceEqual([1]));
        // What to set is named by field (the editor's checklist says it in words); each step says why it is there.
        c.Check("a to-do names fields, not sentences", JsonSerializer.Serialize(cast.Steps[2], MacroRules.PageJson).Contains("\"todo\":[\"region\",\"target\",\"marker\"]"),
            JsonSerializer.Serialize(cast.Steps[2], MacroRules.PageJson));
        c.Check("a template's steps carry their notes, kept once saved", JsonSerializer.Serialize(cast.Steps[1], MacroRules.PageJson).Contains("\"note\":"));
        var rejoin = MacroRules.Read(page.GetProperty("macros")[3]);
        MacroRules.Check(rejoin, new List<Guid>(), []);
        c.Check("the rejoin template starts by itself after a rejoin, on all accounts", rejoin.Trigger.Kind == "rejoin" && rejoin.Targets == "all");
        // A note: plain text up to 80, trimmed; anything else goes.
        var noted = MacroRules.Read(JsonDocument.Parse($$"""{"name":"n","steps":[{"kind":"wait","ms":10,"note":"  {{new string('a', 90)}} "},{"kind":"wait","ms":10,"note":5},{"kind":"wait","ms":10,"note":"  "}]}""").RootElement);
        MacroRules.Check(noted, new List<Guid>(), []);
        string notes = JsonSerializer.Serialize(noted.Steps, MacroRules.PageJson);
        c.Check("a note is cut to 80 and dropped when it isn't text", notes.Contains("\"note\":\"" + new string('a', 80) + "\"") && notes.Split("\"note\"").Length == 2, notes);
        // The panel's names for counters: a counter's name to its label, nothing else.
        var labelled = MacroRules.Read(JsonDocument.Parse("""{"name":"n","steps":[{"kind":"addVar","name":"fish","value":1}],"panel":{"autoShow":false,"placeId":null,"counterLabels":{"fish":" Fish caught ","9bad":"x","empty":"","n":3}}}""").RootElement);
        MacroRules.Check(labelled, new List<Guid>(), []);
        string panel = JsonSerializer.Serialize(labelled.Panel, MacroRules.PageJson);
        c.Check("counter labels keep only a counter's name with its text", panel.Contains("\"counterLabels\":{\"fish\":\"Fish caught\"}"), panel);
        c.Check("no game's name in them", !System.Text.RegularExpressions.Regex.IsMatch(json, "Fisch|Blox Fruits|Pet Simulator|Adopt Me", System.Text.RegularExpressions.RegexOptions.IgnoreCase));
    }

    static void Watch(Checks c)
    {
        var m = M("""[{"kind":"follow","region":{"x":0,"y":0,"w":1,"h":1},"target":"#00ff00","marker":"$o1"},{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1}}]""");
        using var picture = Bars(200, 50, (150, 2, Color.Lime), (50, 2, Color.White));
        var seen = MacroWatch.Look(picture, m, n => n == "o1" ? J("#ffffff") : null);
        var target = seen.Boxes.FirstOrDefault(b => b.Kind == "target");
        var marker = seen.Boxes.FirstOrDefault(b => b.Kind == "marker");
        c.Check("What it sees: the area, the target and the marker in their places, and how far apart",
            seen.Boxes.Any(b => b.Kind == "area") && target is not null && Math.Abs(target.X - 0.745) < 0.01 && marker is not null && Math.Abs(marker.X - 0.245) < 0.01
            && seen.Numbers.Any(n => n.Value == 100), JsonSerializer.Serialize(seen));
        c.Check("and the patches clickColour would click", seen.Boxes.Any(b => b.Kind == "colour") && seen.Numbers.Any(n => n.Label.Contains("patches") && n.Value == 1));
        var missing = MacroWatch.Look(picture, m, _ => null);
        c.Check("a step whose setting is missing is left out", !missing.Boxes.Any(b => b.Kind == "marker"));
        using var small = MacroWatch.Scaled(new Bitmap(1280, 720), 640);
        c.Check("its picture is made small for the page", small.Width == 640 && small.Height == 360);
    }
}
