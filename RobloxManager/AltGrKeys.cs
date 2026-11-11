using System.Runtime.InteropServices;
using System.Text;

namespace RobloxManager;

/// <summary>
/// The keys an installed keyboard types a character with under AltGr. Windows sends AltGr as Ctrl+Alt, so a Ctrl+Alt hotkey
/// on one of them takes that character from every app: @ on a French keyboard (AltGr+0), { [ ] } on a German one (AltGr+7…0),
/// ą ń on a Polish one (AltGr+A, AltGr+N).
/// </summary>
static class AltGrKeys
{
    [DllImport("user32.dll")] static extern int GetKeyboardLayoutList(int count, IntPtr[]? list);
    [DllImport("user32.dll")] static extern uint MapVirtualKeyEx(uint code, uint mapType, IntPtr layout);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int ToUnicodeEx(uint key, uint scan, byte[] state, StringBuilder text, int size, uint flags, IntPtr layout);

    const uint MapVkToScan = 0;
    /// <summary>Leaves the keyboard's state (a dead key waiting) as it was (Windows 10 1607 and later).</summary>
    const uint KeepState = 0x4;
    const int Down = 0x80, VkShift = 0x10, VkControl = 0x11, VkMenu = 0x12, VkLeftControl = 0xA2, VkRightMenu = 0xA5;

    /// <summary>The hotkeys' keys (letters and digits) that any installed keyboard types with under AltGr.</summary>
    public static HashSet<Keys> Typed()
    {
        var keys = new HashSet<Keys>();
        try
        {
            int count = GetKeyboardLayoutList(0, null);
            if (count <= 0)
                return keys;
            var layouts = new IntPtr[count];
            count = GetKeyboardLayoutList(count, layouts);
            for (int i = 0; i < count; i++)
                keys.UnionWith(TypedBy(layouts[i]));
        }
        catch (Exception ex)
        {
            Log.Write(ex); // all hotkeys registered, as before
        }
        return keys;
    }

    /// <summary>The letters and digits <paramref name="layout"/> types a character with under AltGr (a dead key counts) that
    /// the key doesn't type without it. The Vietnamese keyboard gives AltGr+H as h, the key's own letter: nobody types h
    /// that way, and counting it left that keyboard's owners with no hotkey at all (its AltGr+0…9, the digits, do count).</summary>
    internal static IEnumerable<Keys> TypedBy(IntPtr layout)
    {
        var altGr = new byte[256];
        altGr[VkControl] = altGr[VkMenu] = altGr[VkLeftControl] = altGr[VkRightMenu] = Down;
        var shift = new byte[256];
        shift[VkShift] = Down;
        var plain = new byte[256];
        for (var key = Keys.D0; key <= Keys.Z; key++)
        {
            if (key is > Keys.D9 and < Keys.A)
                continue;
            uint scan = MapVirtualKeyEx((uint)key, MapVkToScan, layout);
            var (n, typed) = Type(key, scan, altGr, layout);
            // Under 0 is a dead key (^ ` ~ waiting for the next letter): typed as well. A control character isn't one.
            if (n < 0 || (n > 0 && !char.IsControl(typed[0]) && typed != Type(key, scan, plain, layout).Text
                && typed != Type(key, scan, shift, layout).Text))
                yield return key;
        }
    }

    static (int N, string Text) Type(Keys key, uint scan, byte[] state, IntPtr layout)
    {
        var text = new StringBuilder(8);
        int n = ToUnicodeEx((uint)key, scan, state, text, text.Capacity, KeepState, layout);
        return (n, n > 0 ? text.ToString(0, Math.Min(n, text.Length)) : "");
    }
}
