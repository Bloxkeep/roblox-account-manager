using System.Globalization;

namespace RobloxManager;

/// <summary>A formula that can't be worked out while a macro plays (a division by 0, a missing number).</summary>
sealed class MacroExprError(string message) : Exception(message);

/// <summary>
/// Formulas in a Set variable step: numbers, counters and settings by name ("coins", or "{coins}" as in an alert's
/// text), + - * / % ^, brackets, and a few functions: random(a, b), round(x), round(x, digits), floor, ceil, abs, min,
/// max, sqrt. Comparisons give 1 or 0 (coins >= 100), as do and, or, not. Parsed at once into a tree, so a mistake shows
/// when the macro is saved, not when it plays.
/// </summary>
static class MacroExpr
{
    public const int MaxLength = 200;

    abstract record Node;
    sealed record Num(double Value) : Node;
    sealed record Ref(string Name) : Node;
    sealed record Neg(Node Of) : Node;
    sealed record Not(Node Of) : Node;
    sealed record Bin(char Op, Node L, Node R) : Node;
    sealed record Cmp(string Op, Node L, Node R) : Node;
    sealed record Call(string Fn, List<Node> Args) : Node;

    static readonly Dictionary<string, (int Min, int Max)> Functions = new(StringComparer.OrdinalIgnoreCase)
    {
        ["random"] = (0, 2), ["round"] = (1, 2), ["floor"] = (1, 1), ["ceil"] = (1, 1), ["abs"] = (1, 1),
        ["min"] = (1, 20), ["max"] = (1, 20), ["sqrt"] = (1, 1),
    };

    /// <summary>The formula's mistake, near where it is, or null when it reads well.</summary>
    public static string? Problem(string text)
    {
        try
        {
            Parse(text);
            return null;
        }
        catch (FormatException ex)
        {
            return ex.Message;
        }
    }

    /// <summary>The names a formula reads (counters, settings).</summary>
    public static IEnumerable<string> Names(string text)
    {
        var found = new List<string>();
        try
        {
            Walk(Parse(text));
        }
        catch (FormatException)
        {
        }
        return found.Distinct();

        void Walk(Node n)
        {
            switch (n)
            {
                case Ref r: found.Add(r.Name); break;
                case Neg x: Walk(x.Of); break;
                case Not x: Walk(x.Of); break;
                case Bin b: Walk(b.L); Walk(b.R); break;
                case Cmp c: Walk(c.L); Walk(c.R); break;
                case Call c: c.Args.ForEach(Walk); break;
            }
        }
    }

    /// <summary>Works the formula out; <paramref name="value"/> gives a name's number (throws when there's none).</summary>
    public static double Eval(string text, Func<string, double> value, Random? random = null)
    {
        Node tree;
        try
        {
            tree = Parse(text);
        }
        catch (FormatException ex)
        {
            throw new MacroExprError(ex.Message);
        }
        double result = Run(tree, value, random ?? Random.Shared);
        if (!double.IsFinite(result))
            throw new MacroExprError(L.T("The math divides by 0 or gets too big."));
        return result;
    }

    static double Run(Node n, Func<string, double> value, Random random) => n switch
    {
        Num x => x.Value,
        Ref r => value(r.Name),
        Neg x => -Run(x.Of, value, random),
        Not x => Run(x.Of, value, random) == 0 ? 1 : 0,
        Bin { Op: '&' } b => Run(b.L, value, random) != 0 && Run(b.R, value, random) != 0 ? 1 : 0,
        Bin { Op: '|' } b => Run(b.L, value, random) != 0 || Run(b.R, value, random) != 0 ? 1 : 0,
        Bin b => Apply(b.Op, Run(b.L, value, random), Run(b.R, value, random)),
        Cmp c => MacroPlayer.Compare(Run(c.L, value, random), c.Op, Run(c.R, value, random)) ? 1 : 0,
        Call c => Function(c.Fn, c.Args.Select(a => Run(a, value, random)).ToList(), random),
        _ => 0,
    };

    static double Apply(char op, double a, double b) => op switch
    {
        '+' => a + b,
        '-' => a - b,
        '*' => a * b,
        '/' => b == 0 ? throw new MacroExprError(L.T("The math divides by 0 or gets too big.")) : a / b,
        // A remainder that is never below 0 (-1 % 5 = 4), as a counter going round wants.
        '%' => b == 0 ? throw new MacroExprError(L.T("The math divides by 0 or gets too big.")) : ((a % b) + b) % b,
        '^' => Math.Pow(a, b),
        _ => 0,
    };

