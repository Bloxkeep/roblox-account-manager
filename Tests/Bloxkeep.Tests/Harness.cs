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
        Vaults.Load();
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

    // Fields and methods by name, private ones too. A name is looked up on the owner and its base types; "a.b" goes through
    // the field a first; a name the owner doesn't have is looked for one level down, in the fields that hold Bloxkeep's own
    // objects (the owner keeps its parts in fields of their own types). Two parts that both have it is an error that names
    // them: pass "part.name" then.

    public static T Get<T>(object owner, string path)
    {
        var (holder, field) = FindField(owner, path);
        return (T)field.GetValue(holder)!;
    }

    public static void Set(object owner, string path, object? value)
    {
        var (holder, field) = FindField(owner, path);
        field.SetValue(holder, value);
    }

    public static object? Invoke(object owner, string path, params object?[] args)
    {
        int dot = path.LastIndexOf('.');
        object holder = owner;
        if (dot >= 0)
            holder = Get<object>(owner, path[..dot]) ?? throw new InvalidOperationException($"{path[..dot]} is null on {owner.GetType().Name}");
        string name = path[(dot + 1)..];
        var (target, method) = FindMember(holder, name, t => OnType(t, x => x.GetMethod(name, F | BindingFlags.DeclaredOnly)), $"method {name}");
        return method.Invoke(target, args);
    }

    static (object Holder, FieldInfo Field) FindField(object owner, string path)
    {
        object holder = owner;
        string[] parts = path.Split('.');
        for (int i = 0; ; i++)
        {
            string name = parts[i];
            var found = FindMember(holder, name, t => OnType(t, x => x.GetField(name, F | BindingFlags.DeclaredOnly)), $"field {name}");
            if (i == parts.Length - 1)
                return found;
            holder = found.Member.GetValue(found.Holder) ?? throw new InvalidOperationException($"{string.Join('.', parts[..(i + 1)])} is null on {owner.GetType().Name}");
        }
    }

    static T? OnType<T>(Type type, Func<Type, T?> find) where T : MemberInfo
    {
        for (Type? t = type; t is not null; t = t.BaseType)
        {
            if (find(t) is { } member)
                return member;
        }
        return null;
    }

    static (object Holder, T Member) FindMember<T>(object owner, string name, Func<Type, T?> find, string what) where T : MemberInfo
    {
        if (find(owner.GetType()) is { } direct)
            return (owner, direct);
        var hits = new List<(string Part, object Holder, T Member)>();
        for (Type? t = owner.GetType(); t is not null; t = t.BaseType)
        {
            foreach (var part in t.GetFields(F | BindingFlags.DeclaredOnly))
            {
                // Bloxkeep's own classes only (not a List or a Timer), and not a backing field of a property.
                if (part.Name.Contains('<') || part.FieldType.IsValueType || part.FieldType.Assembly != typeof(AppController).Assembly)
                    continue;
                if (part.GetValue(part.IsStatic ? null : owner) is { } value && find(value.GetType()) is { } member)
                    hits.Add((part.Name, value, member));
            }
        }
        if (hits.Count == 1)
            return (hits[0].Holder, hits[0].Member);
        throw new InvalidOperationException(hits.Count == 0
            ? $"No {what} on {owner.GetType().Name}, its base types or its fields that hold Bloxkeep's objects."
            : $"{what} is on more than one part of {owner.GetType().Name}: {string.Join(", ", hits.Select(h => h.Part))}. Say which: \"part.{name}\".");
    }
}
