using System.Drawing;

namespace RobloxManager;

/// <summary>
/// The account's avatar as its Roblox window's icon (<see cref="WindowIcons"/>), fetched once, and its name in the title.
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
        var shown = new List<(int Pid, List<IntPtr> Windows, Account Account)>();
        foreach (var (id, info) in _running)
        {
            if (FindAccount(id) is { } account && windows.TryGetValue(info.Pid, out var list) && list.Count > 0)
                shown.Add((info.Pid, list, account));
        }
        // One client's windows are asked whether they still have their icon at a time, each every half a minute.
        int? check = WindowIcons.NextToCheck(shown.Select(s => s.Pid));
        foreach (var (pid, list, account) in shown)
        {
            string? url = _avatarUrls.GetValueOrDefault(account.UserId);
            Bitmap? picture = null;
            if (url is not null && !_iconPictures.TryGetValue(url, out picture))
                _ = LoadIconPictureAsync(url);
            if (picture is not null)
            {
                WindowIcons.Apply(pid, list, url!, () => new Bitmap(picture), check: pid == check);
            }
            else
            {
                string name = NameText(account); // "Account 3" while names are hidden
                WindowIcons.Apply(pid, list, "initials:" + name, () => WindowIcons.Initials(name), check: pid == check);
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
    /// Settings › Taskbar buttons (<see cref="TaskbarGroups"/>): a button per account, or a group per preset (windows
    /// without one stay in Roblox's); "roblox" puts every window's own id back.
    /// </summary>
    bool _taskbarLeftoversCleared;

    void ApplyTaskbarGroups(IReadOnlySet<int> alive, Dictionary<int, List<IntPtr>> windows)
    {
        TaskbarGroups.Forget(alive);
        // The first look at the windows: the groups a run that ended suddenly left on them go, whatever the setting.
        if (!_taskbarLeftoversCleared && windows.Count > 0)
        {
            _taskbarLeftoversCleared = true;
            TaskbarGroups.ClearLeftovers(windows.Values.SelectMany(w => w));
        }
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

    /// <summary>Bloxkeep closing: the icons are Bloxkeep's and go with it, so a window left pointing at one would show none.</summary>
    void GiveIconsBack()
    {
        foreach (int pid in WindowIcons.Pids)
            WindowIcons.Remove(pid, RobloxProcesses.VisibleWindows([pid]).GetValueOrDefault(pid) ?? []);
        // And their own taskbar button: a group named after Bloxkeep's accounts means nothing once it has closed.
        foreach (int pid in TaskbarGroups.Pids)
            TaskbarGroups.Remove(pid, RobloxProcesses.VisibleWindows([pid]).GetValueOrDefault(pid) ?? []);
    }
}
