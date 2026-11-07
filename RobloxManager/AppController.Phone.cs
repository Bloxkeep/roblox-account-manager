using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Settings › Notifications › Phone: the same events as the Windows notifications (the same kinds switched off stay off),
/// plus a macro's alerts, sent to the phone through ntfy (<see cref="PhoneAlert"/>) whether the window is in front or not.
/// The topic is kept in the vault: whoever knows it reads the alerts.
/// </summary>
public sealed partial class AppController
{
    readonly Dictionary<string, DateTime> _phoned = new();
    string? _phoneError;

    /// <summary>From <see cref="Notify"/>: at most one per account and event type every five minutes.</summary>
    void NotifyPhone(string text, string kind, string type)
    {
        if (_disposed || _vault is not { Data.Phone: { Enabled: true, Topic.Length: > 0 } p })
            return;
        if (type != "macro" && NotifyKind(type) is { } off && Prefs.NotifyOff.Contains(off))
            return;
        var now = DateTime.UtcNow;
        string key = type + ":" + text;
        if (_phoned.TryGetValue(key, out var last) && last <= now && now - last < NotifyRepeat)
            return;
        if (_phoned.Count > 256)
        {
            foreach (string stale in _phoned.Where(n => now - n.Value >= NotifyRepeat).Select(n => n.Key).ToList())
                _phoned.Remove(stale);
        }
        _phoned[key] = now;
        string topic = p.Topic;
        _ = SendPhoneAsync(topic, text, urgent: kind is "warn" or "error");
    }

    async Task SendPhoneAsync(string topic, string text, bool urgent)
    {
        string? error = await PhoneAlert.SendAsync(topic, text, urgent);
        if (error is null && _phoneError is not null)
            Log.Note("Phone alert: recovered.");
        if (error != _phoneError)
        {
            _phoneError = error;
            PushState();
        }
    }

    object PhoneDto() => _vault?.Data.Phone is { } p
        ? new { enabled = p.Enabled, topic = p.Topic, error = _phoneError }
        : new { enabled = false, topic = "", error = (string?)null };

    /// <summary>enabled and/or topic ("" makes a new one). Switching on without a topic makes one.</summary>
    object? SetPhone(JsonElement a)
    {
        var p = RequireVault().Data.Phone;
        if (OptStr(a, "topic") is { } topic)
        {
            if (topic.Trim().Length == 0)
                p.Topic = PhoneAlert.NewTopic();
            else if (PhoneAlert.TryNormalize(topic, out string normalized))
                p.Topic = normalized;
            else
                throw new UserError(L.T("A topic is 6 to 64 letters, digits, - or _ (or https://your-server/topic)."));
            _phoneError = null;
        }
        if (Has(a, "enabled", JsonValueKind.True) || Has(a, "enabled", JsonValueKind.False))
        {
            p.Enabled = Bool(a, "enabled");
            if (p.Enabled && p.Topic.Length == 0)
                p.Topic = PhoneAlert.NewTopic();
        }
        PersistSoon();
        PushState();
        return null;
    }

    async Task<object?> SendPhoneTestAsync()
    {
        var p = RequireVault().Data.Phone;
        if (p.Topic.Length == 0)
            p.Topic = PhoneAlert.NewTopic();
        string? error = await PhoneAlert.SendAsync(p.Topic, L.T("Bloxkeep can reach this phone. Alerts will come here."), urgent: false);
        _phoneError = error;
        PushState();
        if (error is not null)
            throw new UserError(error);
        return null;
    }
}
