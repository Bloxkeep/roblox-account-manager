using System.Reflection;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>PASS/FAIL lines and the count of failures.</summary>
public sealed class Checks
{
    public int Failures { get; private set; }

    public void Check(string what, bool ok, object? detail = null)
    {
        Console.WriteLine($"{(ok ? "PASS" : "FAIL")}  {what}{(ok || detail is null ? "" : $"  [{detail}]")}");
        if (!ok)
            Failures++;
    }
}

/// <summary>Bloxkeep's own code driven from outside: a throwaway data folder, an unlocked vault, its private parts by reflection.</summary>
static class Harness
{
    public const BindingFlags F = BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public;
    public const string Password = "harness-pass-1";

    /// <summary>A fresh data folder; must run before anything reads Vault.DataDir or Prefs.</summary>
    public static void UseDataFolder(string dir)
    {
        if (Directory.Exists(dir))
            Directory.Delete(dir, true);
        Directory.CreateDirectory(dir);
        Environment.SetEnvironmentVariable("BLOXKEEP_DATA", dir);
        WindowsFormsSynchronizationContext.AutoInstall = false; // no message loop here: awaits carry on on the pool
        Prefs.Load();
    }

    public static async Task<object?> Call(AppController app, string command, object args)
    {
        var dispatch = typeof(AppController).GetMethod("DispatchAsync", F)!;
        try
        {
            return await (Task<object?>)dispatch.Invoke(app, [command, JsonSerializer.SerializeToElement(args)])!;
        }
        catch (TargetInvocationException e) when (e.InnerException is not null)
        {
            // A command refused before its first await is thrown through reflection: the app's own error, as the page gets it.
            System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(e.InnerException).Throw();
            throw;
        }
    }

    /// <summary>A command's answer, or the UserError it was refused with.</summary>
    public static async Task<object?> Try(AppController app, string command, object args)
    {
        try { return await Call(app, command, args); }
        catch (UserError e) { return e; }
    }

    public static Task Unlock(AppController app) => Call(app, "unlock", new { password = Password });

    public static T Get<T>(object owner, string field) => (T)owner.GetType().GetField(field, F)!.GetValue(owner)!;

    public static void Set(object owner, string field, object? value) => owner.GetType().GetField(field, F)!.SetValue(owner, value);

    public static object? Invoke(object owner, string method, params object?[] args) => owner.GetType().GetMethod(method, F)!.Invoke(owner, args);
}
