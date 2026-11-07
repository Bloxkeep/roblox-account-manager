using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Text in a window's picture, read by Windows' own text recognition (Windows.Media.Ocr: offline, built into Windows 10
/// and 11), in English and, when Windows has its language pack, Vietnamese. Called through the WinRT objects' COM
/// tables directly (interface ids and method order from Windows' own metadata, C:\Windows\System32\WinMetadata): the
/// .NET projection a windows10 target framework brings would add 57 MB to the app. Runs on a worker thread.
/// </summary>
static unsafe class MacroOcr
{
    /// <summary>A word read, and where (pixels of the picture given).</summary>
    public readonly record struct Word(string Text, RectangleF Box);

    /// <summary>A line read: its words in order.</summary>
    public sealed record Line(IReadOnlyList<Word> Words)
    {
        public string Text => string.Join(" ", Words.Select(w => w.Text));
    }

    static readonly Guid IID_IOcrEngineStatics = new("5BFFA85A-3384-3540-9940-699120D428A8");
    static readonly Guid IID_ILanguageFactory = new("9B0252AC-0C27-44F8-B792-9793FB66C63E");
    static readonly Guid IID_ISoftwareBitmapStatics = new("DF0385DB-672F-4A9D-806E-C2442F343E86");
    static readonly Guid IID_ICryptographicBufferStatics = new("320B7E22-3CB0-4CDF-8663-1D28910065EB");
    static readonly Guid IID_IAsyncInfo = new("00000036-0000-0000-C000-000000000046");
    const int BitmapPixelFormatBgra8 = 87;
    const int AsyncStarted = 0, AsyncCompleted = 1;
    /// <summary>OcrEngine.MaxImageDimension on Windows 10 and 11.</summary>
    const int MaxSide = 2600;

    [DllImport("combase.dll")] static extern int RoInitialize(int type);
    [DllImport("combase.dll")] static extern int RoGetActivationFactory(IntPtr classId, in Guid iid, out IntPtr factory);
    [DllImport("combase.dll", CharSet = CharSet.Unicode)] static extern int WindowsCreateString(string text, int length, out IntPtr hstring);
    [DllImport("combase.dll")] static extern int WindowsDeleteString(IntPtr hstring);
    [DllImport("combase.dll")] static extern char* WindowsGetStringRawBuffer(IntPtr hstring, out uint length);

    [ThreadStatic] static bool _initialised;
    static readonly object Lock = new();
    static List<IntPtr>? _engines;
    static bool _tried;
    /// <summary>error.log: the last unfinished recognition status written.</summary>
    static int _loggedStatus = AsyncCompleted;

    /// <summary>Windows can read text here (some editions lack every recognition language).</summary>
    public static bool Available
    {
        get
        {
            try
            {
                return Engines().Count > 0;
            }
            catch (Exception ex) when (ex is COMException or DllNotFoundException or EntryPointNotFoundException)
            {
                Log.Note($"Macro text reading: Windows' text recognition didn't start ({ex.GetType().Name}).");
                return false;
            }
        }
    }

    /// <summary>
    /// The lines read in <paramref name="area"/> of <paramref name="picture"/> by each language's engine, boxes in the
    /// picture's pixels; null when Windows has no text recognition. Small text is enlarged first (it reads better).
    /// </summary>
    public static List<List<Line>>? Read(Bitmap picture, Rectangle area)
    {
        area = Rectangle.Intersect(area, new Rectangle(0, 0, picture.Width, picture.Height));
        if (area.Width < 8 || area.Height < 8)
            return [];
        var engines = Engines();
        if (engines.Count == 0)
            return null;
        double scale = Math.Min(Math.Max(1, 1200.0 / Math.Max(area.Width, area.Height)), 3);
        scale = Math.Min(scale, (double)MaxSide / Math.Max(area.Width, area.Height));
        int w = Math.Max(8, (int)(area.Width * scale)), h = Math.Max(8, (int)(area.Height * scale));
        using var cut = new Bitmap(w, h, PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(cut))
        {
            g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBicubic;
            g.DrawImage(picture, new Rectangle(0, 0, w, h), area, GraphicsUnit.Pixel);
        }
        var bytes = Pixels(cut);
        var result = new List<List<Line>>();
        foreach (var engine in engines)
        {
            var lines = Recognize(engine, bytes, w, h);
            // Back to the picture's pixels.
            result.Add(lines.Select(l => new Line(l.Words.Select(word => word with
            {
                Box = new RectangleF(area.X + word.Box.X / (float)scale, area.Y + word.Box.Y / (float)scale, word.Box.Width / (float)scale, word.Box.Height / (float)scale),
            }).ToList())).ToList());
        }
        return result;
    }

    /// <summary>
    /// Where <paramref name="text"/> is (case and spacing aside) in what was read: the box around its words, or an empty
    /// box when it runs over several lines. Null when it isn't there.
    /// </summary>
    public static RectangleF? Locate(IEnumerable<Line> lines, string text)
    {
        string want = Squeeze(text);
        if (want.Length == 0)
            return null;
        var all = lines.ToList();
        foreach (var line in all)
        {
            // Each word's place in the line's text, to box only the words the text covers.
            var spans = new List<(int Start, int End, RectangleF Box)>();
            var joined = new System.Text.StringBuilder();
            foreach (var word in line.Words)
            {
                if (joined.Length > 0)
                    joined.Append(' ');
                int start = joined.Length;
                joined.Append(Squeeze(word.Text));
                spans.Add((start, joined.Length, word.Box));
            }
            int at = joined.ToString().IndexOf(want, StringComparison.OrdinalIgnoreCase);
            if (at < 0)
                continue;
            var boxes = spans.Where(s => s.End > at && s.Start < at + want.Length).Select(s => s.Box).ToList();
            return boxes.Count == 0 ? RectangleF.Empty : boxes.Aggregate(RectangleF.Union);
        }
        return Squeeze(string.Join(" ", all.Select(l => l.Text))).Contains(want, StringComparison.OrdinalIgnoreCase) ? RectangleF.Empty : null;
    }

    /// <summary>
    /// Which of <paramref name="words"/> is in what was read, and where: line by line, case and spacing aside, a letter
    /// read wrong forgiven in longer words (two from 10 letters; game fonts). <paramref name="all"/> is the whole list the
    /// words were ticked from: a longer name read ("Blessed Song") is not a shorter one in it ("Blessed").
    /// </summary>
    public static (RectangleF Box, string Word)? LocateAny(IEnumerable<Line> lines, IReadOnlyCollection<string> words, IEnumerable<string> all)
    {
        var names = all.Concat(words).Select(Squeeze).Where(w => w.Length > 0).Distinct(StringComparer.OrdinalIgnoreCase).ToList();
        foreach (var line in lines)
        {
            string text = Squeeze(line.Text);
            var hits = names.Where(n => Near(text, n)).ToList();
            hits.RemoveAll(h => hits.Any(o => o.Length > h.Length && o.Contains(h, StringComparison.OrdinalIgnoreCase)));
            foreach (string hit in hits)
            {
                if (words.FirstOrDefault(w => Squeeze(w).Equals(hit, StringComparison.OrdinalIgnoreCase)) is { } word)
                    return (Locate([line], word) is { IsEmpty: false } box ? box : line.Words.Select(w => w.Box).Aggregate(RectangleF.Union), word);
            }
        }
        return null;
    }

    /// <summary>The word somewhere in the text, with at most one letter wrong from 5 letters, two from 10.</summary>
    internal static bool Near(string text, string word)
    {
        if (text.Contains(word, StringComparison.OrdinalIgnoreCase))
            return true;
        int allowed = word.Length >= 10 ? 2 : word.Length >= 5 ? 1 : 0;
        if (allowed == 0)
            return false;
        // The fewest changes that make the word match some part of the text (it may start anywhere).
        string t = text.ToLowerInvariant(), w = word.ToLowerInvariant();
        var prev = new int[w.Length + 1];
        var cur = new int[w.Length + 1];
        for (int j = 0; j <= w.Length; j++)
            prev[j] = j;
        foreach (char c in t)
        {
            cur[0] = 0;
            for (int j = 1; j <= w.Length; j++)
                cur[j] = Math.Min(Math.Min(prev[j - 1] + (c == w[j - 1] ? 0 : 1), prev[j] + 1), cur[j - 1] + 1);
            if (cur[w.Length] <= allowed)
                return true;
            (prev, cur) = (cur, prev);
        }
        return false;
    }

    /// <summary>Runs of spaces as one, ends trimmed.</summary>
    internal static string Squeeze(string text) => string.Join(' ', text.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries));

    static byte[] Pixels(Bitmap bitmap)
    {
        var data = bitmap.LockBits(new Rectangle(0, 0, bitmap.Width, bitmap.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        try
        {
            var bytes = new byte[bitmap.Width * bitmap.Height * 4];
            for (int y = 0; y < bitmap.Height; y++)
                Marshal.Copy(data.Scan0 + y * data.Stride, bytes, y * bitmap.Width * 4, bitmap.Width * 4);
            for (int i = 3; i < bytes.Length; i += 4)
                bytes[i] = 255; // a window's picture has no transparency; its alpha byte may be anything
            return bytes;
        }
        finally
        {
            bitmap.UnlockBits(data);
        }
    }

    static void Initialise()
    {
        if (_initialised)
            return;
        // The worker joins the multithreaded apartment (S_FALSE: it already had; a changed mode is fine too).
        RoInitialize(1);
        _initialised = true;
    }

    /// <summary>English first, then Vietnamese; the person's own languages when neither is installed. Made once.</summary>
    static List<IntPtr> Engines()
    {
        lock (Lock)
        {
            if (_tried)
                return _engines ?? [];
            _tried = true;
            Initialise();
            var engines = new List<IntPtr>();
            IntPtr statics = Factory("Windows.Media.Ocr.OcrEngine", IID_IOcrEngineStatics);
            try
            {
                foreach (string tag in new[] { "en-US", "vi" })
                {
                    IntPtr language = Language(tag);
                    if (language == IntPtr.Zero)
                        continue;
                    try
                    {
                        IntPtr engine;
                        if (Call(statics, 9, language, &engine) >= 0 && engine != IntPtr.Zero)
                            engines.Add(engine);
                    }
                    finally
                    {
                        Release(language);
                    }
                }
                if (engines.Count == 0)
                {
                    IntPtr engine;
                    if (((delegate* unmanaged[Stdcall]<IntPtr, IntPtr*, int>)Slot(statics, 10))(statics, &engine) >= 0 && engine != IntPtr.Zero)
                        engines.Add(engine);
                }
            }
            finally
            {
                Release(statics);
            }
            _engines = engines;
            Log.Note($"Macro text reading: {engines.Count} recognition language(s) ready.");
            return engines;
        }
    }

    static IntPtr Language(string tag)
    {
        IntPtr factory = Factory("Windows.Globalization.Language", IID_ILanguageFactory);
        try
        {
            using var name = new HString(tag);
            IntPtr language;
            return Call(factory, 6, name.Handle, &language) >= 0 ? language : IntPtr.Zero;
        }
        finally
        {
            Release(factory);
        }
    }

    static List<Line> Recognize(IntPtr engine, byte[] bgra, int w, int h)
    {
        Initialise();
        IntPtr buffer = IntPtr.Zero, bitmap = IntPtr.Zero, operation = IntPtr.Zero, info = IntPtr.Zero, result = IntPtr.Zero;
        try
        {
            IntPtr cryptography = Factory("Windows.Security.Cryptography.CryptographicBuffer", IID_ICryptographicBufferStatics);
            try
            {
                fixed (byte* p = bgra)
                    Check(((delegate* unmanaged[Stdcall]<IntPtr, uint, byte*, IntPtr*, int>)Slot(cryptography, 9))(cryptography, (uint)bgra.Length, p, &buffer));
            }
            finally
            {
                Release(cryptography);
            }
            IntPtr bitmaps = Factory("Windows.Graphics.Imaging.SoftwareBitmap", IID_ISoftwareBitmapStatics);
            try
            {
                Check(((delegate* unmanaged[Stdcall]<IntPtr, IntPtr, int, int, int, IntPtr*, int>)Slot(bitmaps, 9))(bitmaps, buffer, BitmapPixelFormatBgra8, w, h, &bitmap));
            }
            finally
            {
                Release(bitmaps);
            }
            Check(Call(engine, 6, bitmap, &operation));
            Check(Marshal.QueryInterface(operation, in IID_IAsyncInfo, out info));
            int status = AsyncStarted;
            var waited = System.Diagnostics.Stopwatch.StartNew();
            while (true)
            {
                Check(((delegate* unmanaged[Stdcall]<IntPtr, int*, int>)Slot(info, 7))(info, &status));
                if (status != AsyncStarted || waited.Elapsed > TimeSpan.FromSeconds(10))
                    break;
                Thread.Sleep(5);
            }
            if (status != AsyncCompleted)
            {
                // Once per status: a read every second would repeat it.
                if (Interlocked.Exchange(ref _loggedStatus, status) != status)
                    Log.Note($"Macro text reading: Windows' recognition ended with status {status} after {waited.ElapsedMilliseconds} ms; nothing read.");
                ((delegate* unmanaged[Stdcall]<IntPtr, int>)Slot(info, 9))(info); // Cancel
                return [];
            }
            Check(((delegate* unmanaged[Stdcall]<IntPtr, IntPtr*, int>)Slot(operation, 8))(operation, &result));
            return Lines(result);
        }
        finally
        {
            if (info != IntPtr.Zero)
                ((delegate* unmanaged[Stdcall]<IntPtr, int>)Slot(info, 10))(info); // Close
            Release(result);
            Release(info);
            Release(operation);
            Release(bitmap);
            Release(buffer);
        }
    }

    static List<Line> Lines(IntPtr result)
    {
        var lines = new List<Line>();
        foreach (IntPtr line in Items(Get(result, 6)))
        {
            try
            {
                var words = new List<Word>();
                foreach (IntPtr word in Items(Get(line, 6)))
                {
                    try
                    {
                        RectangleF box;
                        Check(((delegate* unmanaged[Stdcall]<IntPtr, RectangleF*, int>)Slot(word, 6))(word, &box));
                        words.Add(new Word(Text(word, 7), box));
                    }
                    finally
                    {
                        Release(word);
                    }
                }
                lines.Add(new Line(words));
            }
            finally
            {
                Release(line);
            }
        }
        return lines;
    }

    /// <summary>The items of an IVectorView (the view itself released).</summary>
    static List<IntPtr> Items(IntPtr view)
    {
        var items = new List<IntPtr>();
        try
        {
            uint size;
            Check(((delegate* unmanaged[Stdcall]<IntPtr, uint*, int>)Slot(view, 7))(view, &size));
            for (uint i = 0; i < size; i++)
            {
                IntPtr item;
                Check(((delegate* unmanaged[Stdcall]<IntPtr, uint, IntPtr*, int>)Slot(view, 6))(view, i, &item));
                items.Add(item);
            }
        }
        finally
        {
            Release(view);
        }
        return items;
    }

    /// <summary>A property that is an object (its getter at <paramref name="slot"/>).</summary>
    static IntPtr Get(IntPtr obj, int slot)
    {
        IntPtr value;
        Check(((delegate* unmanaged[Stdcall]<IntPtr, IntPtr*, int>)Slot(obj, slot))(obj, &value));
        return value;
    }

    static string Text(IntPtr obj, int slot)
    {
        IntPtr hstring;
        Check(((delegate* unmanaged[Stdcall]<IntPtr, IntPtr*, int>)Slot(obj, slot))(obj, &hstring));
        try
        {
            char* chars = WindowsGetStringRawBuffer(hstring, out uint length);
            return chars == null ? "" : new string(chars, 0, (int)length);
        }
        finally
        {
            WindowsDeleteString(hstring);
        }
    }

    static IntPtr Factory(string className, Guid iid)
    {
        using var name = new HString(className);
        Check(RoGetActivationFactory(name.Handle, in iid, out IntPtr factory));
        return factory;
    }

    static IntPtr Slot(IntPtr obj, int slot) => (*(IntPtr**)obj)[slot];

    static int Call(IntPtr obj, int slot, IntPtr arg, IntPtr* result) =>
        ((delegate* unmanaged[Stdcall]<IntPtr, IntPtr, IntPtr*, int>)Slot(obj, slot))(obj, arg, result);

    static void Release(IntPtr obj)
    {
        if (obj != IntPtr.Zero)
            Marshal.Release(obj);
    }

    static void Check(int hr)
    {
        if (hr < 0)
            Marshal.ThrowExceptionForHR(hr);
    }

    sealed class HString : IDisposable
    {
        public IntPtr Handle { get; }

        public HString(string text)
        {
            Check(WindowsCreateString(text, text.Length, out IntPtr handle));
            Handle = handle;
        }

        public void Dispose() => WindowsDeleteString(Handle);
    }
}
