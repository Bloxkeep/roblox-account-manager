namespace RobloxManager;

/// <summary>
/// Unexpected errors go to error.log in the data folder instead of crashing the app.
/// Only exception details are written — never account data.
/// </summary>
static class Log
{
    const long MaxSize = 256 * 1024;
    static readonly object Sync = new();

    public static void Write(Exception ex) => Append(ex.ToString());

    /// <summary>A short diagnostic line (e.g. which kind of check Roblox asked for). Callers pass no account data.</summary>
    public static void Note(string text) => Append(text);

    static void Append(string text)
    {
        try
        {
            lock (Sync)
            {
                Directory.CreateDirectory(Vault.DataDir);
                string path = Path.Combine(Vault.DataDir, "error.log");
                if (File.Exists(path) && new FileInfo(path).Length > MaxSize)
                    File.Delete(path);
                File.AppendAllText(path, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {text}{Environment.NewLine}{Environment.NewLine}");
            }
        }
        catch (Exception logEx) when (logEx is IOException or UnauthorizedAccessException)
        {
            // Nowhere left to report it.
        }
    }
}
