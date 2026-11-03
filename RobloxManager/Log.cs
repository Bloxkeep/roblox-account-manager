namespace RobloxManager;

/// <summary>
/// Unexpected errors go to error.log in the data folder instead of crashing the app.
/// Only exception details are written — never account data. At 256 KB the log moves to error.log.1 (the earlier one of
/// those is replaced), so a bug report still has the lines from before.
/// </summary>
static class Log
{
    const long MaxSize = 256 * 1024;
    static readonly object Sync = new();

    public static void Write(Exception ex) => Append(ex.ToString());

    /// <summary>A short diagnostic line (e.g. which kind of check Roblox asked for). Callers pass no account data.</summary>
    public static void Note(string text) => Append(text);

    /// <summary>A last guard on what is written: a session cookie or a Discord webhook link that reached a message (an exception's
    /// text can hold a request's address or a header) never lands in a file that is sent with a bug report and shown on request.</summary>
    static readonly System.Text.RegularExpressions.Regex Secrets = new(
        @"_\|WARNING:-DO-NOT-SHARE-THIS[^\s""']*|https?://(?:[a-z]+\.)?discord(?:app)?\.com/api/webhooks/\S+",
        System.Text.RegularExpressions.RegexOptions.IgnoreCase | System.Text.RegularExpressions.RegexOptions.CultureInvariant);

    static void Append(string text)
    {
        text = Secrets.Replace(text, "[secret]");
        try
        {
            lock (Sync)
            {
                Directory.CreateDirectory(Vault.DataDir);
                string path = Path.Combine(Vault.DataDir, "error.log");
                // At the limit the log is kept as error.log.1 (the one before replaced): a report needs the lines from before it.
                if (File.Exists(path) && new FileInfo(path).Length > MaxSize)
                    File.Move(path, path + ".1", overwrite: true);
                File.AppendAllText(path, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {text}{Environment.NewLine}{Environment.NewLine}");
            }
        }
        catch (Exception logEx) when (logEx is IOException or UnauthorizedAccessException)
        {
            // Nowhere left to report it.
        }
    }
}
