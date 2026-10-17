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
        if (_vault is not { Data.Settings.RenameWindows: true })
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
    /// Bloxkeep closing: the windows get Roblox's own icon back first. The icons are Bloxkeep's, and they go with it: a
    /// window still pointing at one would show none.
    /// </summary>
    void GiveIconsBack()
    {
        foreach (int pid in WindowIcons.Pids)
            WindowIcons.Remove(pid, RobloxProcesses.VisibleWindows([pid]).GetValueOrDefault(pid) ?? []);
    }
}
