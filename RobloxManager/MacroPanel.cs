using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace RobloxManager;

// Macros (Beta): what the player may change on a macro's panel (PanelForm.cs) and the boxes it looks in on the game.
// The maker ticks "Player can change" on a step's value: the value becomes an option ("$o1" in the step), shown on the
// panel; an area is a named box ("@a1" in the step), moved and sized on the game itself (GameOverlay.cs). Kept in the
// macro; the shape is the page's and the core's shared contract (macro-contract.md, "REWORK").

/// <summary>A value the player may change on the macro's panel.</summary>
public sealed class MacroOption
{
    /// <summary>Made by the page ("o1", "o2"…), never shown; steps name it as "$o1".</summary>
    public string Name { get; set; } = "";
    public string Label { get; set; } = "";
    /// <summary>"number", "colour", "check" (1/0), "key", "choice", "point" ({x, y}, parts of the window; a step's x
    /// and y both name it), "list" (the player ticks some of <see cref="Choices"/>; a step reading text takes any of them)
    /// or "heading" (a title over the settings below it, no value).</summary>
    public string Kind { get; set; } = "number";
    /// <summary>The panel's tab it shows on (one of <see cref="MacroPanel.Tabs"/>); null: the first.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Tab { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public JsonElement? Value { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public JsonElement? Default { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Min { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Max { get; set; }
    /// <summary>A number's unit: "ms", "s" (kept in ms like the step's field, shown in seconds), "px", "%" or "".</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Unit { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public List<string>? Choices { get; set; }
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>A named box on the game, as parts (0..1) of the window's picture area.</summary>
public sealed class MacroArea
{
    /// <summary>Made by the page ("a1", "a2"…); steps name it as "@a1".</summary>
    public string Name { get; set; } = "";
    public string Label { get; set; } = "";
    public string Colour { get; set; } = "#22c55e";
    public MacroRect Rect { get; set; } = new() { X = 0.25, Y = 0.25, W = 0.5, H = 0.5 };
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>The macro's panel: shown by itself while an account plays <see cref="PlaceId"/>.</summary>
public sealed class MacroPanel
{
    public bool AutoShow { get; set; }
    public long? PlaceId { get; set; }
    /// <summary>Its tabs' names, in order; fewer than two: no tabs.</summary>
    public List<string> Tabs { get; set; } = new();
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>A macro's options and areas: their checks (each refusal a sentence for the person), their values as the
/// panel shows and takes them, and when a panel shows by itself.</summary>
static class MacroOptions
{
    public const int MaxOptions = 30, MaxAreas = 8, MaxLabel = 30, MaxChoices = 60, MaxTabs = 6, MaxTab = 20;
    static readonly string[] Kinds = ["number", "colour", "check", "key", "choice", "point", "list", "heading"];
    static readonly string[] Units = ["ms", "s", "px", "%", ""];
    static readonly Regex ColourText = new("^#[0-9a-fA-F]{6}$", RegexOptions.CultureInvariant);
    public const string AreaColour = "#22c55e";