    static double Function(string fn, List<double> a, Random random)
    {
        switch (fn.ToLowerInvariant())
        {
            case "random":
            {
                if (a.Count == 0)
                    return random.NextDouble();
                double lo = a.Count == 1 ? 1 : a[0], hi = a.Count == 1 ? a[0] : a[1];
                if (lo > hi)
                    (lo, hi) = (hi, lo);
                // Whole numbers both ends: a whole number between them, both included (random(1, 6) is a die).
                if (lo == Math.Floor(lo) && hi == Math.Floor(hi) && hi - lo < int.MaxValue)
                    return lo + random.NextInt64((long)(hi - lo) + 1);
                return lo + random.NextDouble() * (hi - lo);
            }
            case "round":
                return a.Count == 2 ? Math.Round(a[0], Math.Clamp((int)a[1], 0, 10), MidpointRounding.AwayFromZero)
                    : Math.Round(a[0], MidpointRounding.AwayFromZero);
            case "floor": return Math.Floor(a[0]);
            case "ceil": return Math.Ceiling(a[0]);
            case "abs": return Math.Abs(a[0]);
            case "min": return a.Min();
            case "max": return a.Max();
            case "sqrt": return a[0] < 0 ? throw new MacroExprError(L.T("The math divides by 0 or gets too big.")) : Math.Sqrt(a[0]);
            default: return 0;
        }
    }

    // ---- Reading -------------------------------------------------------------------------------------------------

    static Node Parse(string text)
    {
        if (text.Length > MaxLength)
            throw new FormatException(L.F("The math can be up to {0} characters.", MaxLength));
        var p = new Parser(text);
        var tree = p.Or();
        p.Skip();
        if (!p.End)
            throw p.Wrong();
        return tree;
    }

    sealed class Parser(string s)
    {
        int _at;
        int _depth;
        public bool End => _at >= s.Length;

        public void Skip()
        {
            while (_at < s.Length && char.IsWhiteSpace(s[_at]))
                _at++;
        }

        public FormatException Wrong()
        {
            if (End)
                return new FormatException(L.T("The math stops too soon."));
            string near = s[_at..Math.Min(s.Length, _at + 12)];
            return new FormatException(L.F("The math is wrong near “{0}”.", near));
        }

        bool Eat(string token)
        {
            Skip();
            if (string.CompareOrdinal(s, _at, token, 0, token.Length) != 0)
                return false;
            // A word (and, or, not) is a whole word only.
            if (char.IsLetter(token[0]) && _at + token.Length < s.Length && (char.IsLetterOrDigit(s[_at + token.Length]) || s[_at + token.Length] == '_'))
                return false;
            _at += token.Length;
            return true;
        }

        public Node Or()
        {
            var left = And();
            while (Eat("||") || Eat("or"))
                left = new Bin('|', left, And());
            return left;
        }

        Node And()
        {
            var left = Compare();
            while (Eat("&&") || Eat("and"))
                left = new Bin('&', left, Compare());
            return left;
        }

        Node Compare()
        {
            var left = Sum();
            foreach (var op in new[] { "<=", ">=", "!=", "<>", "==", "<", ">", "=" })
            {
                if (Eat(op))
                    return new Cmp(op switch { "<>" => "!=", "==" => "=", _ => op }, left, Sum());
            }
            return left;
        }

        Node Sum()
        {
            var left = Product();
            while (true)
            {
                if (Eat("+"))
                    left = new Bin('+', left, Product());
                else if (Eat("-"))
                    left = new Bin('-', left, Product());
                else
                    return left;
            }
        }

        Node Product()
        {
            var left = Unary();
            while (true)
            {
                if (Eat("*") || Eat("×"))
                    left = new Bin('*', left, Unary());
                else if (Eat("/") || Eat("÷"))
                    left = new Bin('/', left, Unary());
                else if (Eat("%"))
                    left = new Bin('%', left, Unary());
                else
                    return left;
            }
        }

        Node Unary()
        {
            if (++_depth > 40)
                throw new FormatException(L.T("The math has too many brackets."));
            try
            {
                if (Eat("-"))
                    return new Neg(Unary());
                if (Eat("+"))
                    return Unary();
                if (Eat("!") || Eat("not"))
                    return new Not(Unary());
                var atom = Atom();
                return Eat("^") ? new Bin('^', atom, Unary()) : atom;
            }
            finally
            {
                _depth--;
            }
        }

