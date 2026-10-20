namespace RobloxManager;

/// <summary>
/// Several vaults (<see cref="Vaults"/>): the lock screen's picker, Steam-like ("Who's using Bloxkeep?"), shown at
/// start when asked for (<see cref="Prefs.AskVault"/>) or from Switch vault; adding, naming, a picture, taking one off
/// the list. Opening the picker (or the form for a new vault) over an open vault leaves it open, so Back returns to it
/// without the password; only picking or setting up another vault locks it. Its Roblox windows keep running then, its
/// auto-rejoin, schedules and Discord wait for it like when it is locked.
/// </summary>
public sealed partial class AppController
{
    /// <summary>The picker is up instead of the password (start, or Switch vault).</summary>
    bool _vaultPick = Prefs.AskVault && Prefs.VaultList.Count > 1;

    /// <summary>
    /// A vault being added: the form sets it up (name and master password) before it joins the list. The vault the
    /// lock screen offers (or the one open) stays the active one until then.
    /// </summary>
    Prefs.VaultEntry? _newVault;

    /// <summary>Adding started from the picker: Cancel goes back to it, else to the screen it came from.</summary>
    bool _addFromPicker;

    object VaultsDto() => new
    {
        list = Prefs.VaultList.Select(v => new { id = v.Id, name = v.Name, picture = v.Picture is { } p ? "user/" + p : null }).ToArray(),
        // The vault the lock screen (or the form over the open vault) is about: the one being added, or the one offered.
        current = (_newVault ?? Vaults.Current)?.Id,
        name = (_newVault ?? Vaults.Current)?.Name,
        picture = _newVault is null && Vaults.Current?.Picture is { } own ? "user/" + own : null,
        // The vault open right now (none while locked): the sidebar's Switch vault, Settings › Security.
        open = _vault is null ? null : Vaults.Current?.Id,
        pick = _vaultPick,
        adding = _newVault is not null,
        ask = Prefs.AskVault,
    };

    /// <summary>"Switch vault" (in the app or on the lock screen): the picker shows; an open vault stays open behind it.</summary>
    object? ShowVaultPicker()
    {
        _newVault = null;
        _vaultPick = true;
        PushState();
        return null;
    }

    /// <summary>The picker's Back: to the open vault as it was, or to the password of the vault the lock screen offers.</summary>
    object? CloseVaultPicker()
    {
        _vaultPick = false;
        PushState();
        return null;
    }

    /// <summary>
    /// A vault chosen in the picker. The one open right now: back to it as it was. Another: the open one locks and
    /// the lock screen asks for the chosen one's master password.
    /// </summary>
    object? SelectVault(string id)
    {
        var entry = Prefs.VaultList.FirstOrDefault(v => v.Id == id) ?? throw new UserError(L.T("That vault is no longer on this PC."));
        _newVault = null;
        if (_vault is not null && entry != Vaults.Current)
            Lock();
        Vaults.Use(entry);
        if (Prefs.LastVault != entry.Id)
        {
            Prefs.LastVault = entry.Id;
            Prefs.Save();
        }
        _vaultPick = false;
        PushState();
        return null;
    }

    /// <summary>
    /// "Add a vault" (the picker's +, or the lock screen's Create a new vault): a form for its name and own master
    /// password. An open vault stays open until the new one is set up (Cancel goes back to it).
    /// </summary>
    object? AddVault()
    {
        _addFromPicker = _vaultPick;
        _newVault = Vaults.New();
        _vaultPick = false;
        PushState();
        return null;
    }

    /// <summary>Adding cancelled: back to the picker, or to the screen it came from (an open vault still open).</summary>
    object? CancelAddVault()
    {
        _newVault = null;
        _vaultPick = _addFromPicker && Prefs.VaultList.Count > 0;
        PushState();
        return null;
    }

    /// <summary>The vault being added is about to be set up: the open one locks (saved first) and the new one becomes the active vault.</summary>
    void SwitchToNewVault()
    {
        if (_newVault is null)
            return;
        if (_vault is not null)
            Lock();
        Vaults.Use(_newVault);
    }