    /// <summary>Checks the macro's options, areas and panel, and fills in defaults; a value that no longer fits goes back
    /// to its default.</summary>
    public static void Check(Macro m)
    {
        m.Options ??= new();
        m.Areas ??= new();
        m.Panel ??= new();
        if (m.Options.Count > MaxOptions)
            throw new UserError(L.F("A macro can have up to {0} settings for the player.", MaxOptions));
        var names = new HashSet<string>(StringComparer.Ordinal);
        foreach (var o in m.Options)
        {
            if (o is null)
                throw new UserError(L.T("A setting for the player is empty."));
            CheckOption(o);
            if (!names.Add(o.Name))
                throw new UserError(L.T("Two settings for the player have the same name."));
        }
        if (m.Areas.Count > MaxAreas)
            throw new UserError(L.F("A macro can have up to {0} areas.", MaxAreas));
        names.Clear();
        foreach (var a in m.Areas)
        {
            if (a is null)
                throw new UserError(L.T("An area is empty."));
            CheckArea(a);
            if (!names.Add(a.Name))
                throw new UserError(L.T("Two areas have the same name."));
        }
        if (m.Panel.PlaceId is <= 0)
            m.Panel.PlaceId = null;
        m.Panel.Tabs = (m.Panel.Tabs ?? new()).Select(t => (t ?? "").Trim()).ToList();
        if (m.Panel.Tabs.Count > MaxTabs)
            throw new UserError(L.F("A panel can have up to {0} tabs.", MaxTabs));
        if (m.Panel.Tabs.Any(t => t.Length is 0 or > MaxTab))
            throw new UserError(L.F("A tab's name is 1 to {0} characters.", MaxTab));
        if (m.Panel.Tabs.Distinct(StringComparer.OrdinalIgnoreCase).Count() != m.Panel.Tabs.Count)
            throw new UserError(L.T("Two tabs have the same name."));
        if (m.Panel.Tabs.Count < 2)
            m.Panel.Tabs.Clear();
        foreach (var o in m.Options)
            o.Tab = m.Panel.Tabs.FirstOrDefault(t => t.Equals(o.Tab?.Trim(), StringComparison.OrdinalIgnoreCase));
    }

    /// <summary>The settings on a tab of the panel, in order (all of them when it has no tabs).</summary>
    public static List<MacroOption> OnTab(Macro m, string? tab) => m.Panel.Tabs.Count < 2 ? m.Options
        : m.Options.Where(o => (o.Tab ?? m.Panel.Tabs[0]) == (tab ?? m.Panel.Tabs[0])).ToList();

    static void CheckOption(MacroOption o)
    {
        if (o.Name is null || !MacroValues.Name.IsMatch(o.Name))
            throw new UserError(L.T("A setting for the player has no name."));
        o.Label = (o.Label ?? "").Trim();
        if (o.Label.Length is 0 or > MaxLabel)
            throw new UserError(L.F("A setting's label is 1 to {0} characters.", MaxLabel));
        if (!Kinds.Contains(o.Kind))
            throw new UserError(L.T("Unknown kind of setting."));
        if (o.Kind == "number")
        {
            if (o.Min is { } min && !double.IsFinite(min))
                o.Min = null;
            if (o.Max is { } max && !double.IsFinite(max))
                o.Max = null;
            if (o.Min > o.Max)
                (o.Min, o.Max) = (o.Max, o.Min);
            o.Unit ??= "";
            if (!Units.Contains(o.Unit))
                o.Unit = "";
        }
        else
        {
            o.Min = o.Max = null;
            o.Unit = null;
        }
        if (o.Kind is "choice" or "list")
        {
            o.Choices = (o.Choices ?? new()).Select(c => (c ?? "").Trim()).ToList();
            if (o.Choices.Count is < 1 or > MaxChoices)
                throw new UserError(L.F("A choice has 1 to {0} options.", MaxChoices));
            if (o.Choices.Any(c => c.Length is 0 or > MaxLabel))
                throw new UserError(L.F("An option is 1 to {0} characters.", MaxLabel));
            if (o.Choices.Distinct(StringComparer.OrdinalIgnoreCase).Count() != o.Choices.Count)
                throw new UserError(L.T("Two options are the same."));
        }
        else
            o.Choices = null;
        if (o.Kind == "heading")
        {
            o.Default = o.Value = null;
            return;
        }
        o.Default = o.Default is { } d && Normalize(o, d) is { } fits ? fits : Plain(o);
        o.Value = o.Value is { } v && Normalize(o, v) is { } now ? now : o.Default;
    }

    static void CheckArea(MacroArea a)
    {
        if (a.Name is null || !MacroValues.Name.IsMatch(a.Name))
            throw new UserError(L.T("An area has no name."));
        a.Label = (a.Label ?? "").Trim();
        if (a.Label.Length is 0 or > MaxLabel)
            throw new UserError(L.F("An area's name is 1 to {0} characters.", MaxLabel));
        a.Colour = a.Colour is not null && ColourText.IsMatch(a.Colour) ? a.Colour.ToLowerInvariant() : AreaColour;
        a.Rect = Rect(a.Rect) ?? throw new UserError(L.F("{0}: the area must be inside the window.", a.Label));
    }

