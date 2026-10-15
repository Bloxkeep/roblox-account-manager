using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Events that matter while the person isn't looking (an account dropped, auto-rejoin slowed down, a scheduled preset,
/// a sign-in to renew, low RAM, a launch that failed) also go out as Windows notifications when the window is
/// hidden, minimised or behind another program. Settings has one switch for all of them and one per kind. The same
/// events feed Discord (AppController.Discord.cs), with switches of their own.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan NotifyRepeat = TimeSpan.FromMinutes(5);
    /// <summary>When each event type was last shown per account, so a rejoin loop can't spam.</summary>
    readonly Dictionary<string, DateTime> _notified = new();
    /// <summary>The kinds Settings offers, each covering the event types <see cref="NotifyKind"/> maps to it.</summary>
    static readonly string[] NotifyKinds = ["drop", "schedule", "signin", "ram", "launch"];

    /// <summary>Shows a Windows notification (text, kind) unless the window is in front; returns whether it did.</summary>
    public Func<string, string, bool>? ShowNotification { get; init; }

    /// <summary>The Settings kind an event type belongs to; null for one no switch covers (always shown).</summary>
    static string? NotifyKind(string type) => type switch
    {
        "drop" or "rejoinFail" or "rejoinSlow" or "afk" => "drop",
        "signin" or "banned" => "signin",
        "ram" => "ram",
        "launch" => "launch",
        _ when type.StartsWith("schedule", StringComparison.Ordinal) => "schedule",
        _ => null,
    };

    /// <summary>
    /// The usual in-app toast, plus a Windows notification while the window isn't in front — at most one per account
    /// and event type every five minutes, and only for the kinds left on in Settings (the toast shows either way).
    /// </summary>
    /// <param name="toast">false when the page shows its own message for this (a launch result).</param>
    void Notify(string text, string kind, string type, Guid account = default, bool toast = true)
    {
        // Statistics › Activity keeps the text in both languages: like Discord's copy, before any other L.F.
        NoteNotified(text, kind, type, account);
        // First, while the L.F that made the text is still the last one (Discord may want it in another language, or
        // with account names shown or hidden by its own switch).
        NotifyDiscord(DiscordText(text), type, account);
        if (toast)
            Toast(text, kind);
        if (_disposed || !Prefs.Notifications || ShowNotification is null)
            return;
        if (NotifyKind(type) is { } off && Prefs.NotifyOff.Contains(off))
            return;
        var now = DateTime.UtcNow;
        string key = $"{type}:{account}";
        // A time ahead of now (the clock was set back) doesn't hold the notification back.
        if (_notified.TryGetValue(key, out var last) && last <= now && now - last < NotifyRepeat)
            return;
        if (_notified.Count > 256)
        {
            foreach (string stale in _notified.Where(n => now - n.Value >= NotifyRepeat).Select(n => n.Key).ToList())
                _notified.Remove(stale);
        }
        if (ShowNotification(text, kind))
            _notified[key] = now;
    }

    /// <summary>
    /// The questions that offer "Don't ask again": ones asked often whose yes can be undone (a preset deleted, windows
    /// closed, a launch despite a warning, Multi-Roblox switched on, a form's typing left). Deleting accounts, restoring
    /// a backup and removing the Discord link always ask.
    /// </summary>
    static readonly string[] SkippableConfirms = ["deletePreset", "closeAll", "killAll", "openAnyway", "multi", "discard"];

    /// <summary>A question answered "Don't ask again": from now on its yes is assumed.</summary>
    public object? SkipConfirm(string key)
    {
        if (!SkippableConfirms.Contains(key))
            throw new UserError(L.T("Unknown question."));
        if (Prefs.SkipConfirm.Add(key))
        {
            Prefs.Save();
            PushState();
        }
        return null;
    }

    /// <summary>{enabled} for the switch over all of them; {kind, enabled} for one kind.</summary>
    object? SetNotifications(JsonElement a)
    {
        bool enabled = Bool(a, "enabled");
        if (OptStr(a, "kind") is { } kind)
        {
            if (!NotifyKinds.Contains(kind))
                throw new UserError(L.T("Unknown notification kind."));
            if (enabled)
                Prefs.NotifyOff.Remove(kind);
            else
                Prefs.NotifyOff.Add(kind);
        }
        else
            Prefs.Notifications = enabled;
        Prefs.Save();
        PushState();
        return null;
    }
}
