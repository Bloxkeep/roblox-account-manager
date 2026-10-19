using System.Buffers.Binary;
using System.IO.Pipes;
using System.Text;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// "Playing Bloxkeep" on the Discord profile (DiscordPresence.cs): against a stand-in for the Discord app's pipe, the
/// handshake names the application, the activity says the windows and the game with a link and never twice the same,
/// and turning it off clears it. With no Discord running nothing breaks. No real Discord is touched.
/// </summary>
static class DiscordPresenceSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string pipeName = $"bloxkeep-test-discord-{Guid.NewGuid():N}";
        Environment.SetEnvironmentVariable("BLOXKEEP_DISCORD_PIPE", pipeName);

        // Nothing is listening yet: no Discord on this PC.
        using (var none = new DiscordPresence())
        {
            var quiet = System.Diagnostics.Stopwatch.StartNew();
            await none.SetAsync(new("1 Roblox window open", null, 1, "Get Bloxkeep", "https://example.com/"), "123456");
            c.Check("with no Discord running nothing breaks, and it doesn't hang", quiet.ElapsedMilliseconds < 3000, quiet.ElapsedMilliseconds);
        }

        using var server = new NamedPipeServerStream(pipeName, PipeDirection.InOut, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
        var frames = new List<(int Op, string Body)>();
        var served = Task.Run(async () =>
        {
            await server.WaitForConnectionAsync();
            while (true)
            {
                var frame = await Read(server);
                if (frame is null)
                    return;
                lock (frames)
                    frames.Add(frame.Value);
                await Write(server, 1, frame.Value.Op == 0 ? """{"cmd":"DISPATCH","evt":"READY","data":{}}""" : """{"cmd":"SET_ACTIVITY","data":null}""");
            }
        });

        using var presence = new DiscordPresence();
        var activity = new DiscordPresence.Activity("2 Roblox windows open", "Blox Fruits", 1_700_000_000, "Get Bloxkeep", "https://minhhoang0122.github.io/Bloxkeep/");
        await presence.SetAsync(activity, "123456");
        List<(int Op, string Body)> Seen() { lock (frames) return frames.ToList(); }
        var seen = Seen();
        c.Check("the handshake names Bloxkeep's application", seen.Count >= 1 && seen[0].Op == 0 && Json(seen[0].Body).GetProperty("client_id").GetString() == "123456",
            seen.FirstOrDefault().Body);
        var set = seen.Count >= 2 ? Json(seen[1].Body) : default;
        var sent = set.ValueKind == JsonValueKind.Object ? set.GetProperty("args").GetProperty("activity") : default;
        c.Check("the activity says the windows, the game and since when, with a link to Bloxkeep",
            set.ValueKind == JsonValueKind.Object && set.GetProperty("cmd").GetString() == "SET_ACTIVITY"
            && sent.GetProperty("details").GetString() == "2 Roblox windows open" && sent.GetProperty("state").GetString() == "Blox Fruits"
            && sent.GetProperty("timestamps").GetProperty("start").GetInt64() == 1_700_000_000
            && sent.GetProperty("buttons")[0].GetProperty("url").GetString() == "https://minhhoang0122.github.io/Bloxkeep/",
            seen.Count >= 2 ? seen[1].Body : "nothing sent");

        await presence.SetAsync(activity, "123456");
        c.Check("the same activity again isn't sent again", Seen().Count == 2, Seen().Count);

        await presence.SetAsync(null, "123456");
        var last = Seen().LastOrDefault();
        c.Check("turned off, the activity is cleared", Seen().Count == 3 && Json(last.Body).GetProperty("args").GetProperty("activity").ValueKind == JsonValueKind.Null, last.Body);

        presence.Dispose();
        await Task.WhenAny(served, Task.Delay(3000));
    }

    static JsonElement Json(string body) => JsonDocument.Parse(body).RootElement.Clone();

    static async Task<(int Op, string Body)?> Read(Stream pipe)
    {
        try
        {
            byte[] head = new byte[8];
            await pipe.ReadExactlyAsync(head);
            byte[] body = new byte[BinaryPrimitives.ReadInt32LittleEndian(head.AsSpan(4))];
            await pipe.ReadExactlyAsync(body);
            return (BinaryPrimitives.ReadInt32LittleEndian(head), Encoding.UTF8.GetString(body));
        }
        catch (Exception ex) when (ex is EndOfStreamException or IOException or ObjectDisposedException)
        {
            return null;
        }
    }

    static async Task Write(Stream pipe, int op, string json)
    {
        byte[] body = Encoding.UTF8.GetBytes(json), frame = new byte[8 + body.Length];
        BinaryPrimitives.WriteInt32LittleEndian(frame, op);
        BinaryPrimitives.WriteInt32LittleEndian(frame.AsSpan(4), body.Length);
        body.CopyTo(frame, 8);
        await pipe.WriteAsync(frame);
        await pipe.FlushAsync();
    }
}