    /// <summary>A box inside the window (a little room for rounding), rounded to 4 places; null when it isn't one.</summary>
    public static MacroRect? Rect(MacroRect? r)
    {
        if (r is null || !(r.X >= 0 && r.Y >= 0 && r.W > 0.001 && r.H > 0.001 && r.X + r.W <= 1.0001 && r.Y + r.H <= 1.0001))
            return null;
        return new MacroRect { X = Round(r.X), Y = Round(r.Y), W = Math.Min(Round(r.W), 1 - Round(r.X)), H = Math.Min(Round(r.H), 1 - Round(r.Y)) };
    }

    static double Round(double d) => Math.Round(d, 4);

    static JsonElement Num(double d) => JsonSerializer.SerializeToElement(d);

    static JsonElement Str(string s) => JsonSerializer.SerializeToElement(s);

    /// <summary>A value as the option takes it (a number kept in its range, a colour in small letters, a check as 1/0, a
    /// known key or choice as it is spelled there), or null.</summary>
    public static JsonElement? Normalize(MacroOption o, JsonElement v)
    {
        switch (o.Kind)
        {
            case "number":
            {
                double? d = v.ValueKind switch
                {
                    JsonValueKind.Number => v.GetDouble(),
                    JsonValueKind.String when double.TryParse(v.GetString(), NumberStyles.Float, CultureInfo.InvariantCulture, out double t) => t,
                    _ => null,
                };
                if (d is not { } n || !double.IsFinite(n))
                    return null;
                return Num(Math.Clamp(n, o.Min ?? double.MinValue, o.Max ?? double.MaxValue));
            }
            case "colour":
                return v.ValueKind == JsonValueKind.String && ColourText.IsMatch(v.GetString()!) ? Str(v.GetString()!.ToLowerInvariant()) : null;
            case "check":
                return v.ValueKind switch
                {
                    JsonValueKind.True => Num(1),
                    JsonValueKind.False => Num(0),
                    JsonValueKind.Number when double.IsFinite(v.GetDouble()) => Num(v.GetDouble() != 0 ? 1 : 0),
                    JsonValueKind.String when v.GetString() is "1" or "true" => Num(1),
                    JsonValueKind.String when v.GetString() is "0" or "false" => Num(0),
                    _ => null,
                };
            case "key":
            {
                if (v.ValueKind != JsonValueKind.String)
                    return null;
                string text = v.GetString()!;
                string? key = MacroKeys.All.Keys.Concat(MacroRules.MouseKeys.Keys).FirstOrDefault(k => k.Equals(text, StringComparison.OrdinalIgnoreCase));
                return key is null ? null : Str(key);
            }
            case "choice":
                return v.ValueKind == JsonValueKind.String && o.Choices?.FirstOrDefault(c => c.Equals(v.GetString(), StringComparison.OrdinalIgnoreCase)) is { } choice
                    ? Str(choice) : null;
            case "list":
            {
                // The ticked ones, spelled and ordered as the list.
                if (v.ValueKind != JsonValueKind.Array)
                    return null;
                var ticked = v.EnumerateArray().Where(w => w.ValueKind == JsonValueKind.String).Select(w => w.GetString()!).ToList();
                return JsonSerializer.SerializeToElement((o.Choices ?? new()).Where(c => ticked.Contains(c, StringComparer.OrdinalIgnoreCase)).ToList());
            }
            case "point":
                return v.ValueKind == JsonValueKind.Object && v.TryGetProperty("x", out var x) && v.TryGetProperty("y", out var y)
                    && x.ValueKind == JsonValueKind.Number && y.ValueKind == JsonValueKind.Number && double.IsFinite(x.GetDouble()) && double.IsFinite(y.GetDouble())
                    ? Point(x.GetDouble(), y.GetDouble()) : null;
            default:
                return null;
        }
    }

