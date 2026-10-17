using System.Drawing;

namespace RobloxManager;

/// <summary>
/// The account's picture as its Roblox window's icon (<see cref="WindowIcons"/>), with the account's name in the title
/// (Settings › Show account names on windows). The pictures are the avatars the page shows, fetched once.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Pictures by address; null when it couldn't be read (the initials then).</summary>
    readonly Dictionary<string, Bitmap?> _iconPictures = new();
    readonly HashSet<string> _iconLoading = new();

    void ApplyWindowIcons(IReadOnlySet<int> alive, Dictionary<int, List<IntPtr>> windows)
    {
        WindowIcons.Forget(alive);
        if (_vault?.Data.Settings is not { } s || !(s.WindowPictures ?? s.RenameWindows))
        {
            foreach (int pid in WindowIcons.Pids)
                WindowIcons.Remove(pid, windows.GetValueOrDefault(pid) ?? []);
            return;
        }
        foreach (var (id, info) in _running)
        {
            if (FindAccount(id) is not { } account || !windows.TryGetValue(info.Pid, out var list) || list.Count == 0)
                continue;
            string? url = _avatarUrls.GetValueOrDefault(account.UserId);
            Bitmap? picture = null;
            if (url is not null && !_iconPictures.TryGetValue(url, out picture))
                _ = LoadIconPictureAsync(url);
            if (picture is not null)
            {
                WindowIcons.Apply(info.Pid, list, url!, () => new Bitmap(picture));
            }
            else
            {
                string name = NameText(account); // "Account 3" while names are hidden
                WindowIcons.Apply(info.Pid, list, "initials:" + name, () => WindowIcons.Initials(name));
            }
        }
    }

    async Task LoadIconPictureAsync(string url)
    {
        if (!_iconLoading.Add(url))
            return;
        try
        {
            Bitmap? picture = null;
            if (await SummaryCard.LoadAvatarAsync(url, CancellationToken.None) is { } bytes)
            {
                try
                {
                    using var stream = new MemoryStream(bytes);
                    using var image = Image.FromStream(stream);
                    picture = new Bitmap(image);
                }
                catch (ArgumentException)
                {
                    // Not a picture System.Drawing reads: the initials.
                }
            }
            if (_iconPictures.Count >= 200)
            {
                foreach (var old in _iconPictures.Values)
                    old?.Dispose();
                _iconPictures.Clear();
            }
            _iconPictures[url] = picture;
        }
        finally
        {
            _iconLoading.Remove(url);
        }
    }

    /// <summary>
    /// Settings › Taskbar buttons (<see cref="TaskbarGroups"/>): each account's window under a button of its own, or
    /// each preset's windows in a group of their own (windows opened without a preset stay in Roblox's); Roblox's one
    /// button for all of them puts every window's own id back.
    /// </summary>
    void ApplyTaskbarGroups(IReadOnlySet<int> alive, Dictionary<int, List<IntPtr>> windows)
    {
        TaskbarGroups.Forget(alive);
        string mode = _vault?.Data.Settings.TaskbarGroups ?? "roblox";
        var grouped = new HashSet<int>();
        foreach (var (id, info) in _running)
        {
            string? group = mode switch
            {
                "account" => $"Bloxkeep.Roblox.{id:N}",
                "preset" when info.Launch.PresetId is { } preset => $"Bloxkeep.Preset.{preset:N}",
                _ => null,
            };
            if (group is null || !windows.TryGetValue(info.Pid, out var list) || list.Count == 0)
                continue;
            TaskbarGroups.Apply(info.Pid, list, group);
            grouped.Add(info.Pid);
        }
        foreach (int pid in TaskbarGroups.Pids.Where(pid => !grouped.Contains(pid)))
            TaskbarGroups.Remove(pid, windows.GetValueOrDefault(pid) ?? []);
    }

    /// <summary>
    /// Bloxkeep closing: the windows get Roblox's own icon back first. The icons are Bloxkeep's, and they go with it: a
    /// window still pointing at one would show none.
    /// </summary>
    void GiveIconsBack()
    {
        foreach (int pid in WindowIcons.Pids)
            WindowIcons.Remove(pid, RobloxProcesses.VisibleWindows([pid]).GetValueOrDefault(pid) ?? []);
        // And their own taskbar button: a group named after Bloxkeep's accounts means nothing once it has closed.
        foreach (int pid in TaskbarGroups.Pids)
            TaskbarGroups.Remove(pid, RobloxProcesses.VisibleWindows([pid]).GetValueOrDefault(pid) ?? []);
    }
}
