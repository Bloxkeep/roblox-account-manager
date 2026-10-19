using System.Buffers.Binary;
using System.IO.Pipes;
using System.Text;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// "Playing Bloxkeep" on the person's Discord profile (Rich Presence): Bloxkeep talks to the Discord app running on
/// this PC through its local pipe (discord-ipc-0 to 9), the way games do. No server and no token: an application id
/// is public. While Discord isn't running nothing happens, and it is tried again a little later.
/// </summary>
public sealed class DiscordPresence : IDisposable
{
    /// <summary>What the profile shows; the application's icon and name come with it.</summary>
    public sealed record Activity(string Details, string? State, long StartUnix, string ButtonLabel, string ButtonUrl);

    /// <summary>The tests' stand-in for Discord's pipe; Discord's own otherwise.</summary>
    static readonly string? TestPipe = Environment.GetEnvironmentVariable("BLOXKEEP_DISCORD_PIPE") is { Length: > 0 } p ? p : null;
    static readonly TimeSpan RetryAfter = TimeSpan.FromSeconds(30), AnswerWithin = TimeSpan.FromSeconds(3);

    readonly SemaphoreSlim _gate = new(1, 1);
    NamedPipeClientStream? _pipe;
    string? _clientId, _sent;
    DateTime _nextTry;
    bool _disposed;

    /// <summary>Shows <paramref name="activity"/> (null clears it) for application <paramref name="clientId"/>; the same one twice is sent once.</summary>
    public async Task SetAsync(Activity? activity, string clientId)
    {
        string json = activity is null ? "null" : ActivityJson(activity);
        if (_disposed || (json == _sent && clientId == _clientId) || !await _gate.WaitAsync(0))
            return;
        try
        {
            if (_pipe is null || clientId != _clientId)
            {
                if (activity is null || DateTime.UtcNow < _nextTry)
                    return;
                Close();
                if (!await ConnectAsync(clientId))
                {
                    _nextTry = DateTime.UtcNow + RetryAfter;
                    return;
                }
            }
            string nonce = Guid.NewGuid().ToString();
            await WriteAsync(1, $$"""{"cmd":"SET_ACTIVITY","args":{"pid":{{Environment.ProcessId}},"activity":{{json}}},"nonce":"{{nonce}}"}""");
            await ReadAsync(); // its answer, so the pipe never fills up
            _sent = json;
        }
        catch (Exception ex) when (ex is IOException or OperationCanceledException or ObjectDisposedException or JsonException or InvalidOperationException)
        {
            // Discord closed or answered something unexpected: connected again on a later try.
            Close();
            _nextTry = DateTime.UtcNow + RetryAfter;
        }
        finally
        {
            _gate.Release();
        }
    }

    /// <summary>The activity as Discord's SET_ACTIVITY takes it: lines of 2 to 128 characters, one link button.</summary>
    internal static string ActivityJson(Activity a)
    {
        static string Fit(string s) => s.Length > 128 ? s[..127] + "…" : s.Length < 2 ? s.PadRight(2) : s;
        var activity = new Dictionary<string, object?>
        {
            ["details"] = Fit(a.Details),
            ["timestamps"] = new { start = a.StartUnix },
            ["assets"] = new { large_text = "Bloxkeep" },
            ["buttons"] = new[] { new { label = a.ButtonLabel.Length > 32 ? a.ButtonLabel[..32] : a.ButtonLabel, url = a.ButtonUrl } },
        };
        if (a.State is { Length: > 0 } state)
            activity["state"] = Fit(state);
        return JsonSerializer.Serialize(activity);
    }

    async Task<bool> ConnectAsync(string clientId)
    {
        foreach (string name in TestPipe is { } test ? [test] : Enumerable.Range(0, 10).Select(i => $"discord-ipc-{i}"))
        {
            var pipe = new NamedPipeClientStream(".", name, PipeDirection.InOut, PipeOptions.Asynchronous);
            try
            {
                using var cts = new CancellationTokenSource(500);
                await pipe.ConnectAsync(cts.Token);
                _pipe = pipe;
                await WriteAsync(0, JsonSerializer.Serialize(new { v = 1, client_id = clientId }));
                var (op, body) = await ReadAsync();
                // READY, or a close with the reason (an application id Discord doesn't know).
                if (op != 1 || !body.Contains("\"READY\"", StringComparison.Ordinal))
                {
                    Log.Note($"Discord presence: Discord didn't accept the application (op {op}).");
                    Close();
                    return false;
                }
                _clientId = clientId;
                return true;
            }
            catch (Exception ex) when (ex is IOException or OperationCanceledException or TimeoutException or UnauthorizedAccessException)
            {
                pipe.Dispose();
                _pipe = null;
            }
        }
        return false;
    }

    async Task WriteAsync(int op, string json)
    {
        byte[] body = Encoding.UTF8.GetBytes(json);
        byte[] frame = new byte[8 + body.Length];
        BinaryPrimitives.WriteInt32LittleEndian(frame, op);
        BinaryPrimitives.WriteInt32LittleEndian(frame.AsSpan(4), body.Length);
        body.CopyTo(frame, 8);
        using var cts = new CancellationTokenSource(AnswerWithin);
        await _pipe!.WriteAsync(frame, cts.Token);
        await _pipe.FlushAsync(cts.Token);
    }

    async Task<(int Op, string Body)> ReadAsync()
    {
        using var cts = new CancellationTokenSource(AnswerWithin);
        byte[] head = new byte[8];
        await _pipe!.ReadExactlyAsync(head, cts.Token);
        int op = BinaryPrimitives.ReadInt32LittleEndian(head), length = BinaryPrimitives.ReadInt32LittleEndian(head.AsSpan(4));
        if (length is < 0 or > 1 << 20)
            throw new InvalidOperationException("Discord sent a frame of an unexpected size.");
        byte[] body = new byte[length];
        await _pipe.ReadExactlyAsync(body, cts.Token);
        return (op, Encoding.UTF8.GetString(body));
    }

    void Close()
    {
        try
        {
            _pipe?.Dispose();
        }
        catch (IOException)
        {
        }
        _pipe = null;
        _clientId = _sent = null;
    }

    /// <summary>Quitting: the profile stops showing Bloxkeep (closing the pipe clears it on Discord's side too).</summary>
    public void Dispose()
    {
        _disposed = true;
        Close();
    }
}