    /// <summary>An option's value when nothing else is set: its smallest number (or 0), white, off, Space, its first choice.</summary>
    public static JsonElement Plain(MacroOption o) => o.Kind switch
    {
        "number" => Num(Math.Clamp(0, o.Min ?? double.MinValue, o.Max ?? double.MaxValue)),
        "colour" => Str("#ffffff"),
        "check" => Num(0),
        "key" => Str("Space"),
        "point" => Point(0.5, 0.5),
        "list" => JsonSerializer.SerializeToElement(Array.Empty<string>()),
        _ => Str(o.Choices?.FirstOrDefault() ?? ""), // a heading's "": nothing a step can use
    };

    /// <summary>A point on the window, kept inside it, rounded to 4 places.</summary>
    public static JsonElement Point(double x, double y) =>
        JsonSerializer.SerializeToElement(new { x = Round(Math.Clamp(x, 0, 1)), y = Round(Math.Clamp(y, 0, 1)) });

    /// <summary>The option's value now (its default, else a plain one, when it has none).</summary>
    public static JsonElement ValueOf(MacroOption o) => o.Value ?? o.Default ?? Plain(o);

    /// <summary>Seconds shown in seconds; a part (0..1) in percent shown as percent.</summary>
    static double Shown(MacroOption o) => o.Unit == "s" ? 1000 : o.Unit == "%" && o.Max is <= 1 ? 0.01 : 1;

    /// <summary>A number as the panel shows it, with its unit ("0.5 s", "120 px").</summary>
    public static string Show(MacroOption o, JsonElement v)
    {
        switch (o.Kind)
        {
            case "number":
            {
                double d = v.ValueKind == JsonValueKind.Number ? v.GetDouble() / Shown(o) : 0;
                string unit = o.Unit is { Length: > 0 } u ? (u == "%" ? "%" : " " + u) : "";
                return d.ToString("0.##", CultureInfo.InvariantCulture) + unit;
            }
            case "check":
                return v.ValueKind == JsonValueKind.Number && v.GetDouble() != 0 ? L.T("On") : L.T("Off");
            case "list":
                return v.ValueKind == JsonValueKind.Array ? L.F("{0} of {1}", v.GetArrayLength(), o.Choices?.Count ?? 0) : "";
            case "point":
                // As parts of the window: the same place whatever its size.
                return v.ValueKind == JsonValueKind.Object && v.TryGetProperty("x", out var x) && v.TryGetProperty("y", out var y)
                    ? $"{Math.Round(x.GetDouble() * 100)}%, {Math.Round(y.GetDouble() * 100)}%" : "";
            default:
                return v.ValueKind == JsonValueKind.String ? v.GetString()! : "";
        }
    }

    /// <summary>The number typed on the panel (in seconds for seconds), as the option keeps it; null when it isn't one.</summary>
    public static JsonElement? Typed(MacroOption o, string text)
    {
        string t = Regex.Replace(text.Trim(), @"\s*(ms|px|s|%)$", "", RegexOptions.IgnoreCase).Replace(',', '.');
        if (!double.TryParse(t, NumberStyles.Float, CultureInfo.InvariantCulture, out double d) || !double.IsFinite(d))
            return null;
        return Normalize(o, Num(Math.Round(d * Shown(o), 6)));
    }

    /// <summary>What − and + add on the panel: a tenth of a second, 10 ms, 1 % or a small step for a small range.</summary>
    public static double Step(MacroOption o)
    {
        if (o.Unit == "s")
            return 100;
        if (o.Unit == "ms")
            return 10;
        if (o.Unit == "%" && o.Max is <= 1)
            return 0.01;
        double span = o.Min is { } min && o.Max is { } max ? max - min : double.MaxValue;
        return span <= 1 ? 0.01 : span <= 20 ? 0.1 : 1;
    }

    /// <summary>A number moved by − or + (<paramref name="by"/> steps), kept in its range.</summary>
    public static JsonElement Nudge(MacroOption o, int by)
    {
        double now = ValueOf(o) is { ValueKind: JsonValueKind.Number } v ? v.GetDouble() : 0;
        double step = Step(o);
        double next = Math.Round((now + by * step) / step) * step;
        return Normalize(o, Num(Math.Round(next, 6)))!.Value;
    }

