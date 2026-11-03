using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace RobloxManager;

// Macros (Beta): what a macro is, as the vault keeps it and the page edits it (the shape is the page's and the core's
// shared contract), the keys it may press, and the checks a macro passes before it is saved. Played by MacroPlayer.cs.

/// <summary>A sequence of steps played into the Roblox windows of some accounts, one window at a time.</summary>
public sealed class Macro
{
    public Guid Id { get; set; }
    public string Name { get; set; } = "";
    public List<MacroStep> Steps { get; set; } = new();
    /// <summary>"selected" (the accounts selected when run), "all" (every account with a window) or "accounts" (<see cref="AccountIds"/>).</summary>
    public string Targets { get; set; } = "selected";
    public List<Guid> AccountIds { get; set; } = new();
    public MacroTrigger Trigger { get; set; } = new();
    /// <summary>Triggers fire only while on; running it by hand always works.</summary>
    public bool Enabled { get; set; }
    /// <summary>1..9: Ctrl+Alt+that digit runs it; null: none. Unique across macros.</summary>
    public int? Hotkey { get; set; }
    /// <summary>± this much random time on every wait and hold.</summary>
    public int JitterMs { get; set; } = 40;
    /// <summary>± this many pixels on every click.</summary>
    public int JitterPx { get; set; } = 3;
    /// <summary>A run on one window stops after this.</summary>
    public int MaxMinutes { get; set; } = 10;
    /// <summary>When a wait times out or an image isn't found, or it is stuck: "stop", "skip", "restart" (through its
    /// "recover" steps, if any, then from the top), "rejoin" or "alert". Stuck with "skip": restarted.</summary>
    public string OnFail { get; set; } = "stop";
    /// <summary>Stuck when what its steps look for hasn't been seen this long (then <see cref="OnFail"/>); null: never.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? StuckMinutes { get; set; }
    /// <summary>The window's client size it was recorded or pictured at; images are scaled from it.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public MacroSize? RecordedSize { get; set; }
    /// <summary>What the player may change on its panel, in the panel's order (a step's field holds "$o1"; MacroPanel.cs).</summary>
    public List<MacroOption> Options { get; set; } = new();
    /// <summary>Named boxes on the game; a step's area holds "@a1".</summary>
    public List<MacroArea> Areas { get; set; } = new();
    public MacroPanel Panel { get; set; } = new();
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class MacroTrigger
{
    /// <summary>"manual", "interval", "join" (the account got into a game) or "rejoin" (auto-rejoin got it back in).</summary>
    public string Kind { get; set; } = "manual";
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? EverySeconds { get; set; }
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class MacroSize
{
    public int W { get; set; }
    public int H { get; set; }
}

/// <summary>Part of a window's client area, each value 0..1 of its width or height.</summary>
public sealed class MacroRect
{
    public double X { get; set; }
    public double Y { get; set; }
    public double W { get; set; }
    public double H { get; set; }
}

/// <summary>One step; which fields it has depends on <see cref="Kind"/> (see the contract, macro-contract.md).</summary>
public sealed class MacroStep
{
    public string Kind { get; set; } = "";
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Key { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Ms { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? X { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Y { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Button { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public bool? Double { get; set; }
    /// <summary>A PNG as base64. On a recorded click, the place clicked (the page may offer "click by image").</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Image { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public MacroRect? Region { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Threshold { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? TimeoutMs { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Dx { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Dy { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Notches { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public bool? Gone { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Text { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Colour { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Tolerance { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public MacroTest? Test { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Times { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Max { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Name { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Value { get; set; }
    /// <summary>follow: the colour of what to keep up with (a line), and the marker kept on it: one colour, or a box's
    /// edges {left, right} (each a colour or "$name"; see MacroFollow.cs).</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Target { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public JsonElement? Marker { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Fps { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? UntilGoneMs { get; set; }
    /// <summary>follow: "onOff" (held while behind, let go past) or "smooth" (short presses sized by distance and speed).</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Mode { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Kp { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Kd { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public bool? Predict { get; set; }
    /// <summary>click: held this long (a charge, a cast); 0 or none: a plain click.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? HoldMs { get; set; }
    /// <summary>clickColour: patches smaller than this (px across) are specks, not the button.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? MinSize { get; set; }
    /// <summary>clickColour: clicks again each time it shows up, until it has stayed away this long.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? RepeatUntilGoneMs { get; set; }
    /// <summary>clickColour: every patch is clicked, each once, after it has shown this long (rings to hit as they come);
    /// null: the first patch, at once.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? EachAfterMs { get; set; }
    /// <summary>waitText: any of these read counts (in place of <see cref="Text"/>); in practice "$o1", a list the
    /// player ticks on the panel.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public List<string>? Words { get; set; }
    /// <summary>alert: the window's picture goes with it.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public bool? Picture { get; set; }
    /// <summary>Unknown fields, and board values named instead of a number, colour or area ("ms": "$wait"; see MacroValues.cs).</summary>
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    internal MacroStep Clone() => (MacroStep)MemberwiseClone();
}

/// <summary>What an "if" or "loopUntil" asks: "image", "text", "colour" (on the window's picture) or "var".</summary>
public sealed class MacroTest
{
    public string Kind { get; set; } = "";
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Image { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public MacroRect? Region { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Threshold { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public bool? Not { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Text { get; set; }
    /// <summary>text: any of these read counts, as a waitText's <see cref="MacroStep.Words"/>.</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public List<string>? Words { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? X { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Y { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Colour { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public int? Tolerance { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Name { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public string? Op { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] public double? Value { get; set; }
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

    internal MacroTest Clone() => (MacroTest)MemberwiseClone();
}

/// <summary>A key a macro may press: its scan code (set 1), whether it is an extended key, and its virtual-key code.</summary>
readonly record struct MacroKey(string Name, ushort Scan, bool Extended, ushort Vk);

/// <summary>The keys macros may press, by the names the page uses. Sent as scan codes: games read those (Roblox among them).</summary>
static class MacroKeys
{
    public static readonly IReadOnlyDictionary<string, MacroKey> All = Build();

    static Dictionary<string, MacroKey> Build()
    {
        var keys = new Dictionary<string, MacroKey>(StringComparer.Ordinal);
        void Add(string name, ushort scan, ushort vk, bool extended = false) => keys[name] = new MacroKey(name, scan, extended, vk);
        // Letters, by their place on a US keyboard (scan codes name places, not letters).
        ushort[] letters = [0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18, 0x19, 0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D, 0x15, 0x2C];
        for (int i = 0; i < 26; i++)
            Add(((char)('A' + i)).ToString(), letters[i], (ushort)('A' + i));
        for (int d = 1; d <= 9; d++)
            Add(d.ToString(), (ushort)(0x01 + d), (ushort)('0' + d));
        Add("0", 0x0B, '0');
        for (int f = 1; f <= 10; f++)
            Add("F" + f, (ushort)(0x3A + f), (ushort)(0x6F + f));
        Add("F11", 0x57, 0x7A);
        Add("F12", 0x58, 0x7B);
        Add("Space", 0x39, 0x20);
        Add("Enter", 0x1C, 0x0D);
        Add("Tab", 0x0F, 0x09);
        Add("Backspace", 0x0E, 0x08);
        Add("Escape", 0x01, 0x1B);
        Add("Shift", 0x2A, 0xA0);
        Add("Ctrl", 0x1D, 0xA2);
        Add("Alt", 0x38, 0xA4);
        Add("Up", 0x48, 0x26, true);
        Add("Down", 0x50, 0x28, true);
        Add("Left", 0x4B, 0x25, true);
        Add("Right", 0x4D, 0x27, true);
        Add("Insert", 0x52, 0x2D, true);
        Add("Delete", 0x53, 0x2E, true);
        Add("Home", 0x47, 0x24, true);
        Add("End", 0x4F, 0x23, true);
        Add("PageUp", 0x49, 0x21, true);
        Add("PageDown", 0x51, 0x22, true);
        Add("Minus", 0x0C, 0xBD);
        Add("Equals", 0x0D, 0xBB);
        Add("LeftBracket", 0x1A, 0xDB);
        Add("RightBracket", 0x1B, 0xDD);
        Add("Semicolon", 0x27, 0xBA);
        Add("Quote", 0x28, 0xDE);
        Add("Comma", 0x33, 0xBC);
        Add("Period", 0x34, 0xBE);
        Add("Slash", 0x35, 0xBF);
        Add("Backslash", 0x2B, 0xDC);
        Add("Backquote", 0x29, 0xC0);
        // The number pad without Num Lock's say: its own scan codes, not extended (the arrows' are).
        ushort[] pad = [0x52, 0x4F, 0x50, 0x51, 0x4B, 0x4C, 0x4D, 0x47, 0x48, 0x49];
        for (int n = 0; n <= 9; n++)
            Add("Numpad" + n, pad[n], (ushort)(0x60 + n));
        return keys;
    }

    /// <summary>The key a scan code (and extended flag) names, as a keyboard hook or TinyTask reports it; null for others.</summary>
    public static MacroKey? FromScan(ushort scan, bool extended, ushort vk = 0)
    {
        // Right Ctrl and right Alt are the extended Ctrl and Alt; right Shift has a scan code of its own.
        if (scan == 0x1D)
            return All["Ctrl"];
        if (scan == 0x38)
            return All["Alt"];
        if (scan == 0x36)
            return All["Shift"];
        // The number pad's keys share scan codes with the arrows and Insert…: told apart by the extended flag.
        foreach (var key in All.Values)
        {
            if (key.Scan == scan && key.Extended == extended)
                return key;
        }
        // No scan code (some programs' recordings): by virtual key.
        if (scan == 0 && vk != 0)
            return FromVk(vk);
        return null;
    }

    public static MacroKey? FromVk(ushort vk)
    {
        vk = vk switch { 0x10 or 0xA1 => 0xA0, 0x11 or 0xA3 => 0xA2, 0x12 or 0xA5 => 0xA4, _ => vk };
        foreach (var key in All.Values)
        {
            if (key.Vk == vk)
                return key;
        }
        return null;
    }

    /// <summary>Keys a Vietnamese Telex input method rewrites (UniKey, EVKey): typed into Roblox they may come out changed.</summary>
    public static bool TelexRewrites(string key) => key is "W" or "A" or "O" or "E" or "D" or "S" or "F" or "R" or "X" or "J" or "Z";

    /// <summary>The key and Shift for a character typed into chat (US layout); null for one typed as itself (Unicode).</summary>
    public static (MacroKey Key, bool Shift)? ForChar(char c)
    {
        if (c is >= 'a' and <= 'z')
            return (All[char.ToUpperInvariant(c).ToString()], false);
        if (c is >= 'A' and <= 'Z')
            return (All[c.ToString()], true);
        if (c is >= '0' and <= '9')
            return (All[c.ToString()], false);
        const string plain = " -=[];',./\\`", shifted = "_+{}:\"<>?|~", digitsShifted = ")!@#$%^&*(";
        string[] names = ["Space", "Minus", "Equals", "LeftBracket", "RightBracket", "Semicolon", "Quote", "Comma", "Period", "Slash", "Backslash", "Backquote"];
        int i = plain.IndexOf(c);
        if (i >= 0)
            return (All[names[i]], false);
        i = shifted.IndexOf(c);
        if (i >= 0)
            return (All[names[i + 1]], true);
        i = digitsShifted.IndexOf(c);
        if (i >= 0)
            return (All[i.ToString()], true);
        return null;
    }
}

/// <summary>The checks a macro passes before it is saved, each refusal a sentence for the person; and the jumps its
/// if/else/loop steps make, worked out once.</summary>
static class MacroRules
{
    public const int MaxMacros = 100, MaxName = 60, MaxSteps = 500, MaxImageSide = 256, MaxDepth = 20;
    public const int MaxTypeText = 200, MaxWaitMs = 3_600_000;
    /// <summary>Base64 of every picture in one macro together: they ride in the encrypted vault, saved every few seconds.</summary>
    const int MaxImagesBase64 = 4 * 1024 * 1024;
    static readonly string[] TargetKinds = ["selected", "all", "accounts"];
    static readonly string[] TriggerKinds = ["manual", "interval", "join", "rejoin"];
    static readonly string[] OnFails = ["stop", "skip", "restart", "rejoin", "alert"];
    static readonly string[] Ops = ["<", "<=", "=", ">=", ">", "!="];
    static readonly Regex VarName = new("^[A-Za-z_][A-Za-z0-9_]{0,31}$", RegexOptions.CultureInvariant);
    static readonly Regex Colour = new("^#[0-9a-fA-F]{6}$", RegexOptions.CultureInvariant);

    /// <summary>
    /// Checks <paramref name="m"/> and fills in defaults. <paramref name="known"/> are the vault's accounts (others are
    /// dropped); <paramref name="others"/> the other macros (a hotkey is one macro's).
    /// </summary>
    public static void Check(Macro m, ICollection<Guid> known, IEnumerable<Macro> others)
    {
        m.Name = (m.Name ?? "").Trim();
        if (m.Name.Length == 0)
            throw new UserError(L.T("Give the macro a name."));
        if (m.Name.Length > MaxName)
            m.Name = m.Name[..MaxName].TrimEnd();
        m.Steps ??= new();
        if (m.Steps.Count == 0)
            throw new UserError(L.T("Add at least one step."));
        if (m.Steps.Count > MaxSteps)
            throw new UserError(L.F("A macro can have up to {0} steps.", MaxSteps));
        if (!TargetKinds.Contains(m.Targets))
            m.Targets = "selected";
        m.AccountIds = (m.AccountIds ?? new()).Where(known.Contains).Distinct().ToList();
        if (m.Targets == "accounts" && m.AccountIds.Count == 0)
            throw new UserError(L.T("Pick at least one account for the macro."));
        m.Trigger ??= new();
        if (!TriggerKinds.Contains(m.Trigger.Kind))
            throw new UserError(L.T("Unknown way to start the macro."));
        if (m.Trigger.Kind == "interval")
        {
            if (m.Trigger.EverySeconds is not (>= 10 and <= 86400))
                throw new UserError(L.T("Repeat every 10 seconds to 24 hours."));
        }
        else
            m.Trigger.EverySeconds = null;
        // A macro that starts by itself has no selection to go by.
        if (m.Trigger.Kind != "manual" && m.Targets == "selected")
            throw new UserError(L.T("A macro that starts by itself needs its accounts: choose All or pick them."));
        if (m.Hotkey is { } hotkey)
        {
            if (hotkey is < 1 or > 9)
                throw new UserError(L.T("The hotkey is Ctrl+Alt+1 to Ctrl+Alt+9."));
            if (others.FirstOrDefault(o => o.Id != m.Id && o.Hotkey == hotkey) is { } holder)
                throw new UserError(L.F("Ctrl+Alt+{0} already runs {1}.", hotkey, holder.Name));
        }
        if (m.JitterMs is < 0 or > 500)
            throw new UserError(L.T("Random extra wait is 0 to 0.5 s."));
        if (m.JitterPx is < 0 or > 20)
            throw new UserError(L.T("Random click offset is 0 to 20 px."));
        if (m.MaxMinutes is < 1 or > 240)
            throw new UserError(L.T("A run lasts 1 to 240 minutes."));
        if (!OnFails.Contains(m.OnFail))
            m.OnFail = "stop";
        if (m.StuckMinutes is <= 0)
            m.StuckMinutes = null;
        if (m.StuckMinutes is > 60)
            throw new UserError(L.T("Stuck after 1 to 60 minutes."));
        if (m.RecordedSize is { } size && (size.W is < 16 or > 16384 || size.H is < 16 or > 16384))
            m.RecordedSize = null;
        long images = 0;
        for (int i = 0; i < m.Steps.Count; i++)
        {
            var step = m.Steps[i] ?? throw new UserError(L.F("Step {0} is empty.", i + 1));
            try
            {
                CheckStep(step);
            }
            catch (UserError e)
            {
                throw new UserError(L.F("Step {0}: {1}", i + 1, e.Message));
            }
            images += (step.Image?.Length ?? 0) + (step.Test?.Image?.Length ?? 0);
            Note(step);
        }
        CounterLabels(m);
        if (images > MaxImagesBase64)
            throw new UserError(L.T("The macro's pictures are too big together. Use smaller ones."));
        MacroOptions.Check(m);
        Pairs(m.Steps);
        Jumps(m.Steps);
    }

    /// <summary>The longest note a step carries (why it is there, shown under it in the editor).</summary>
    public const int MaxNote = 80;

    /// <summary>A step's note: plain text up to <see cref="MaxNote"/>, kept trimmed; anything else is dropped.</summary>
    static void Note(MacroStep s)
    {
        if (s.Extra is null || !s.Extra.TryGetValue("note", out var note))
            return;
        string text = note.ValueKind == JsonValueKind.String ? (note.GetString() ?? "").Trim() : "";
        text = string.Concat(text.Where(ch => !char.IsControl(ch)));
        if (text.Length == 0)
            s.Extra.Remove("note");
        else
            s.Extra["note"] = JsonSerializer.SerializeToElement(text.Length > MaxNote ? text[..MaxNote].TrimEnd() : text);
    }

    /// <summary>What the panel calls each counter ({ name: label }): a counter's name to up to 30 characters of plain
    /// text; anything else is dropped.</summary>
    static void CounterLabels(Macro m)
    {
        if (m.Panel?.Extra is not { } extra || !extra.TryGetValue("counterLabels", out var labels))
            return;
        var kept = new Dictionary<string, string>();
        if (labels.ValueKind == JsonValueKind.Object)
            foreach (var p in labels.EnumerateObject())
            {
                string text = p.Value.ValueKind == JsonValueKind.String ? string.Concat((p.Value.GetString() ?? "").Where(ch => !char.IsControl(ch))).Trim() : "";
                if (VarName.IsMatch(p.Name) && text.Length > 0 && kept.Count < 100)
                    kept[p.Name] = text.Length > 30 ? text[..30].TrimEnd() : text;
            }
        if (kept.Count == 0)
            extra.Remove("counterLabels");
        else
            extra["counterLabels"] = JsonSerializer.SerializeToElement(kept);
    }

    static void CheckStep(MacroStep s)
    {
        // A field naming a board value passes with a stand-in; the value is checked when it plays.
        var filled = MacroValues.Fill(s);
        CheckStepFilled(s);
        MacroValues.Unfill(s, filled);
    }

    static void CheckStepFilled(MacroStep s)
    {
        switch (s.Kind)
        {
            case "key":
                NeedKey(s.Key, allowAlt: false);
                s.Ms = s.Ms is null or 0 ? 60 : s.Ms is >= 30 and <= 10_000 ? s.Ms : throw new UserError(L.T("Hold a key 0.03 to 10 s."));
                break;
            case "down" or "up":
                NeedKey(s.Key, allowAlt: true);
                break;
            case "click":
                NeedPoint(s.X, s.Y);
                s.Button ??= "left";
                if (s.Button is not ("left" or "right" or "middle"))
                    throw new UserError(L.T("Unknown mouse button."));
                // A recorded click's picture of where it clicked: kept when it is a usable one, else dropped quietly.
                if (s.Image is not null && !ImageOk(s.Image))
                    s.Image = null;
                // A plain click (none, 0), or held long enough to be a hold.
                if (s.HoldMs is < 0 or (> 0 and < 30) or > 10_000)
                    throw new UserError(L.T("Hold the button 0.03 to 10 s."));
                if (s.HoldMs == 0)
                    s.HoldMs = null;
                break;
            case "clickImage":
                NeedImage(s.Image);
                s.Button ??= "left";
                if (s.Button is not ("left" or "right"))
                    throw new UserError(L.T("Unknown mouse button."));
                NeedRegion(s.Region);
                s.Threshold = Threshold(s.Threshold);
                s.TimeoutMs = Timeout(s.TimeoutMs);
                break;
            case "camera":
                if (s.Dx is not (>= -100_000 and <= 100_000) || s.Dy is not (>= -100_000 and <= 100_000))
                    throw new UserError(L.T("Turn the camera by at most 100000 in each direction."));
                s.Ms = s.Ms is >= 0 and <= 10_000 ? s.Ms : s.Ms is null ? 200 : throw new UserError(L.T("A camera turn takes 0 to 10 s."));
                s.Button ??= "right";
                if (s.Button is not ("right" or "none"))
                    throw new UserError(L.T("Unknown mouse button."));
                break;
            case "scroll":
                if (s.Notches is not (>= -50 and <= 50) || s.Notches == 0)
                    throw new UserError(L.T("Scroll 1 to 50 notches up or down."));
                break;
            case "move":
                NeedPoint(s.X, s.Y);
                break;
            case "wait":
                if (s.Ms is not (>= 0 and <= MaxWaitMs))
                    throw new UserError(L.T("Wait up to 1 hour."));
                break;
            case "waitImage":
                NeedImage(s.Image);
                NeedRegion(s.Region);
                s.Threshold = Threshold(s.Threshold);
                s.TimeoutMs = Timeout(s.TimeoutMs);
                break;
            case "waitText":
                NeedWords(s.Text, s.Words);
                NeedRegion(s.Region);
                s.TimeoutMs = Timeout(s.TimeoutMs);
                break;
            case "waitColour":
                // A place, or an area searched for the colour.
                if (s.Region is null)
                    NeedPoint(s.X, s.Y);
                NeedRegion(s.Region);
                NeedColour(s.Colour);
                s.Tolerance = Tolerance(s.Tolerance);
                s.TimeoutMs = Timeout(s.TimeoutMs);
                break;
            case "waitStill":
                NeedRegion(s.Region);
                if (s.Ms is not (>= 100 and <= 60_000))
                    throw new UserError(L.T("Still for 0.1 to 60 s."));
                s.TimeoutMs = Timeout(s.TimeoutMs);
                break;
            case "if":
                CheckTest(s.Test);
                break;
            case "loop":
                if (s.Times is not (>= 0 and <= 100_000))
                    throw new UserError(L.T("Repeat up to 100000 times."));
                break;
            case "loopUntil":
                CheckTest(s.Test);
                s.Max ??= 0;
                if (s.Max is < 0 or > 100_000)
                    throw new UserError(L.T("Repeat up to 100000 times."));
                break;
            case "else" or "endIf" or "endLoop" or "break" or "rejoin" or "stop" or "recover":
                break;
            case "type":
                NeedText(s.Text, MaxTypeText);
                break;
            case "setVar" or "addVar":
                NeedVar(s.Name);
                if (s.Value is not { } value || !double.IsFinite(value))
                    throw new UserError(L.T("Enter a number."));
                break;
            case "alert":
                NeedText(s.Text, 200);
                if (s.Picture != true)
                    s.Picture = null;
                break;
            case "clickColour":
                NeedColour(s.Colour);
                NeedRegion(s.Region);
                s.Tolerance = Tolerance(s.Tolerance);
                s.Button ??= "left";
                if (s.Button is not ("left" or "right"))
                    throw new UserError(L.T("Unknown mouse button."));
                s.MinSize ??= 4;
                if (s.MinSize is < 1 or > 500)
                    throw new UserError(L.T("The smallest patch is 1 to 500 px."));
                s.TimeoutMs = Timeout(s.TimeoutMs);
                if (s.RepeatUntilGoneMs is < 0 or > MaxWaitMs)
                    throw new UserError(L.T("Wait up to 1 hour."));
                if (s.RepeatUntilGoneMs == 0)
                    s.RepeatUntilGoneMs = null;
                if (s.EachAfterMs is < 0 or > 10_000)
                    throw new UserError(L.T("Click after 0 to 10 seconds."));
                if (s.EachAfterMs is not null)
                    s.RepeatUntilGoneMs ??= 1000; // each patch as it comes: goes on until none comes for a while
                break;
            case "follow":
                NeedRegion(s.Region);
                if (s.Region is null)
                    throw new UserError(L.T("Draw the area the bar is in."));
                NeedColour(s.Target);
                MacroFollow.CheckMarker(s.Marker);
                s.Tolerance = Tolerance(s.Tolerance);
                s.Button ??= "left";
                if (s.Button is not ("left" or "right" or "key"))
                    throw new UserError(L.T("Unknown mouse button."));
                if (s.Button == "key")
                    NeedKey(s.Key, allowAlt: false);
                else
                    s.Key = null;
                s.Mode ??= "onOff";
                if (s.Mode is not ("onOff" or "smooth"))
                    throw new UserError(L.T("Unknown way to follow."));
                s.Kp ??= 1.0;
                s.Kd ??= 0.4;
                if (s.Kp is not (>= 0 and <= 10) || s.Kd is not (>= 0 and <= 10))
                    throw new UserError(L.T("Strength and damping are 0 to 10."));
                if (s.Predict != true)
                    s.Predict = null;
                s.Fps ??= 60;
                if (s.Fps is < 15 or > 240)
                    throw new UserError(L.T("Check 15 to 240 times a second."));
                s.UntilGoneMs ??= 1000;
                if (s.UntilGoneMs is < 0 or > 60_000)
                    throw new UserError(L.T("Gone for up to 60 s."));
                s.TimeoutMs = Timeout(s.TimeoutMs);
                break;
            default:
                throw new UserError(L.T("Unknown step."));
        }
    }

    internal static void CheckTest(MacroTest? t)
    {
        if (t is null)
            throw new UserError(L.T("Choose what to check."));
        var filled = MacroValues.Fill(t);
        CheckTestFilled(t);
        MacroValues.Unfill(t, filled);
    }

    static void CheckTestFilled(MacroTest t)
    {
        switch (t.Kind)
        {
            case "image":
                NeedImage(t.Image);
                NeedRegion(t.Region);
                t.Threshold = Threshold(t.Threshold);
                break;
            case "text":
                NeedWords(t.Text, t.Words);
                NeedRegion(t.Region);
                break;
            case "colour":
                NeedPoint(t.X, t.Y);
                NeedColour(t.Colour);
                t.Tolerance = Tolerance(t.Tolerance);
                break;
            case "var":
                NeedVar(t.Name);
                if (!Ops.Contains(t.Op))
                    throw new UserError(L.T("Unknown comparison."));
                // A number, a word (a choice's option), or "$name".
                if (t.Extra is not null && t.Extra.TryGetValue("value", out var word) && word.ValueKind == JsonValueKind.String)
                {
                    t.Value = null;
                    if (word.GetString()!.Length is 0 or > 40)
                        throw new UserError(L.T("Enter a number."));
                }
                else if (t.Value is not { } value || !double.IsFinite(value))
                    throw new UserError(L.T("Enter a number."));
                break;
            default:
                throw new UserError(L.T("Choose what to check."));
        }
    }

    /// <summary>The mouse buttons a Hold / Release step may press, by the names a key has there.</summary>
    internal static readonly Dictionary<string, string> MouseKeys = new(StringComparer.Ordinal)
    {
        ["MouseLeft"] = "left", ["MouseRight"] = "right", ["MouseMiddle"] = "middle",
    };

    static void NeedKey(string? key, bool allowAlt)
    {
        if (allowAlt && key is not null && MouseKeys.ContainsKey(key))
            return;
        if (key is null || !MacroKeys.All.ContainsKey(key))
            throw new UserError(L.T("Choose a key."));
        if (key == "Alt" && !allowAlt)
            throw new UserError(L.T("Alt only works as Hold and Release, around another key."));
    }

    static void NeedPoint(double? x, double? y)
    {
        if (x is not (>= 0 and <= 1) || y is not (>= 0 and <= 1))
            throw new UserError(L.T("The place must be inside the window."));
    }

    static void NeedRegion(MacroRect? r)
    {
        if (r is null)
            return;
        if (!(r.X >= 0 && r.Y >= 0 && r.W > 0 && r.H > 0 && r.X + r.W <= 1.0001 && r.Y + r.H <= 1.0001))
            throw new UserError(L.T("The area must be inside the window."));
    }

    /// <summary>A text to read, or a list of them (any counts).</summary>
    static void NeedWords(string? text, List<string>? words)
    {
        if (words is null)
        {
            NeedText(text, 100);
            return;
        }
        if (words.Count > MacroOptions.MaxChoices || words.Any(w => string.IsNullOrWhiteSpace(w) || w.Length > 100))
            throw new UserError(L.F("A list has up to {0} words of 1 to 100 characters.", MacroOptions.MaxChoices));
    }

    static void NeedText(string? text, int max)
    {
        if (string.IsNullOrWhiteSpace(text))
            throw new UserError(L.T("Enter the text."));
        if (text.Length > max)
            throw new UserError(L.F("Text can be up to {0} characters.", max));
        if (text.Any(char.IsControl))
            throw new UserError(L.T("Text can't have line breaks or tabs."));
    }

    static void NeedVar(string? name)
    {
        if (name is null || !VarName.IsMatch(name))
            throw new UserError(L.T("A counter's name is letters, digits and _ (up to 32), not starting with a digit."));
    }

    static void NeedColour(string? colour)
    {
        if (colour is null || !Colour.IsMatch(colour))
            throw new UserError(L.T("Choose a color."));
    }

    static void NeedImage(string? image)
    {
        if (image is null || !ImageOk(image))
            throw new UserError(L.F("Cut a picture of up to {0}×{0} px from the window.", MaxImageSide));
    }

    /// <summary>A PNG (its header only is read) no bigger than <see cref="MaxImageSide"/> on each side.</summary>
    internal static bool ImageOk(string base64)
    {
        if (base64.Length > MaxImagesBase64 || PngSize(base64) is not { } size)
            return false;
        return size.W is >= 1 and <= MaxImageSide && size.H is >= 1 and <= MaxImageSide;
    }

    /// <summary>A PNG's width and height from its IHDR; null when it isn't one.</summary>
    internal static (int W, int H)? PngSize(string base64)
    {
        if (base64.Length < 44)
            return null;
        Span<byte> head = stackalloc byte[33];
        if (!Convert.TryFromBase64String(base64[..44], head, out int got) || got < 24)
            return null;
        ReadOnlySpan<byte> signature = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        if (!head[..8].SequenceEqual(signature) || head[12] != 'I' || head[13] != 'H' || head[14] != 'D' || head[15] != 'R')
            return null;
        int w = System.Buffers.Binary.BinaryPrimitives.ReadInt32BigEndian(head[16..20]);
        int h = System.Buffers.Binary.BinaryPrimitives.ReadInt32BigEndian(head[20..24]);
        return (w, h);
    }

    static double Threshold(double? threshold) =>
        threshold is null ? 0.9 : threshold is >= 0.5 and <= 1 ? threshold.Value : throw new UserError(L.T("Match 50 to 100 %."));

    static int Timeout(int? timeout) =>
        timeout is >= 0 and <= MaxWaitMs ? timeout.Value : timeout is null ? 10_000 : throw new UserError(L.T("Wait up to 1 hour."));

    static int Tolerance(int? tolerance) =>
        tolerance is null ? 24 : tolerance is >= 0 and <= 255 ? tolerance.Value : throw new UserError(L.T("Color closeness is 0 to 255."));

    /// <summary>Every key held (down) is let go (up) later, and only a held key is let go.</summary>
    static void Pairs(List<MacroStep> steps)
    {
        var held = new HashSet<string>();
        for (int i = 0; i < steps.Count; i++)
        {
            var s = steps[i];
            // The macro's own steps end where its If stuck part begins.
            if (s.Kind == "recover" && held.Count > 0)
                throw new UserError(L.F("{0} is held down and never let go. Add a “Let go of a key” step.", held.First()));
            if (s.Kind == "down" && !held.Add(s.Key!))
                throw new UserError(L.F("Step {0}: {1} is already held down.", i + 1, s.Key));
            if (s.Kind == "up" && !held.Remove(s.Key!))
                throw new UserError(L.F("Step {0}: lets go of {1}, which isn't held down.", i + 1, s.Key));
        }
        if (held.Count > 0)
            throw new UserError(L.F("{0} is held down and never let go. Add a “Let go of a key” step.", held.First()));
    }

    /// <summary>
    /// Where each control step goes: an "if" to its "else" or "endIf", an "else" to its "endIf", a loop to its "endLoop"
    /// and back, a "break" to its loop's "endLoop". Refuses steps that don't nest.
    /// </summary>
    public static int[] Jumps(List<MacroStep> steps)
    {
        var jump = Enumerable.Repeat(-1, steps.Count).ToArray();
        var open = new Stack<(string Kind, int At)>();
        var breaks = new Dictionary<int, List<int>>(); // a loop's start → its breaks
        for (int i = 0; i < steps.Count; i++)
        {
            switch (steps[i].Kind)
            {
                case "if":
                    open.Push(("if", i));
                    break;
                case "else":
                    if (open.Count == 0 || open.Peek().Kind != "if")
                        throw new UserError(L.F("Step {0}: “Otherwise” needs an “If…” before it.", i + 1));
                    jump[open.Pop().At] = i;
                    open.Push(("else", i));
                    break;
                case "endIf":
                    if (open.Count == 0 || open.Peek().Kind is not ("if" or "else"))
                        throw new UserError(L.F("Step {0}: “End if” has no If to end.", i + 1));
                    jump[open.Pop().At] = i;
                    break;
                case "loop" or "loopUntil":
                    open.Push(("loop", i));
                    breaks[i] = [];
                    break;
                case "endLoop":
                    if (open.Count == 0 || open.Peek().Kind != "loop")
                        throw new UserError(L.F("Step {0}: “End repeat” has no repeat to end.", i + 1));
                    int start = open.Pop().At;
                    jump[start] = i;
                    jump[i] = start;
                    foreach (int b in breaks[start])
                        jump[b] = i;
                    break;
                case "recover":
                    // The steps after it run only to get unstuck: it ends the macro's own steps, so it can't be inside one.
                    if (open.Count > 0)
                        throw new UserError(L.F("Step {0}: “If stuck…” can't be inside a repeat or an If.", i + 1));
                    if (steps.FindIndex(st => st.Kind == "recover") != i)
                        throw new UserError(L.F("Step {0}: a macro has one If stuck part.", i + 1));
                    break;
                case "break":
                    var loop = open.FirstOrDefault(o => o.Kind == "loop");
                    if (loop.Kind is null)
                        throw new UserError(L.F("Step {0}: “Leave the repeat” only works inside a repeat.", i + 1));
                    breaks[loop.At].Add(i);
                    break;
            }
            if (open.Count > MaxDepth)
                throw new UserError(L.F("Steps can be nested {0} deep at most.", MaxDepth));
        }
        if (open.Count > 0)
        {
            var (kind, at) = open.Peek();
            throw new UserError(kind == "loop" ? L.F("Step {0}: this repeat never ends. Add “End repeat”.", at + 1) : L.F("Step {0}: this If never ends. Add “End if”.", at + 1));
        }
        return jump;
    }

    /// <summary>A macro the page or a file sent, read leniently (the checks above decide what is kept).</summary>
    public static Macro Read(JsonElement json) => MacroValues.Read(json);

    /// <summary>The page's shape (camelCase), numbers given as text taken too.</summary>
    internal static readonly JsonSerializerOptions PageJson = new(JsonSerializerDefaults.Web) { NumberHandling = JsonNumberHandling.AllowReadingFromString };

    /// <summary>A copy, through its JSON (pictures and unknown fields too).</summary>
    public static Macro Copy(Macro m) => JsonSerializer.Deserialize<Macro>(JsonSerializer.Serialize(m))!;
}
