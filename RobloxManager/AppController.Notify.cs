namespace RobloxManager;

/// <summary>
/// Events that matter while the person isn't looking (an account dropped, auto-rejoin gave up, low RAM) also go out as
/// Windows notifications when the window is hidden, minimised or behind another program.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan NotifyRepeat = TimeSpan.FromMinutes(5);
    /// <summary>When each event type was last shown per account, so a rejoin loop can't spam.</summary>
    readonly Dictionary<string, DateTime> _notified = new();

    /// <summary>Shows a Windows notification (text, kind) unless the window is in front; returns whether it did.</summary>
    public Func<string, string, bool>? ShowNotification { get; init; }

    /// <summary>
    /// The usual in-app toast, plus a Windows notification while the window isn't in front — at most one per account
    /// and event type every five minutes.
    /// </summary>
    /// <param name="toast">false when the page shows its own message for this (a launch result).</param>
    void Notify(string text, string kind, string type, Guid account = default, bool toast = true)
    {
        if (toast)
            Toast(text, kind);
        if (_disposed || !Prefs.Notifications || ShowNotification is null)
            return;
        var now = DateTime.UtcNow;
        string key = $"{type}:{account}";
        if (_notified.TryGetValue(key, out var last) && now - last < NotifyRepeat)
            return;
        if (_notified.Count > 256)
        {
            foreach (string stale in _notified.Where(n => now - n.Value >= NotifyRepeat).Select(n => n.Key).ToList())
                _notified.Remove(stale);
        }
        if (ShowNotification(text, kind))
            _notified[key] = now;
    }

    object? SetNotifications(bool enabled)
    {
        Prefs.Notifications = enabled;
        Prefs.Save();
        PushState();
        return null;
    }
}