    /// <summary>The variables the macro counts (setVar, addVar), shown on its panel as counters.</summary>
    public static List<string> Counters(Macro m) =>
        m.Steps.Where(s => s is { Kind: "setVar" or "addVar", Name: not null }).Select(s => s.Name!).Distinct().ToList();

    /// <summary>
    /// Panels shown by themselves while an account plays their game: those to show now (their game is played, not shown,
    /// not closed by the person meanwhile) and those to hide (shown by themselves, their game no longer played). A panel
    /// the person closed stays closed until nobody plays its game. <paramref name="auto"/> and <paramref name="closed"/>
    /// are kept up to date.
    /// </summary>
    public static (List<Guid> Show, List<Guid> Hide) AutoShow(IEnumerable<Macro> macros, IReadOnlySet<long> played, ISet<Guid> auto,
        ISet<Guid> closed, Func<Guid, bool> shown)
    {
        var show = new List<Guid>();
        var hide = new List<Guid>();
        foreach (var macro in macros)
        {
            if (macro.Panel is not { AutoShow: true, PlaceId: { } place })
                continue;
            if (!played.Contains(place))
            {
                closed.Remove(macro.Id);
                if (auto.Remove(macro.Id))
                    hide.Add(macro.Id);
            }
            else if (!closed.Contains(macro.Id) && !shown(macro.Id))
            {
                auto.Add(macro.Id);
                show.Add(macro.Id);
            }
        }
        return (show, hide);
    }
}

/// <summary>The maths of drawing on a game: areas and points kept as parts (0..1) of the window, shown in screen pixels;
/// areas moved by their middle and sized by their corners, never outside the window nor smaller than a few pixels; points
/// as numbered dots (the step's number).</summary>
static class AreaGeometry
{
    public enum Grip { None, Move, TopLeft, TopRight, BottomLeft, BottomRight }

    /// <summary>A step's point on the game: its index (0-based) and where it is.</summary>
    public readonly record struct Dot(int Step, double X, double Y);

    /// <summary>The points of the macro's click, hold, move and colour-at-a-place steps (those with a number, not a setting).</summary>
    public static List<Dot> Dots(Macro m)
    {
        var dots = new List<Dot>();
        for (int i = 0; i < m.Steps.Count; i++)
        {
            var s = m.Steps[i];
            bool point = s?.Kind is "click" or "move" || s?.Kind == "waitColour" && s.Region is null && MacroValues.RefOf(s.Extra, "region") is null;
            if (point && s!.X is { } x && s.Y is { } y && double.IsFinite(x) && double.IsFinite(y))
                dots.Add(new Dot(i, Math.Clamp(x, 0, 1), Math.Clamp(y, 0, 1)));
            // A point the player sets: where it is now.
            else if (point && MacroValues.RefOf(s!.Extra, "x") is { } name && m.Options.FirstOrDefault(o => o.Name == name && o.Kind == "point") is { } option
                && MacroOptions.ValueOf(option) is { ValueKind: JsonValueKind.Object } at)
                dots.Add(new Dot(i, at.GetProperty("x").GetDouble(), at.GetProperty("y").GetDouble()));
        }
        return dots;
    }

    public static System.Drawing.Point ToScreen(double x, double y, System.Drawing.Rectangle client) =>
        new(client.X + (int)Math.Round(x * client.Width), client.Y + (int)Math.Round(y * client.Height));

    /// <summary>A place on the window as parts of it (kept inside), rounded to 4 places.</summary>
    public static (double X, double Y) ToFraction(System.Drawing.Point p, System.Drawing.Rectangle client) => client.Width <= 0 || client.Height <= 0 ? (0.5, 0.5)
        : (Math.Round(Math.Clamp((p.X - client.X) / (double)client.Width, 0, 1), 4), Math.Round(Math.Clamp((p.Y - client.Y) / (double)client.Height, 0, 1), 4));

