using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>A run needs a setting or area that isn't there (or isn't a number, colour, key or area as the step needs).</summary>
sealed class MacroValueMissing(string name) : Exception($"No value {name}.")
{
    public string Name { get; } = name;
}

/// <summary>
/// Settings and areas in macros: a step's or check's number, colour or key may be "$o1", one of the macro's options
/// (MacroPanel.cs), and its area "@a1", one of the macro's areas; read when the step plays. Such a field is kept as text
/// in the step's unknown fields (<see cref="MacroStep.Extra"/>, under the field's own name), so the page and the file
/// show it where the number would be, and the typed field stays empty.
/// </summary>
static class MacroValues
{
    enum Shape { Int, Real, Colour, Region, Key, Words }

    static readonly Dictionary<string, Shape> StepFields = new(StringComparer.Ordinal)
    {
        ["ms"] = Shape.Int, ["x"] = Shape.Real, ["y"] = Shape.Real, ["dx"] = Shape.Int, ["dy"] = Shape.Int,
        ["notches"] = Shape.Int, ["timeoutMs"] = Shape.Int, ["threshold"] = Shape.Real, ["tolerance"] = Shape.Int,
        ["times"] = Shape.Int, ["max"] = Shape.Int, ["value"] = Shape.Real, ["fps"] = Shape.Int, ["untilGoneMs"] = Shape.Int,
        ["holdMs"] = Shape.Int, ["kp"] = Shape.Real, ["kd"] = Shape.Real, ["minSize"] = Shape.Int, ["repeatUntilGoneMs"] = Shape.Int,
        ["colour"] = Shape.Colour, ["target"] = Shape.Colour, ["region"] = Shape.Region, ["key"] = Shape.Key,
        ["eachAfterMs"] = Shape.Int, ["words"] = Shape.Words,
    };

    static readonly Dictionary<string, Shape> TestFields = new(StringComparer.Ordinal)
    {
        ["x"] = Shape.Real, ["y"] = Shape.Real, ["threshold"] = Shape.Real, ["tolerance"] = Shape.Int, ["value"] = Shape.Real,
        ["colour"] = Shape.Colour, ["region"] = Shape.Region, ["words"] = Shape.Words,
    };

    /// <summary>An option's or area's name ("o1", "a1").</summary>
    internal static readonly Regex Name = new("^[A-Za-z_][A-Za-z0-9_]{0,23}$", RegexOptions.CultureInvariant);
    static readonly Regex ColourText = new("^#[0-9a-fA-F]{6}$", RegexOptions.CultureInvariant);

    /// <summary>A macro the page or a file sent: "$o1" and "@a1" fields are set aside before the typed reading, then kept as text.</summary>
    public static Macro Read(JsonElement json)
    {
        if (json.ValueKind != JsonValueKind.Object)
            throw new UserError(L.T("That isn't a macro."));
        var root = JsonNode.Parse(json.GetRawText(), new JsonNodeOptions { PropertyNameCaseInsensitive = true })!.AsObject();
        DropEmpty(root, "id"); // a new macro comes with an empty id
        var set = new List<(int Step, bool Test, string Field, JsonElement Value)>();
        if (root["steps"] is JsonArray steps)
        {
            for (int i = 0; i < steps.Count; i++)
            {
                if (steps[i] is not JsonObject step)
                    continue;
                foreach (var (field, value) in Take(step, StepFields, test: false))
                    set.Add((i, false, field, value));
                if (step["test"] is JsonObject test)
                {
                    foreach (var (field, value) in Take(test, TestFields, test: true))
                        set.Add((i, true, field, value));
                }
            }
        }
        var macro = root.Deserialize<Macro>(MacroRules.PageJson) ?? throw new UserError(L.T("That isn't a macro."));
        foreach (var (i, test, field, value) in set)
        {
            if (i >= macro.Steps.Count || macro.Steps[i] is not { } step)
                continue;
            if (test)
            {
                if (step.Test is { } t)
                    (t.Extra ??= new())[field] = value;
            }
            else
                (step.Extra ??= new())[field] = value;
        }
        return macro;
    }