    /// <summary>A vault just set up (the first one, or one being added) joins the list under the name typed.</summary>
    void JoinVaultList(string? name)
    {
        var entry = _newVault ?? Vaults.Current ?? Vaults.New(); // the first vault on this PC: vault.rmv, "Vault 1"
        if (Vaults.CleanName(name) is { } typed)
            entry.Name = typed;
        if (!Prefs.VaultList.Contains(entry))
            Prefs.VaultList.Add(entry);
        _newVault = null;
        Prefs.LastVault = entry.Id;
        Prefs.Save();
    }

    object? RenameVault(string id, string name)
    {
        var entry = Prefs.VaultList.FirstOrDefault(v => v.Id == id) ?? throw new UserError(L.T("That vault is no longer on this PC."));
        entry.Name = Vaults.CleanName(name) ?? throw new UserError(L.T("Give the vault a name."));
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>A picture from the PC for a vault's tile, copied into the data folder (shown before unlocking, so anyone at the PC sees it).</summary>
    object? PickVaultPicture(string id)
    {
        var entry = Prefs.VaultList.FirstOrDefault(v => v.Id == id) ?? throw new UserError(L.T("That vault is no longer on this PC."));
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Choose a picture for this vault"),
            Filter = L.T("Pictures") + " (*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp)|*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp",
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyPictures),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
            return null;
        string type = Path.GetExtension(dialog.FileName).ToLowerInvariant();
        if (!PictureTypes.Contains(type))
            throw new UserError(L.T("Pick a picture: a PNG, JPG, WebP, GIF or BMP file."));
        if (new FileInfo(dialog.FileName).Length > MaxThemePicture)
            throw new UserError(L.T("That picture is over 20 MB. Pick a smaller one."));
        Directory.CreateDirectory(Vault.DataDir);
        string name = $"vault-picture-{entry.Id}-{DateTime.UtcNow.Ticks}{type}";
        File.Copy(dialog.FileName, Path.Combine(Vault.DataDir, name));
        DeleteVaultPicture(entry);
        entry.Picture = name;
        Prefs.Save();
        PushState();
        return null;
    }

    object? ClearVaultPicture(string id)
    {
        var entry = Prefs.VaultList.FirstOrDefault(v => v.Id == id) ?? throw new UserError(L.T("That vault is no longer on this PC."));
        DeleteVaultPicture(entry);
        entry.Picture = null;
        Prefs.Save();
        PushState();
        return null;
    }

    static void DeleteVaultPicture(Prefs.VaultEntry entry)
    {
        if (entry.Picture is not { } picture || !Themes.IsPictureName(picture))
            return;
        try
        {
            File.Delete(Path.Combine(Vault.DataDir, picture));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex); // left behind: its name is no longer used
        }
    }

    /// <summary>
    /// A vault off this PC's list (one whose password is lost, say): its file is set aside, not deleted
    /// (<see cref="Vault.SetAside"/>), and can come back with Restore from a backup. Not the vault open right now.
    /// </summary>
    object? RemoveVault(string id)
    {
        var entry = Prefs.VaultList.FirstOrDefault(v => v.Id == id) ?? throw new UserError(L.T("That vault is no longer on this PC."));
        if (_vault is not null && entry == Vaults.Current)
            throw new UserError(L.T("Switch to another vault first."));
        try
        {
            string path = Path.Combine(Vault.DataDir, entry.File);
            if (File.Exists(path) || File.Exists(path + ".bak") || File.Exists(path + ".tmp"))
            {
                if (!File.Exists(path))
                    File.Move(File.Exists(path + ".tmp") ? path + ".tmp" : path + ".bak", path);
                Log.Note($"A vault was taken off the list; its file was set aside as {Path.GetFileName(Vault.SetAside(entry.File))}.");
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new UserError(L.F("Couldn't remove the vault: {0}", ErrorText(ex)));
        }
        DeleteVaultPicture(entry);
        bool offered = entry == Vaults.Current;
        Prefs.VaultList.Remove(entry);
        if (offered || Prefs.LastVault == entry.Id)
        {
            var next = Prefs.VaultList.FirstOrDefault();
            Vaults.Use(next);
            Prefs.LastVault = next?.Id;
        }
        _vaultPick = Prefs.VaultList.Count > 1 || (_vaultPick && Prefs.VaultList.Count > 0);
        Prefs.Save();
        PushState();
        return null;
    }

    object? SetAskVault(bool on)
    {
        Prefs.AskVault = on;
        Prefs.Save();
        PushState();
        return null;
    }
}
