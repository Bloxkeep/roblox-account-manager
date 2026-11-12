using System.Text.Json;

namespace RobloxManager;

// What only the GPUI interface (1.1, Rust) may ask: it calls some of Roblox's services itself now (the core moves to Rust
// bit by bit, the owner's call on 2026-11-09), and the ones that answer only a signed-in account (the charts' games, an
// account's friends) need an account's sign-in. It goes over the shell's own pipe to bloxkeep-ui, which keeps it in memory
// only; the web page's messages never reach this (ShellForm hands only the pipe's messages here).
public sealed partial class AppController
{
    /// <summary>A message from the GPUI window: its own commands, else the page's.</summary>
    public async Task HandleGpuiMessageAsync(string json)
    {
        int id;
        string cmd;
        Guid? wanted = null;
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            id = root.GetProperty("id").GetInt32();
            cmd = root.GetProperty("cmd").GetString() ?? "";
            // robloxSignIn { id }: that account's sign-in (Friends looks through one account); without it, the app's pick.
            if (root.TryGetProperty("args", out var args) && args.ValueKind == JsonValueKind.Object
                && args.TryGetProperty("id", out var one) && one.ValueKind == JsonValueKind.String && Guid.TryParse(one.GetString(), out var g))
                wanted = g;
        }
        catch (Exception ex) when (ex is JsonException or KeyNotFoundException or InvalidOperationException)
        {
            await HandleMessageAsync(json);
            return;
        }
        if (cmd != "robloxSignIn")
        {
            await HandleMessageAsync(json);
            return;
        }
        // The account asked for; else the main account when it has a good sign-in, else any roblox.com account's (as the
        // app's own calls use). Its ids go with it, so the window knows whose it holds.
        var from = _vault?.Data.Accounts
            .Where(a => a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global && (wanted is null || a.Id == wanted))
            .OrderByDescending(a => a.IsMain)
            .FirstOrDefault();
        Post(JsonSerializer.Serialize(new { type = "reply", id, ok = true, result = new { cookie = from?.Cookie, userId = from?.UserId ?? 0, id = from?.Id } }, Json));
    }
}