        Node Atom()
        {
            Skip();
            if (End)
                throw Wrong();
            char c = s[_at];
            if (c == '(')
            {
                _at++;
                var inner = Or();
                if (!Eat(")"))
                    throw Wrong();
                return inner;
            }
            if (c == '{')
            {
                _at++;
                string name = Word();
                if (name.Length == 0 || !Eat("}"))
                    throw Wrong();
                return new Ref(name);
            }
            if (char.IsDigit(c) || c == '.')
            {
                int start = _at;
                while (_at < s.Length && (char.IsDigit(s[_at]) || s[_at] == '.'))
                    _at++;
                if (!double.TryParse(s[start.._at], NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture, out double n))
                {
                    _at = start;
                    throw Wrong();
                }
                return new Num(n);
            }
            if (char.IsLetter(c) || c == '_')
            {
                int start = _at;
                string name = Word();
                if (Eat("("))
                {
                    if (!Functions.TryGetValue(name, out var count))
                    {
                        _at = start;
                        throw new FormatException(L.F("Can’t read “{0}”. It can use random, round, min, max.", name));
                    }
                    var args = new List<Node>();
                    if (!Eat(")"))
                    {
                        do
                            args.Add(Or());
                        while (Eat(","));
                        if (!Eat(")"))
                            throw Wrong();
                    }
                    if (args.Count < count.Min || args.Count > count.Max)
                        throw new FormatException(L.F("“{0}” takes {1} to {2} numbers.", name, count.Min, count.Max));
                    return new Call(name, args);
                }
                if (name.Length > 32)
                {
                    _at = start;
                    throw Wrong();
                }
                return new Ref(name);
            }
            throw Wrong();
        }

        string Word()
        {
            Skip();
            int start = _at;
            if (_at < s.Length && (char.IsLetter(s[_at]) || s[_at] == '_'))
            {
                _at++;
                while (_at < s.Length && (char.IsLetterOrDigit(s[_at]) || s[_at] == '_'))
                    _at++;
            }
            return s[start.._at];
        }
    }

    // ---- Numbers read off the screen ------------------------------------------------------------------------------

    static readonly System.Text.RegularExpressions.Regex NumberText = new(
        @"(?<sign>-)?(?<n>\d[\d,\.]*\d|\d)\s*(?<k>[kKmMbBtT](?![\p{L}]))?", System.Text.RegularExpressions.RegexOptions.CultureInvariant);

    /// <summary>
    /// The first number in a text read off the screen, as games write them: "1,234", "1.234.567", "12.5", "3.2K",
    /// "1.5M", "2B", "Coins: 450". A comma or dot followed by exactly three digits (and no other kind) groups thousands;
    /// otherwise the last one is the decimal point. Null when there's none.
    /// </summary>
    public static double? NumberIn(string text)
    {
        foreach (System.Text.RegularExpressions.Match m in NumberText.Matches(text))
        {
            string raw = m.Groups["n"].Value;
            if (Number(raw) is not { } n)
                continue;
            n *= m.Groups["k"].Value.ToLowerInvariant() switch
            {
                "k" => 1e3,
                "m" => 1e6,
                "b" => 1e9,
                "t" => 1e12,
                _ => 1,
            };
            return m.Groups["sign"].Success ? -n : n;
        }
        return null;
    }

    static double? Number(string raw)
    {
        int comma = raw.LastIndexOf(','), dot = raw.LastIndexOf('.');
        string digits;
        if (comma < 0 && dot < 0)
            digits = raw;
        else if (comma >= 0 && dot >= 0)
        {
            // Both: the later one is the decimal point.
            char point = comma > dot ? ',' : '.', group = point == ',' ? '.' : ',';
            digits = raw.Replace(group.ToString(), "").Replace(point, '.');
        }
        else
        {
            char mark = comma >= 0 ? ',' : '.';
            var parts = raw.Split(mark);
            bool grouped = parts.Skip(1).All(p => p.Length == 3) && (parts.Length > 2 || mark == ',');
            digits = grouped ? string.Concat(parts) : parts.Length == 2 ? parts[0] + "." + parts[1] : string.Concat(parts);
        }
        return double.TryParse(digits, NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture, out double n) ? n : null;
    }
}
