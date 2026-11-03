using System.Collections.Concurrent;
using System.Text;

namespace RobloxManager;

/// <summary>
/// Small files written so that a crash or a power cut in the middle leaves the old file or the new one, never an
/// empty one: the settings (prefs.json), backups, the person's own Roblox settings kept while Bloxkeep's are in place.
/// (The vault has its own, which also keeps the previous save: <see cref="Vault"/>.)
/// </summary>
static class DurableFile
{
    /// <summary>One writer at a time per file: two saves at once (the tick's and a page's) shared the one temp file, and
    /// the second threw "being used by another process".</summary>
    static readonly ConcurrentDictionary<string, object> Locks = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>Writes a temp file beside it, has it flushed to the disk, then puts it in place of the old one.</summary>
    public static void Write(string path, byte[] bytes)
    {
        lock (Locks.GetOrAdd(Path.GetFullPath(path), _ => new object()))
        {
            string tmp = path + ".tmp";
            using (var fs = new FileStream(tmp, FileMode.Create, FileAccess.Write, FileShare.None))
            {
                fs.Write(bytes);
                fs.Flush(flushToDisk: true);
            }
            File.Move(tmp, path, overwrite: true);
        }
    }

    /// <summary>The same for text, as UTF-8 without a byte order mark (as File.WriteAllText writes it).</summary>
    public static void WriteText(string path, string text) => Write(path, new UTF8Encoding(false).GetBytes(text));
}