    /// <summary>Text where a number, colour, key or area goes: "$o1" anywhere, "@a1" for an area, and a var check's word
    /// to compare a choice with.</summary>
    static List<(string Field, JsonElement Value)> Take(JsonObject o, Dictionary<string, Shape> fields, bool test)
    {
        var taken = new List<(string, JsonElement)>();
        foreach (var (key, node) in o.ToList())
        {
            string? field = fields.Keys.FirstOrDefault(f => f.Equals(key, StringComparison.OrdinalIgnoreCase));
            if (field is null || node is not JsonValue v || !v.TryGetValue<string>(out var text))
                continue;
            bool word = test && field == "value" && !double.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture, out _);
            if (!text.StartsWith('$') && !(text.StartsWith('@') && fields[field] == Shape.Region) && !word)
                continue;
            o.Remove(key);
            taken.Add((field, JsonSerializer.SerializeToElement(text)));
        }
        return taken;
    }

    /// <summary>A field sent empty ("") where a Guid goes: left out (a new macro's id).</summary>
    internal static void DropEmpty(JsonObject o, string field)
    {
        if (o[field] is JsonValue v && v.TryGetValue<string>(out var text) && text.Length == 0)
            o.Remove(field);
    }

    /// <summary>What a field names: an option ("o1" for "$o1"), an area as itself ("@a1"), or null.</summary>
    public static string? RefOf(Dictionary<string, JsonElement>? extra, string field) =>
        extra is not null && extra.TryGetValue(field, out var v) && v.ValueKind == JsonValueKind.String
            ? v.GetString() switch { ['$', .. var name] => name, ['@', ..] area when field == "region" => area, _ => null } : null;

    /// <summary>The fields of a step (or check) that name a setting or an area.</summary>
    static IEnumerable<(string Field, string Name, Shape Shape)> Refs(Dictionary<string, JsonElement>? extra, Dictionary<string, Shape> fields)
    {
        if (extra is null)
            yield break;
        foreach (var (field, shape) in fields)
        {
            if (RefOf(extra, field) is { } name)
                yield return (field, name, shape);
        }
    }

    public static bool HasRefs(MacroStep s) => Refs(s.Extra, StepFields).Any() || s.Test is { } t && (Refs(t.Extra, TestFields).Any() || t.Kind == "var");

    /// <summary>Every option ("o1") and area ("@a1") the macro names, for the checks.</summary>
    public static IEnumerable<string> Names(Macro m) =>
        m.Steps.SelectMany(s => Refs(s.Extra, StepFields).Select(r => r.Name)
            .Concat(s.Test is { } t ? Refs(t.Extra, TestFields).Select(r => r.Name) : [])).Distinct();

    // ───────────────────────── checking ─────────────────────────

    /// <summary>
    /// Before a step is checked: each field naming a setting or area gets a stand-in that passes (the value itself is checked
    /// when it plays). Returns the fields to empty again after the check (<see cref="Unfill"/>).
    /// </summary>
    internal static List<string> Fill(MacroStep s) => Fill(s.Extra, StepFields, (f, v) => Set(s, f, v, clamp: false));

    internal static List<string> Fill(MacroTest t) => Fill(t.Extra, TestFields, (f, v) => Set(t, f, v, clamp: false));

    static List<string> Fill(Dictionary<string, JsonElement>? extra, Dictionary<string, Shape> fields, Action<string, JsonElement> set)
    {
        var filled = new List<string>();
        if (extra is null)
            return filled;
        foreach (var (field, shape) in fields)
        {
            if (!extra.TryGetValue(field, out var v) || v.ValueKind != JsonValueKind.String)
                continue;
            string text = v.GetString()!;
            if (text.StartsWith('$') || text.StartsWith('@') && shape == Shape.Region)
            {
                if (!Name.IsMatch(text[1..]))
                    throw new UserError(L.F("{0} isn't a setting of the macro.", text));
                set(field, StandIn(field, shape));
                filled.Add(field);
            }
            else if (field != "value")
                extra.Remove(field); // text where only a number goes, not a name: dropped
        }
        return filled;
    }

    static JsonElement StandIn(string field, Shape shape) => shape switch
    {
        Shape.Colour => JsonSerializer.SerializeToElement("#000000"),
        Shape.Region => JsonSerializer.SerializeToElement(new { x = 0, y = 0, w = 1, h = 1 }),
        Shape.Key => JsonSerializer.SerializeToElement("Space"),
        Shape.Words => JsonSerializer.SerializeToElement(new[] { "x" }),
        _ => JsonSerializer.SerializeToElement(field switch
        {
            "x" or "y" => 0.5,
            "threshold" => 0.9,
            "ms" => 100,
            "timeoutMs" => 1000,
            "tolerance" => 24,
            "fps" => 60,
            "untilGoneMs" => 500,
            "kp" => 1.0,
            "kd" => 0.4,
            "minSize" => 4,
            "notches" or "times" or "max" => 1,
            _ => 0,
        }),
    };

    internal static void Unfill(MacroStep s, List<string> fields)
    {
        foreach (string field in fields)
            Clear(s, field);
    }

    internal static void Unfill(MacroTest t, List<string> fields)
    {
        foreach (string field in fields)
            Clear(t, field);
    }

    // ───────────────────────── playing ─────────────────────────

    /// <summary>The step with the settings and areas it names put in (the step itself when it names none).</summary>
    /// <exception cref="MacroValueMissing">A value it needs isn't there, or isn't what the field takes.</exception>
    public static MacroStep Resolve(MacroStep s, Func<string, JsonElement?> value)
    {
        var refs = Refs(s.Extra, StepFields).ToList();
        var test = s.Test is { } t && Refs(t.Extra, TestFields).Any() ? Resolve(t, value) : s.Test;
        if (refs.Count == 0 && test == s.Test)
            return s;
        var copy = s.Clone();
        copy.Test = test;
        foreach (var (field, name, _) in refs)
        {
            if (value(name) is not { } v || !Set(copy, field, Part(v, field), clamp: true))
                throw new MacroValueMissing(name);
        }
        return copy;
    }

    /// <summary>A point setting ({x, y}) gives x and y each their own part.</summary>
    static JsonElement Part(JsonElement v, string field) =>
        field is "x" or "y" && v.ValueKind == JsonValueKind.Object && v.TryGetProperty(field, out var part) ? part : v;

    public static MacroTest Resolve(MacroTest t, Func<string, JsonElement?> value)
    {
        var refs = Refs(t.Extra, TestFields).Where(r => !(t.Kind == "var" && r.Field == "value")).ToList();
        if (refs.Count == 0)
            return t;
        var copy = t.Clone();
        foreach (var (field, name, _) in refs)
        {
            if (value(name) is not { } v || !Set(copy, field, Part(v, field), clamp: true))
                throw new MacroValueMissing(name);
        }
        return copy;
    }

    /// <summary>
    /// A var check: the account's variable or a setting, against a number, a word (a choice's option) or another
    /// value. A variable the macro sets itself counts as 0 before it is first set.
    /// </summary>
    public static bool VarTest(MacroTest t, Func<string, JsonElement?> value, bool setByMacro)
    {
        JsonElement left = value(t.Name!) ?? (setByMacro ? JsonSerializer.SerializeToElement(0) : throw new MacroValueMissing(t.Name!));
        JsonElement right;
        if (RefOf(t.Extra, "value") is { } name)
            right = value(name) ?? throw new MacroValueMissing(name);
        else if (t.Extra is not null && t.Extra.TryGetValue("value", out var word) && word.ValueKind == JsonValueKind.String)
            right = word;
        else
            right = JsonSerializer.SerializeToElement(t.Value ?? 0);
        if (Number(left) is { } a && Number(right) is { } b)
            return MacroPlayer.Compare(a, t.Op!, b);
        string l = Text(left), r = Text(right);
        bool same = string.Equals(l, r, StringComparison.OrdinalIgnoreCase);
        return t.Op switch
        {
            "=" => same,
            "!=" => !same,
            _ => false,
        };
    }

    static double? Number(JsonElement v) => v.ValueKind switch
    {
        JsonValueKind.Number => v.GetDouble(),
        JsonValueKind.True => 1,
        JsonValueKind.False => 0,
        JsonValueKind.String when double.TryParse(v.GetString(), NumberStyles.Float, CultureInfo.InvariantCulture, out double d) => d,
        _ => null,
    };

    static string Text(JsonElement v) => v.ValueKind == JsonValueKind.String ? v.GetString()! : v.GetRawText();

    static MacroRect? Region(JsonElement v, bool clamp)
    {
        if (v.ValueKind != JsonValueKind.Object)
            return null;
        try
        {
            var r = v.Deserialize<MacroRect>(MacroRules.PageJson);
            if (r is null || !double.IsFinite(r.X + r.Y + r.W + r.H))
                return null;
            if (clamp)
            {
                r.X = Math.Clamp(r.X, 0, 0.99);
                r.Y = Math.Clamp(r.Y, 0, 0.99);
                r.W = Math.Clamp(r.W, 0.01, 1 - r.X);
                r.H = Math.Clamp(r.H, 0.01, 1 - r.Y);
            }
            return r;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    static int? Int(JsonElement v, string field, bool clamp)
    {
        if (Number(v) is not { } d || !double.IsFinite(d))
            return null;
        long n = (long)Math.Round(Math.Clamp(d, int.MinValue, int.MaxValue));
        if (!clamp)
            return (int)n;
        var (lo, hi) = field switch
        {
            "dx" or "dy" => (-100_000, 100_000),
            "notches" => (-50, 50),
            "tolerance" => (0, 255),
            "times" or "max" => (0, 100_000),
            "fps" => (15, 240),
            "holdMs" or "eachAfterMs" => (0, 10_000),
            "minSize" => (1, 500),
            _ => (0, MacroRules.MaxWaitMs),
        };
        return (int)Math.Clamp(n, lo, hi);
    }

    static double? Real(JsonElement v, string field, bool clamp)
    {
        if (Number(v) is not { } d || !double.IsFinite(d))
            return null;
        return !clamp ? d : field switch
        {
            "x" or "y" => Math.Clamp(d, 0, 1),
            "threshold" => Math.Clamp(d, 0.5, 1),
            "kp" or "kd" => Math.Clamp(d, 0, 10),
            _ => d,
        };
    }

    static string? Colour(JsonElement v) =>
        v.ValueKind == JsonValueKind.String && v.GetString() is { } c && ColourText.IsMatch(c) ? c : null;

    /// <summary>A key a step presses (a mouse button too, for Hold and Release only), as the keys list spells it.</summary>
    static string? Key(JsonElement v, bool mouse) => v.ValueKind == JsonValueKind.String && v.GetString() is { } k
        ? MacroKeys.All.Keys.Concat(mouse ? MacroRules.MouseKeys.Keys : []).FirstOrDefault(name => name.Equals(k, StringComparison.OrdinalIgnoreCase)) : null;

    /// <summary>A list's words (a list setting's ticked ones; none ticked: an empty list, so nothing is ever read).</summary>
    static List<string>? Words(JsonElement v) => v.ValueKind == JsonValueKind.Array
        ? v.EnumerateArray().Where(w => w.ValueKind == JsonValueKind.String).Select(w => w.GetString()!).ToList() : null;

    /// <returns>false when the value isn't what the field takes.</returns>
    static bool Set(MacroStep s, string field, JsonElement v, bool clamp)
    {
        switch (field)
        {
            case "x": return (s.X = Real(v, field, clamp)) is not null;
            case "y": return (s.Y = Real(v, field, clamp)) is not null;
            case "threshold": return (s.Threshold = Real(v, field, clamp)) is not null;
            case "value": return (s.Value = Real(v, field, clamp)) is not null;
            case "ms": return (s.Ms = Int(v, field, clamp)) is not null;
            case "dx": return (s.Dx = Int(v, field, clamp)) is not null;
            case "dy": return (s.Dy = Int(v, field, clamp)) is not null;
            case "notches": return (s.Notches = Int(v, field, clamp)) is not null;
            case "timeoutMs": return (s.TimeoutMs = Int(v, field, clamp)) is not null;
            case "tolerance": return (s.Tolerance = Int(v, field, clamp)) is not null;
            case "times": return (s.Times = Int(v, field, clamp)) is not null;
            case "max": return (s.Max = Int(v, field, clamp)) is not null;
            case "fps": return (s.Fps = Int(v, field, clamp)) is not null;
            case "holdMs": return (s.HoldMs = Int(v, field, clamp)) is not null;
            case "minSize": return (s.MinSize = Int(v, field, clamp)) is not null;
            case "repeatUntilGoneMs": return (s.RepeatUntilGoneMs = Int(v, field, clamp)) is not null;
            case "eachAfterMs": return (s.EachAfterMs = Int(v, field, clamp)) is not null;
            case "words": return (s.Words = Words(v)) is not null;
            case "kp": return (s.Kp = Real(v, field, clamp)) is not null;
            case "kd": return (s.Kd = Real(v, field, clamp)) is not null;
            case "untilGoneMs": return (s.UntilGoneMs = Int(v, field, clamp)) is not null;
            case "colour": return (s.Colour = Colour(v)) is not null;
            case "target": return (s.Target = Colour(v)) is not null;
            case "region": return (s.Region = Region(v, clamp)) is not null;
            // A Press step or follow's key can't be a mouse button (a setting of the key kind may hold one): missingValue.
            case "key": return (s.Key = Key(v, mouse: s.Kind is "down" or "up")) is not null;
            default: return false;
        }
    }

    static bool Set(MacroTest t, string field, JsonElement v, bool clamp)
    {
        switch (field)
        {
            case "x": return (t.X = Real(v, field, clamp)) is not null;
            case "y": return (t.Y = Real(v, field, clamp)) is not null;
            case "threshold": return (t.Threshold = Real(v, field, clamp)) is not null;
            case "value": return (t.Value = Real(v, field, clamp)) is not null;
            case "tolerance": return (t.Tolerance = Int(v, field, clamp)) is not null;
            case "colour": return (t.Colour = Colour(v)) is not null;
            case "region": return (t.Region = Region(v, clamp)) is not null;
            case "words": return (t.Words = Words(v)) is not null;
            default: return false;
        }
    }

    static void Clear(MacroStep s, string field)
    {
        switch (field)
        {
            case "x": s.X = null; break;
            case "y": s.Y = null; break;
            case "threshold": s.Threshold = null; break;
            case "value": s.Value = null; break;
            case "ms": s.Ms = null; break;
            case "dx": s.Dx = null; break;
            case "dy": s.Dy = null; break;
            case "notches": s.Notches = null; break;
            case "timeoutMs": s.TimeoutMs = null; break;
            case "tolerance": s.Tolerance = null; break;
            case "times": s.Times = null; break;
            case "max": s.Max = null; break;
            case "fps": s.Fps = null; break;
            case "holdMs": s.HoldMs = null; break;
            case "minSize": s.MinSize = null; break;
            case "repeatUntilGoneMs": s.RepeatUntilGoneMs = null; break;
            case "eachAfterMs": s.EachAfterMs = null; break;
            case "words": s.Words = null; break;
            case "kp": s.Kp = null; break;
            case "kd": s.Kd = null; break;
            case "untilGoneMs": s.UntilGoneMs = null; break;
            case "colour": s.Colour = null; break;
            case "target": s.Target = null; break;
            case "region": s.Region = null; break;
            case "key": s.Key = null; break;
        }
    }

    static void Clear(MacroTest t, string field)
    {
        switch (field)
        {
            case "x": t.X = null; break;
            case "y": t.Y = null; break;
            case "threshold": t.Threshold = null; break;
            case "value": t.Value = null; break;
            case "tolerance": t.Tolerance = null; break;
            case "colour": t.Colour = null; break;
            case "region": t.Region = null; break;
            case "words": t.Words = null; break;
        }
    }
}