    /// <summary>The dot a press at <paramref name="p"/> takes (the last drawn is on top, within <paramref name="radius"/> px); -1 for none.</summary>
    public static int HitDot(IReadOnlyList<Dot> dots, System.Drawing.Point p, System.Drawing.Rectangle client, int radius)
    {
        for (int i = dots.Count - 1; i >= 0; i--)
        {
            var at = ToScreen(dots[i].X, dots[i].Y, client);
            int dx = at.X - p.X, dy = at.Y - p.Y;
            if (dx * dx + dy * dy <= radius * radius)
                return i;
        }
        return -1;
    }

    public static System.Drawing.Rectangle ToScreen(MacroRect r, System.Drawing.Rectangle client) => new(
        client.X + (int)Math.Round(r.X * client.Width), client.Y + (int)Math.Round(r.Y * client.Height),
        Math.Max(1, (int)Math.Round(r.W * client.Width)), Math.Max(1, (int)Math.Round(r.H * client.Height)));

    public static MacroRect ToFraction(System.Drawing.Rectangle r, System.Drawing.Rectangle client)
    {
        var inside = System.Drawing.Rectangle.Intersect(r, client);
        if (inside.Width <= 0 || inside.Height <= 0 || client.Width <= 0 || client.Height <= 0)
            return new MacroRect { X = 0, Y = 0, W = 1, H = 1 };
        return new MacroRect
        {
            X = Math.Round((inside.X - client.X) / (double)client.Width, 4),
            Y = Math.Round((inside.Y - client.Y) / (double)client.Height, 4),
            W = Math.Round(inside.Width / (double)client.Width, 4),
            H = Math.Round(inside.Height / (double)client.Height, 4),
        };
    }

    /// <summary>What a press at <paramref name="p"/> takes hold of: a corner (within <paramref name="grip"/> px), the inside, or nothing.</summary>
    public static Grip HitTest(System.Drawing.Rectangle box, System.Drawing.Point p, int grip)
    {
        bool Near(int a, int b) => Math.Abs(a - b) <= grip;
        if (Near(p.X, box.Left) && Near(p.Y, box.Top)) return Grip.TopLeft;
        if (Near(p.X, box.Right) && Near(p.Y, box.Top)) return Grip.TopRight;
        if (Near(p.X, box.Left) && Near(p.Y, box.Bottom)) return Grip.BottomLeft;
        if (Near(p.X, box.Right) && Near(p.Y, box.Bottom)) return Grip.BottomRight;
        return box.Contains(p) ? Grip.Move : Grip.None;
    }

    /// <summary>The box after dragging <paramref name="grip"/> by <paramref name="dx"/>, <paramref name="dy"/>: inside
    /// <paramref name="bounds"/>, at least <paramref name="min"/> px each way.</summary>
    public static System.Drawing.Rectangle Drag(System.Drawing.Rectangle start, Grip grip, int dx, int dy, System.Drawing.Rectangle bounds, int min)
    {
        int left = start.Left, top = start.Top, right = start.Right, bottom = start.Bottom;
        switch (grip)
        {
            case Grip.Move:
                dx = Math.Clamp(dx, bounds.Left - left, bounds.Right - right);
                dy = Math.Clamp(dy, bounds.Top - top, bounds.Bottom - bottom);
                return new System.Drawing.Rectangle(left + dx, top + dy, start.Width, start.Height);
            case Grip.TopLeft:
                left = Math.Clamp(left + dx, bounds.Left, right - min);
                top = Math.Clamp(top + dy, bounds.Top, bottom - min);
                break;
            case Grip.TopRight:
                right = Math.Clamp(right + dx, left + min, bounds.Right);
                top = Math.Clamp(top + dy, bounds.Top, bottom - min);
                break;
            case Grip.BottomLeft:
                left = Math.Clamp(left + dx, bounds.Left, right - min);
                bottom = Math.Clamp(bottom + dy, top + min, bounds.Bottom);
                break;
            case Grip.BottomRight:
                right = Math.Clamp(right + dx, left + min, bounds.Right);
                bottom = Math.Clamp(bottom + dy, top + min, bounds.Bottom);
                break;
            default:
                return start;
        }
        return System.Drawing.Rectangle.FromLTRB(left, top, right, bottom);
    }
}
