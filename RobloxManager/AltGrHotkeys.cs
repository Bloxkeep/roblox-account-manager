using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The Ctrl+Alt hotkeys on keys the keyboard also types with AltGr (<see cref="AltGrKeys"/>). Windows sends AltGr as Ctrl+Alt,
/// and RegisterHotKey can't tell them apart: a French keyboard's AltGr+0 (@) was taken by Ctrl+Alt+0. A low-level keyboard
/// hook can, by which Alt is down: the left one (and a Ctrl) is the hotkey, kept from the app; the right one (AltGr) is
/// typing and goes through untouched. Only these keys use the hook, and only on a PC whose keyboard needs it: the hook sees
/// every key pressed, so the plain hotkeys stay on RegisterHotKey.
/// </summary>
sealed class AltGrHotkeys : IDisposable
{
    /// <summary>Virtual-key code to hotkey id.</summary>
    readonly Dictionary<int, int> _keys;
    readonly Action<int> _fire;
    /// <summary>Keys taken as a hotkey and still down: their repeats and their release are kept from the app too.</summary>
    readonly HashSet<int> _held = [];
    readonly Thread _thread;
    HookProc? _proc;
    IntPtr _hook;
    uint _threadId;

    AltGrHotkeys(IReadOnlyDictionary<Keys, int> keys, Action<int> fire)
    {
        _keys = keys.ToDictionary(k => (int)k.Key, k => k.Value);
        _fire = fire;
        _thread = new Thread(Run) { IsBackground = true, Name = "Bloxkeep AltGr hotkeys" };
    }

    /// <summary>The hook, set; null when Windows refused it (the keys are then left to the keyboard).</summary>
    /// <param name="fire">Called on the hook's thread with the hotkey's id: it must only post, not do the work.</param>
    public static AltGrHotkeys? Start(IReadOnlyDictionary<Keys, int> keys, Action<int> fire)
    {
        var hotkeys = new AltGrHotkeys(keys, fire);
        using var ready = new ManualResetEventSlim();
        hotkeys._ready = ready;
        hotkeys._thread.Start();
        bool inTime = ready.Wait(TimeSpan.FromSeconds(5));
        hotkeys._ready = null;
        if (hotkeys._hook != IntPtr.Zero)
            return hotkeys;
        Log.Note($"Hotkeys: the keyboard hook for AltGr keys was refused{(inTime ? $" (error {hotkeys._error})" : ", its thread took over 5 s")}.");
        hotkeys.Dispose();
        return null;
    }

    ManualResetEventSlim? _ready;
    int _error;

    void Run()
    {
        _threadId = GetCurrentThreadId();
        _proc = OnKeyboard;
        _hook = SetWindowsHookEx(WH_KEYBOARD_LL, _proc, GetModuleHandle(null), 0);
        _error = _hook == IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
        _ready?.Set();
        if (_hook == IntPtr.Zero)
            return;
        while (GetMessage(out var message, IntPtr.Zero, 0, 0) > 0)
            DispatchMessage(ref message);
        UnhookWindowsHookEx(_hook);
    }

    // Quick, on the hook's thread: Windows drops a hook that takes long (LowLevelHooksTimeout), and every key on the PC
    // waits for it.
    IntPtr OnKeyboard(int code, IntPtr wParam, IntPtr lParam)
    {
        if (code >= 0)
        {
            var info = Marshal.PtrToStructure<KBDLLHOOKSTRUCT>(lParam);
            int vk = (int)info.vkCode;
            if (_keys.TryGetValue(vk, out int id) && (info.flags & LLKHF_INJECTED) == 0)
            {
                bool up = (info.flags & LLKHF_UP) != 0;
                if (up)
                {
                    if (_held.Remove(vk))
                        return 1;
                }
                else if (_held.Contains(vk))
                    return 1; // a repeat of a hotkey held down (as MOD_NOREPEAT)
                else if (Hotkey())
                {
                    _held.Add(vk);
                    _fire(id);
                    return 1;
                }
            }
        }
        return CallNextHookEx(_hook, code, wParam, lParam);
    }

    /// <summary>Ctrl and the left Alt, nothing else. AltGr is the right Alt (with a Ctrl Windows adds itself): typing.
    /// Before this key's own event the keys' state is up to date.</summary>
    static bool Hotkey() =>
        (Down(VK_LCONTROL) || Down(VK_RCONTROL)) && Down(VK_LMENU) && !Down(VK_RMENU)
        && !Down(VK_SHIFT) && !Down(VK_LWIN) && !Down(VK_RWIN);

    static bool Down(int vk) => (GetAsyncKeyState(vk) & 0x8000) != 0;

    public void Dispose()
    {
        if (_threadId != 0)
            PostThreadMessage(_threadId, WM_QUIT, IntPtr.Zero, IntPtr.Zero);
    }

    delegate IntPtr HookProc(int code, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    struct KBDLLHOOKSTRUCT
    {
        public uint vkCode, scanCode, flags, time;
        public IntPtr dwExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct MSG
    {
        public IntPtr hwnd;
        public uint message;
        public IntPtr wParam, lParam;
        public uint time;
        public int x, y;
    }

    const int WH_KEYBOARD_LL = 13;
    const uint LLKHF_INJECTED = 0x10, LLKHF_UP = 0x80, WM_QUIT = 0x12;
    const int VK_SHIFT = 0x10, VK_LWIN = 0x5B, VK_RWIN = 0x5C, VK_LCONTROL = 0xA2, VK_RCONTROL = 0xA3, VK_LMENU = 0xA4, VK_RMENU = 0xA5;

    [DllImport("user32.dll", SetLastError = true)] static extern IntPtr SetWindowsHookEx(int id, HookProc proc, IntPtr module, uint thread);
    [DllImport("user32.dll")] static extern bool UnhookWindowsHookEx(IntPtr hook);
    [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] static extern short GetAsyncKeyState(int vk);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetModuleHandle(string? name);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern int GetMessage(out MSG message, IntPtr hWnd, uint min, uint max);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG message);
    [DllImport("user32.dll")] static extern bool PostThreadMessage(uint thread, uint message, IntPtr wParam, IntPtr lParam);
}
