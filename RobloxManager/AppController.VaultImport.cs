using System.Security.Cryptography;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// "Import accounts from a vault file…": accounts from another Bloxkeep vault file (another PC's vault.rmv, or a
/// backup) are added to this vault and saved under its master password. The other file is only read
/// (<see cref="Vault.ReadOther"/>), never changed. Its decrypted contents stay in memory only while the preview is
/// open, under a one-time token; the page gets names and states, never cookies or passwords. They are dropped on
/// cancel, lock, a new page, the import itself, or after ten minutes.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan VaultImportLifetime = TimeSpan.FromMinutes(10);
    /// <summary>Wrong passwords for one file before each further try waits <see cref="VaultImportWait"/>.</summary>
    const int VaultImportFreeTries = 3;
    static readonly TimeSpan VaultImportWait = TimeSpan.FromSeconds(10);

    /// <param name="Owner">The vault it was opened for: a lock (and unlock) in between voids it.</param>
    sealed record VaultImportSession(string Token, Vault Owner, VaultData Data, System.Windows.Forms.Timer Expiry);

    /// <summary>The file picked to import from; kept here, so the page never names a path to read.</summary>
    string? _vaultImportPath;
    VaultImportSession? _vaultImport;
    bool _vaultImportOpening;
    /// <summary>Wrong passwords per file (full path) and when the next try is allowed; in memory only.</summary>
    readonly Dictionary<string, (int Wrong, DateTime NextTry)> _vaultImportTries = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>
    /// "Choose file…": Windows' own file dialog. Returns the file's name, or null when cancelled (a file picked
    /// earlier, and its open preview, then stay as they were).
    /// </summary>
    object? VaultImportPick()
    {
        var vault = RequireVault();
        string path;
        using (var dialog = new OpenFileDialog
               {
                   Title = L.T("Choose another Bloxkeep data file"),
                   Filter = L.T("Bloxkeep data files") + " (*.rmv)|*.rmv",
                   CheckFileExists = true,
                   InitialDirectory = Prefs.BackupFolder is { } folder && Directory.Exists(folder) ? folder : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
               })
        {
            if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
                return null;
            path = Path.GetFullPath(dialog.FileName);
        }
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        CheckImportFile(path, vault);
        DropVaultImport();
        _vaultImportPath = path;
        return new { name = Path.GetFileName(path), wait = ImportWaitSeconds(path) };
    }

    /// <summary>Refuses what can't be imported from, by the header alone (before any password is typed).</summary>
    static void CheckImportFile(string path, Vault vault)
    {
        long size;
        try
        {
            size = new FileInfo(path).Length;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new UserError(L.F("Couldn't open the data file: {0}", ErrorText(ex)));
        }
        if (size > Vault.MaxOtherSize)
            throw new UserError(L.T("This file is too large to be a Bloxkeep data file."));
        // The id comes from the salt: vault.rmv itself, its .bak and its backups all carry this vault's.
        string? id = Vault.IdOf(path);
        if (SamePath(path, vault.FilePath) || id == vault.Id)
            throw new UserError(L.T("This is the vault you're using."));
        if (id is null)
            throw new UserError(L.T("This file is not a Bloxkeep data file (.rmv)."));
    }

    int ImportWaitSeconds(string path) =>
        _vaultImportTries.TryGetValue(path, out var tries) && tries.NextTry > DateTime.UtcNow
            ? (int)Math.Ceiling((tries.NextTry - DateTime.UtcNow).TotalSeconds) : 0;

    /// <summary>
    /// Opens the picked file with its own master password and lists the accounts in it, each marked new or already
    /// here. A wrong password is an answer, not an error, so the page can count down the wait after three.
    /// </summary>
    async Task<object?> VaultImportOpenAsync(string password)
    {
        var vault = RequireVault();
        if (_vaultImportPath is not { } path)
            throw new UserError(L.T("Choose the file first."));
        if (password.Length == 0)
            throw new UserError(L.T("Enter the master password of that file."));
        if (_vaultImportOpening)
            throw new UserError(L.T("Still opening the file — please wait."));
        if (ImportWaitSeconds(path) is int wait and > 0)
            return new { status = "wrong", wait, error = L.F("Too many wrong tries. Wait {0} seconds, then try again.", wait) };
        CheckImportFile(path, vault); // it may have been replaced since it was picked
        DropVaultImport(keepPath: true);
        _vaultImportOpening = true;
        (Vault.OtherStatus Status, VaultData? Data) read;
        try
        {
            read = await Task.Run(() => Vault.ReadOther(path, password));
        }
        finally
        {
            _vaultImportOpening = false;
        }
        // Locked meanwhile: what was decrypted isn't kept.
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        switch (read.Status)
        {
            case Vault.OtherStatus.WrongPassword:
                int wrong = _vaultImportTries.GetValueOrDefault(path).Wrong + 1;
                _vaultImportTries[path] = (wrong, wrong >= VaultImportFreeTries ? DateTime.UtcNow + VaultImportWait : DateTime.MinValue);
                return new { status = "wrong", wait = ImportWaitSeconds(path), error = L.T("Wrong master password for this file.") };
            case Vault.OtherStatus.TooLarge:
                throw new UserError(L.T("This file is too large to be a Bloxkeep data file."));
            case Vault.OtherStatus.NotAVault:
                throw new UserError(L.T("This file is not a Bloxkeep data file (.rmv)."));
            case Vault.OtherStatus.Newer:
                throw new UserError(L.T("This data file was made by a newer Bloxkeep. Update Bloxkeep to open it."));
            case Vault.OtherStatus.Unreadable:
                throw new UserError(L.T("Windows couldn't read or write the file (another program may be using it)."));
        }
        _vaultImportTries.Remove(path);
        var data = read.Data!;
        // Pictures for the list: a short wait at most (offline, initials show instead).
        var avatars = LoadAvatarsAsync(data.Accounts.Select(a => a.UserId));
        await Task.WhenAny(avatars, Task.Delay(TimeSpan.FromSeconds(3)));
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));

        var expiry = new System.Windows.Forms.Timer { Interval = (int)VaultImportLifetime.TotalMilliseconds };
        expiry.Tick += (_, _) => DropVaultImport();
        var session = new VaultImportSession(Convert.ToHexString(RandomNumberGenerator.GetBytes(16)), vault, data, expiry);
        _vaultImport = session;
        expiry.Start();
        var here = vault.Data.Accounts;
        return new
        {
            status = "ok",
            token = session.Token,
            name = Path.GetFileName(path),
            presets = data.Settings.Presets.Count,
            // Only what the list shows; "key" is the account's place in the file, which the import sends back.
            accounts = data.Accounts.Select((a, i) => new
            {
                key = i,
                label = a.Label,
                username = a.Username,
                realm = Realm.Normalize(a.Realm),
                group = a.Group,
                hasPassword = a.Password.Length > 0,
                cookieValid = a.CookieValid,
                exists = ImportMatch(here, a) is not null,
                avatarUrl = _avatarUrls.GetValueOrDefault(a.UserId),
            }).ToArray(),
        };
    }

    /// <summary>
    /// The account here that is the same Roblox account: by user id within its realm (Roblox and Roblox VNG count
    /// ids apart, as in <see cref="AddOrMerge"/>), or by username (any case) when either side doesn't know the id.
    /// </summary>
    static Account? ImportMatch(List<Account> here, Account other)
    {
        string realm = Realm.Normalize(other.Realm);
        bool SameName(Account a) => other.Username.Length > 0 && string.Equals(a.Username, other.Username, StringComparison.OrdinalIgnoreCase);
        return (other.UserId != 0 ? here.FirstOrDefault(a => a.UserId == other.UserId && Realm.Normalize(a.Realm) == realm) : null)
            ?? here.FirstOrDefault(a => (other.UserId == 0 || a.UserId == 0) && Realm.Normalize(a.Realm) == realm && SameName(a));
    }

    /// <summary>The import command: the merge itself is <see cref="MergeImport"/>; then saved, shown and checked.</summary>
    object? VaultImportApply(JsonElement a)
    {
        var vault = RequireVault();
        if (_vaultImport is not { } session || session.Owner != vault || Str(a, "token") != session.Token)
            throw new UserError(L.T("The file was closed because it was open too long. Open it again."));
        var keys = Has(a, "keys", JsonValueKind.Array)
            ? a.GetProperty("keys").EnumerateArray().Where(e => e.ValueKind == JsonValueKind.Number && e.TryGetInt32(out _)).Select(e => e.GetInt32()).ToHashSet()
            : [];
        ImportOutcome result;
        try
        {
            result = MergeImport(vault.Data, session.Data, keys, Bool(a, "presets"));
        }
        finally
        {
            DropVaultImport(); // the token is good once
        }
        Persist();
        PushState();
        var touched = result.Added.Concat(result.Updated).ToList();
        if (touched.Count > 0)
            _ = LoadAvatarsAsync(touched.Select(x => x.UserId));
        if (touched.Where(x => x.CookieValid is null).Select(x => x.Id).ToArray() is { Length: > 0 } toCheck)
            _ = CheckImportedAsync(toCheck);
        return new
        {
            added = result.Added.Count,
            updated = result.Updated.Count,
            presets = result.PresetsAdded,
            result.PresetsSkipped,
            ids = result.Added.Select(x => x.Id).ToArray(),
        };
    }

    /// <param name="Updated">Accounts already here that took the file's sign-in or password.</param>
    /// <param name="PresetsSkipped">Presets left out: none of their accounts is here, or the preset limit was reached.</param>
    internal sealed record ImportOutcome(List<Account> Added, List<Account> Updated, int PresetsAdded, int PresetsSkipped);

    /// <summary>
    /// Brings the ticked accounts (<paramref name="keys"/>: their places in the file) into <paramref name="target"/>.
    /// New ones get a new id here: the file's could already name something in this vault (a deleted account's play
    /// history, the launch order). One already here only takes the file's sign-in, and its password when it has
    /// none, if its own sign-in isn't known to work; its name, group and note as set here are never changed. Presets
    /// come only when asked, schedules switched off and without their own Discord channel. Settings, Discord, the
    /// launch order and play history stay this vault's own.
    /// </summary>
    internal static ImportOutcome MergeImport(VaultData target, VaultData source, IReadOnlySet<int> keys, bool withPresets)
    {
        var here = target.Accounts;
        var added = new List<Account>();
        var updated = new List<Account>();
        // The file's account id → the same account's id here, for its presets.
        var localIds = new Dictionary<Guid, Guid>();
        for (int i = 0; i < source.Accounts.Count; i++)
        {
            var other = source.Accounts[i];
            // Matched against the list as it grows, so an account listed twice in the file is added once.
            var existing = ImportMatch(here, other);
            if (existing is not null)
                localIds.TryAdd(other.Id, existing.Id); // its presets can use it, ticked or not
            if (!keys.Contains(i))
                continue;
            if (existing is null)
            {
                var account = new Account
                {
                    UserId = other.UserId,
                    Username = other.Username,
                    DisplayName = other.DisplayName,
                    Alias = other.Alias,
                    Group = other.Group,
                    Note = other.Note,
                    Password = other.Password,
                    Authenticator = other.Authenticator,
                    Cookie = other.Cookie,
                    Realm = Realm.Normalize(other.Realm),
                    AutoRejoin = other.AutoRejoin,
                    UseRobloxConfig = other.UseRobloxConfig,
                    // When it was last played (on the other PC) is still true, and keeps "Recently used" meaningful.
                    LastUsed = other.LastUsed,
                    // What the other PC last knew may be out of date: checked right after the import.
                    CookieValid = null,
                    // Fields from a newer Bloxkeep go along, as a save keeps them.
                    Extra = other.Extra?.ToDictionary(p => p.Key, p => p.Value.Clone()),
                };
                here.Add(account);
                added.Add(account);
                localIds.TryAdd(other.Id, account.Id);
                continue;
            }
            // An authenticator key the file has and the account here lacks comes along: it doesn't depend on the sign-in.
            bool changed = false;
            if (existing.Authenticator is null && other.Authenticator is not null)
            {
                existing.Authenticator = other.Authenticator;
                changed = true;
            }
            // A sign-in that works here is newer than the file's; only a dead or unchecked one is replaced.
            if (existing.CookieValid == true)
            {
                if (changed && !added.Contains(existing) && !updated.Contains(existing))
                    updated.Add(existing);
                continue;
            }
            if (other.Cookie.Length > 0 && other.Cookie != existing.Cookie)
            {
                existing.Cookie = other.Cookie;
                existing.CookieValid = null;
                changed = true;
            }
            if (existing.Password.Length == 0 && other.Password.Length > 0)
            {
                existing.Password = other.Password;
                changed = true;
            }
            if (changed && !added.Contains(existing) && !updated.Contains(existing))
                updated.Add(existing);
        }

        int presetsAdded = 0, presetsSkipped = 0;
        if (withPresets)
        {
            var presets = target.Settings.Presets;
            foreach (var p in source.Settings.Presets)
            {
                // Accounts that aren't here (not ticked) are left out; a preset left with none isn't added.
                var ids = p.AccountIds.Where(localIds.ContainsKey).Select(id => localIds[id]).Distinct().ToList();
                if (ids.Count == 0 || presets.Count >= MaxPresets)
                {
                    presetsSkipped++;
                    continue;
                }
                var s = p.Schedule;
                presets.Add(new LaunchPreset
                {
                    Name = ImportedPresetName(presets, p.Name),
                    AccountIds = ids,
                    PlaceInput = p.PlaceInput,
                    GameName = p.GameName,
                    Server = PresetServers.Contains(p.Server) ? p.Server : "any",
                    JobId = p.JobId,
                    DelaySeconds = p.DelaySeconds is int delay ? Math.Clamp(delay, 0, 120) : null,
                    AntiAfk = p.AntiAfk,
                    AutoRejoin = p.AutoRejoin,
                    Profile = p.Profile == AfkProfile ? AfkProfile : "",
                    RestartHours = p.PlaceInput.Length > 0 && RestartChoices.Contains(p.RestartHours) ? p.RestartHours : 0,
                    Arrange = p.Arrange,
                    // Not its own Discord channel: a file from someone else would send this PC's alerts about these
                    // accounts (names, games, status) to that person's channel. Set it again here if it's yours.
                    DiscordWebhook = "",
                    // Off, so the two PCs don't start the same accounts at the same minute.
                    Schedule = s is null ? null : new PresetSchedule
                    {
                        Enabled = false,
                        Time = s.Time,
                        Days = s.Days.ToList(),
                        Date = s.Date,
                        Stop = s.Stop,
                        StopHours = s.StopHours,
                        StopTime = s.StopTime,
                    },
                    Extra = p.Extra?.ToDictionary(x => x.Key, x => x.Value.Clone()),
                });
                presetsAdded++;
            }
        }
        return new ImportOutcome(added, updated, presetsAdded, presetsSkipped);
    }

    /// <summary>The preset's own name, or "… (imported)" (numbered when taken too) when a preset here already has it.</summary>
    static string ImportedPresetName(List<LaunchPreset> presets, string name)
    {
        name = CutText(name.Trim(), MaxPresetName).TrimEnd();
        if (name.Length == 0)
            name = L.T("Imported preset");
        var names = presets.Select(p => p.Name).ToHashSet(StringComparer.OrdinalIgnoreCase);
        if (!names.Contains(name))
            return name;
        for (int n = 1; ; n++)
        {
            string suffix = n == 1 ? L.T(" (imported)") : L.F(" (imported {0})", n);
            // The name is shortened, not the suffix, so a 60-character name still reads as imported.
            string candidate = CutText(name, MaxPresetName - suffix.Length).TrimEnd() + suffix;
            if (!names.Contains(candidate))
                return candidate;
        }
    }

    /// <summary>
    /// Forgets the opened file's contents and, unless <paramref name="keepPath"/>, which file was picked. Cookies and
    /// passwords are .NET strings, which can't be wiped; with every reference gone they go at the next collection.
    /// </summary>
    object? DropVaultImport(bool keepPath = false)
    {
        if (_vaultImport is { } session)
        {
            _vaultImport = null;
            session.Expiry.Dispose();
            session.Data.Accounts.Clear();
            session.Data.Settings.Presets.Clear();
        }
        if (!keepPath)
            _vaultImportPath = null;
        return null;
    }
}
