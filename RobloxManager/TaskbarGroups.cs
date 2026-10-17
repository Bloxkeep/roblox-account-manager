using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Which taskbar button a Roblox window sits under. Windows groups windows by their AppUserModelID, so every Roblox
/// window shares one button; giving a window an id of its own (one per account, or one per preset) gives it its own
/// button or group, which also shows the window's icon (<see cref="WindowIcons"/>). This is a property Windows keeps
/// on the window: nothing in Roblox is touched, and each window's own id is put back when it is turned off or
/// Bloxkeep closes. UI thread only.
/// </summary>
static class TaskbarGroups
{
    [DllImport("shell32.dll")]
    static extern int SHGetPropertyStoreForWindow(IntPtr hwnd, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IPropertyStore store);
    [DllImport("ole32.dll")]
    static extern int PropVariantClear(ref PropVariant pvar);

    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPropertyStore
    {
        [PreserveSig] int GetCount(out uint count);
        [PreserveSig] int GetAt(uint index, out PropertyKey key);
        [PreserveSig] int GetValue(ref PropertyKey key, out PropVariant value);
        [PreserveSig] int SetValue(ref PropertyKey key, ref PropVariant value);
        [PreserveSig] int Commit();
    }

    [StructLayout(LayoutKind.Sequential)]
    struct PropertyKey(Guid format, uint id)
    {
        public Guid Format = format;
        public uint Id = id;
    }

    [StructLayout(LayoutKind.Explicit, Size = 24)]
    struct PropVariant
    {
        [FieldOffset(0)] public ushort Type;
        [FieldOffset(8)] public IntPtr Pointer;
    }

    const ushort VT_EMPTY = 0, VT_LPWSTR = 31;
    static PropertyKey AppUserModelId = new(new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"), 5);
    static Guid PropertyStoreIid = typeof(IPropertyStore).GUID;

    /// <summary>A client's group, and each of its windows' own id from before (put back when it goes).</summary>
    sealed record Placed(string Group, Dictionary<IntPtr, string?> Own);
    static readonly Dictionary<int, Placed> ByPid = new();

    /// <summary>Puts the client's windows in this group (an AppUserModelID of Bloxkeep's), noting their own ids first.</summary>
    public static void Apply(int pid, IEnumerable<IntPtr> windows, string group)
    {
        var placed = ByPid.TryGetValue(pid, out var had) ? had with { Group = group } : new Placed(group, new());
        ByPid[pid] = placed;
        foreach (var hWnd in windows)
        {
            string? now = Read(hWnd);
            if (now == group)
                continue;
            if (!placed.Own.ContainsKey(hWnd))
                placed.Own[hWnd] = now;
            Write(hWnd, group);
        }
    }

    /// <summary>The windows' own ids back (turned off, the client's account no longer grouped, Bloxkeep closing).</summary>
    public static void Remove(int pid, IEnumerable<IntPtr> windows)
    {
        if (!ByPid.Remove(pid, out var placed))
            return;
        foreach (var hWnd in windows)
        {
            if (placed.Own.TryGetValue(hWnd, out var own))
                Write(hWnd, own);
        }
    }

    /// <summary>Clients that have closed: nothing of theirs to put back.</summary>
    public static void Forget(IReadOnlySet<int> alive)
    {
        foreach (int pid in ByPid.Keys.Where(pid => !alive.Contains(pid)).ToList())
            ByPid.Remove(pid);
    }

    public static IEnumerable<int> Pids => ByPid.Keys.ToList();

    /// <summary>The window's AppUserModelID, or null when it has none (it groups by its program).</summary>
    internal static string? Read(IntPtr hWnd)
    {
        if (SHGetPropertyStoreForWindow(hWnd, ref PropertyStoreIid, out var store) != 0)
            return null;
        try
        {
            if (store.GetValue(ref AppUserModelId, out var value) != 0)
                return null;
            try
            {
                return value.Type == VT_LPWSTR ? Marshal.PtrToStringUni(value.Pointer) : null;
            }
            finally
            {
                PropVariantClear(ref value);
            }
        }
        finally
        {
            Marshal.ReleaseComObject(store);
        }
    }

    /// <summary>Sets the window's AppUserModelID; null clears it (Windows then groups it by its program again).</summary>
    static void Write(IntPtr hWnd, string? id)
    {
        if (SHGetPropertyStoreForWindow(hWnd, ref PropertyStoreIid, out var store) != 0)
            return;
        var value = id is null ? new PropVariant { Type = VT_EMPTY } : new PropVariant { Type = VT_LPWSTR, Pointer = Marshal.StringToCoTaskMemUni(id) };
        try
        {
            // Clearing answers 0x8007007B though the id is gone (seen 2026-10-17): the answer isn't relied on.
            store.SetValue(ref AppUserModelId, ref value);
            store.Commit();
        }
        finally
        {
            PropVariantClear(ref value);
            Marshal.ReleaseComObject(store);
        }
    }
}
