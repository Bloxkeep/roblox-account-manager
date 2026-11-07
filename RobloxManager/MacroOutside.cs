using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>What a macro reads from or writes to outside the game: the clipboard, a text file in its own folder, a web
/// address. Its own interface, so the checks swap it for one that touches nothing.</summary>
interface IMacroOutside
{
    Task<string?> ClipboardAsync();
    Task CopyAsync(string text);
    /// <summary>The file's last line that isn't empty; null when there's no such file.</summary>
    Task<string?> ReadFileAsync(string name);
    /// <summary>Adds a line at the end of the file (made when it isn't there).</summary>
    Task AppendFileAsync(string name, string line);
    /// <summary>GET (post null) or POST {"content": post}; the answer's text, or null when the address refused.</summary>
    Task<string?> WebAsync(string url, string? post, CancellationToken ct);
}

/// <summary>
/// The real one. Files live in Documents\Bloxkeep only, named by the step ("coins log" → "coins log.txt"): a macro shared
/// as a file can't write anywhere else. Web addresses are https and public only (never this PC or the home network), with a
/// short timeout and a small answer kept.
/// </summary>
sealed class MacroOutside : IMacroOutside
{
    public const int MaxRead = 2000;
    static readonly HttpClient Http = new(new SocketsHttpHandler
    {
        AllowAutoRedirect = false,
        ConnectCallback = ConnectPublicAsync,
    }) { Timeout = TimeSpan.FromSeconds(8) };

    public static string Folder => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "Bloxkeep");

    /// <summary>A file's name as a step may give it: letters, digits, spaces, - and _, up to 40.</summary>
    internal static readonly Regex FileName = new(@"^[\p{L}\p{N} _\-]{1,40}$", RegexOptions.CultureInvariant);

    static string PathOf(string name)
    {
        if (!FileName.IsMatch(name.Trim()))
            throw new MacroExprError(L.T("A file's name is letters, digits, spaces, - and _."));
        return Path.Combine(Folder, name.Trim() + ".txt");
    }

    public Task<string?> ClipboardAsync() => Sta<string?>(() => Clipboard.ContainsText() ? Clipboard.GetText() : "");

    public Task CopyAsync(string text) => Sta<object?>(() =>
    {
        if (text.Length == 0)
            Clipboard.Clear();
        else
            Clipboard.SetDataObject(text, true, 5, 100);
        return null;
    });

    public async Task<string?> ReadFileAsync(string name)
    {
        string path = PathOf(name);
        if (!File.Exists(path))
            return null;
        var lines = await File.ReadAllLinesAsync(path);
        return lines.LastOrDefault(l => l.Trim().Length > 0)?.Trim() is { } last ? last[..Math.Min(last.Length, MaxRead)] : "";
    }

    public async Task AppendFileAsync(string name, string line)
    {
        string path = PathOf(name);
        Directory.CreateDirectory(Folder);
        // A log never grows past 5 MB: the older half goes.
        if (new FileInfo(path) is { Exists: true, Length: > 5 * 1024 * 1024 })
        {
            var all = await File.ReadAllLinesAsync(path);
            await File.WriteAllLinesAsync(path, all.Skip(all.Length / 2));
        }
        await File.AppendAllTextAsync(path, line.Replace('\r', ' ').Replace('\n', ' ') + Environment.NewLine);
    }

    public async Task<string?> WebAsync(string url, string? post, CancellationToken ct)
    {
        if (!Uri.TryCreate(url.Trim(), UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps)
            throw new MacroExprError(L.T("A web address starts with https://."));
        using var req = new HttpRequestMessage(post is null ? HttpMethod.Get : HttpMethod.Post, uri);
        req.Headers.UserAgent.ParseAdd("Bloxkeep-macro");
        if (post is not null)
            req.Content = new StringContent(JsonSerializer.Serialize(new { content = post }), Encoding.UTF8, "application/json");
        try
        {
            using var res = await Http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, ct);
            if (!res.IsSuccessStatusCode)
            {
                WebNote($"Macro web step: HTTP {(int)res.StatusCode} from {uri.Host}.");
                return null;
            }
            _lastWebNote = null;
            await using var body = await res.Content.ReadAsStreamAsync(ct);
            var buffer = new char[MaxRead];
            using var reader = new StreamReader(body, Encoding.UTF8);
            int n = await reader.ReadBlockAsync(buffer, ct);
            return new string(buffer, 0, n);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException)
        {
            WebNote($"Macro web step: {uri.Host} didn't answer ({ex.GetType().Name}).");
            return null;
        }
    }

    /// <summary>error.log: the last web failure written; a step in a loop would write the same one every few seconds.</summary>
    static string? _lastWebNote;

    static void WebNote(string text)
    {
        if (Interlocked.Exchange(ref _lastWebNote, text) != text)
            Log.Note(text);
    }

    /// <summary>Connects only to a public address: never this PC, the home network or a link-local one.</summary>
    static async ValueTask<Stream> ConnectPublicAsync(SocketsHttpConnectionContext context, CancellationToken ct)
    {
        var addresses = await Dns.GetHostAddressesAsync(context.DnsEndPoint.Host, ct);
        var target = addresses.FirstOrDefault(Public) ?? throw new HttpRequestException("Not a public address.");
        var socket = new Socket(target.AddressFamily, SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            await socket.ConnectAsync(new IPEndPoint(target, context.DnsEndPoint.Port), ct);
            return new NetworkStream(socket, ownsSocket: true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    internal static bool Public(IPAddress ip)
    {
        if (ip.IsIPv4MappedToIPv6)
            ip = ip.MapToIPv4();
        if (IPAddress.IsLoopback(ip) || ip.Equals(IPAddress.Any) || ip.Equals(IPAddress.IPv6Any))
            return false;
        if (ip.AddressFamily == AddressFamily.InterNetwork)
        {
            var b = ip.GetAddressBytes();
            return !(b[0] == 10 || b[0] == 0 || b[0] >= 224 || (b[0] == 172 && b[1] is >= 16 and <= 31) || (b[0] == 192 && b[1] == 168)
                || (b[0] == 169 && b[1] == 254) || (b[0] == 100 && b[1] is >= 64 and <= 127));
        }
        return !(ip.IsIPv6LinkLocal || ip.IsIPv6SiteLocal || ip.IsIPv6Multicast || ip.IsIPv6UniqueLocal);
    }

    static Task<T> Sta<T>(Func<T> work)
    {
        var done = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        var thread = new Thread(() =>
        {
            try
            {
                done.SetResult(work());
            }
            catch (Exception ex)
            {
                done.SetException(ex);
            }
        }) { IsBackground = true };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return done.Task;
    }
}
