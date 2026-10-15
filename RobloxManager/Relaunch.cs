using System.Security.Cryptography;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Updating without a "Restart to update" click, as the Claude app does: once an update is downloaded and nobody is
/// using Bloxkeep, it writes down what the next run needs to carry on unnoticed, lets Velopack install the update and
/// start it again, and the new run picks that up — unlocked, with the window as it was (hidden in the tray, minimised,
/// or open on the same view, without taking the keyboard), and the Roblox windows followed again (AdoptOpenSessions).
/// The vault key travels in a file only this Windows user can read (DPAPI), mixed with a random value that only the
/// new process's command line carries; the file is deleted as it is read, and refused once a couple of minutes old.
/// </summary>
public static class Relaunch
{
    const string Arg = "--resume";
    /// <summary>Started again after an update to this version (with or without a hand-over): see <see cref="AfterUpdate"/>.</summary>
    public const string AfterUpdateArg = "--after-update";
    static readonly TimeSpan Life = TimeSpan.FromMinutes(3);
    static string FilePath => Path.Combine(Vault.DataDir, "resume.bin");

    public sealed class State
    {
        public DateTime WrittenAt { get; set; }
        public string? From { get; set; }
        /// <summary>The version being installed: a run that comes back as anything else knows the install didn't take.</summary>
        public string? To { get; set; }
        /// <summary>The vault's key, salt and iterations (base64), or null when it was locked.</summary>
        public string? Key { get; set; }
        public string? Salt { get; set; }
        public int Iterations { get; set; }
        public bool WeakPassword { get; set; }
        /// <summary>hidden (in the tray), minimized, or shown.</summary>
        public string Window { get; set; } = "shown";
        /// <summary>The window was the one in front: it comes back in front, else behind what is.</summary>
        public bool Foreground { get; set; }
        /// <summary>What the page had on screen (see AppController.KeepView).</summary>
        public string? View { get; set; }
    }

    /// <summary>Writes <paramref name="state"/> for the next run and returns the arguments it must be started with.</summary>
    public static string[] Write(State state)
    {
        byte[] entropy = RandomNumberGenerator.GetBytes(32);
        byte[] json = JsonSerializer.SerializeToUtf8Bytes(state);
        try
        {
            Directory.CreateDirectory(Vault.DataDir);
            File.WriteAllBytes(FilePath, ProtectedData.Protect(json, entropy, DataProtectionScope.CurrentUser));
        }
        finally
        {
            CryptographicOperations.ZeroMemory(json);
        }
        return [Arg, Convert.ToHexString(entropy)];
    }

    /// <summary>Removes what <see cref="Write"/> wrote when the quit it was for didn't happen after all.</summary>
    public static void Discard()
    {
        try
        {
            File.Delete(FilePath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex);
        }
    }

    /// <summary>
    /// What the previous run left, when this run was started by it (and in time); null otherwise. The file goes
    /// either way, so a key never stays on disk.
    /// </summary>
    public static State? Take(string[] args) => Read(args) ?? AfterUpdate(args);

    /// <summary>
    /// Started after an update without a usable hand-over (it came too late, or Bloxkeep was started by the watch over
    /// the update): back in the tray, locked, saying so, rather than a lock screen popping up over what the person is doing.
    /// </summary>
    static State? AfterUpdate(string[] args)
    {
        int at = Array.IndexOf(args, AfterUpdateArg);
        return at < 0 ? null : new State { Window = "hidden", To = at + 1 < args.Length ? args[at + 1] : null };
    }

    static State? Read(string[] args)
    {
        int at = Array.IndexOf(args, Arg);
        byte[]? entropy = null;
        try
        {
            entropy = at >= 0 && at + 1 < args.Length ? Convert.FromHexString(args[at + 1]) : null;
        }
        catch (FormatException)
        {
        }
        byte[]? sealedBytes = null;
        try
        {
            if (File.Exists(FilePath))
            {
                sealedBytes = File.ReadAllBytes(FilePath);
                File.Delete(FilePath);
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex);
        }
        if (entropy is null || sealedBytes is null)
            return null;
        byte[]? json = null;
        try
        {
            json = ProtectedData.Unprotect(sealedBytes, entropy, DataProtectionScope.CurrentUser);
            var state = JsonSerializer.Deserialize<State>(json);
            if (state is null || DateTime.UtcNow - state.WrittenAt > Life || state.WrittenAt > DateTime.UtcNow + TimeSpan.FromMinutes(1))
                return null;
            return state;
        }
        catch (Exception ex) when (ex is CryptographicException or JsonException)
        {
            Log.Write(ex);
            return null;
        }
        finally
        {
            if (json is not null)
                CryptographicOperations.ZeroMemory(json);
        }
    }
}
