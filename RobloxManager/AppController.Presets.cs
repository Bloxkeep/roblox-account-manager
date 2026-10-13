using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Launch presets: saved accounts, game and kind of server, run from the dock or the tray icon's menu. Servers other
/// than "any" and a fixed Job ID are picked here when the preset runs, so the tray can run one while the page is closed.
/// </summary>
public sealed partial class AppController
{
    /// <param name="NeedsConfirm">Nothing was opened yet: <paramref name="Message"/> asks whether to open anyway.</param>
    /// <param name="Notice">Something the person should know that isn't an error (no server had room).</param>
    sealed record LaunchResult(bool NeedsConfirm = false, string? Message = null, int Launched = 0, List<string>? Errors = null,
        bool Cancelled = false, string? Notice = null);

    const int MaxPresets = 50;
    const int MaxPresetName = 60;
    static readonly string[] PresetServers = ["any", "most", "fewest", "ping", "random", "job"];

    object PresetDto(LaunchPreset p) => new
    {
        id = p.Id,
        p.Name,
        p.AccountIds,
        p.PlaceInput,
        p.GameName,
        p.Server,
        p.JobId,
        p.DelaySeconds,
    };

    /// <summary>Adds a preset, or replaces the one with the given id. Returns its id.</summary>
    async Task<object?> SavePresetAsync(JsonElement a)
    {
        var vault = RequireVault();
        string name = Str(a, "name").Trim();
        if (name.Length == 0)
            throw new UserError(L.T("Give the preset a name."));
        if (name.Length > MaxPresetName)
            name = name[..MaxPresetName];
        var ids = Ids(a).Where(id => vault.Data.Accounts.Any(acc => acc.Id == id)).ToList();
        if (ids.Count == 0)
            throw new UserError(L.T("Select at least one account."));
        string place = Str(a, "placeInput").Trim();
        // Without a game the preset just opens Roblox, so there is no server to choose.
        string server = place.Length == 0 ? "any" : PresetServers.Contains(Str(a, "server")) ? Str(a, "server") : "any";
        string job = server == "job" ? Str(a, "jobId").Trim() : "";
        if (server == "job" && job.Length == 0)
            throw new UserError(L.T("Enter the server's Job ID."));
        int? delay = Has(a, "delaySeconds", JsonValueKind.Number) ? (int)Math.Clamp(Long(a, "delaySeconds"), 0, 120) : null;

        // Checked now, so a typo shows while saving rather than when the preset runs.
        string gameName = "";
        if (place.Length > 0)
        {
            if (!GameTarget.TryParse(place, job, out var target, out string error))
                throw new UserError(error);
            if (target!.ShareCode is null && await GetGameInfoAsync(target.PlaceId) is { } info)
                gameName = info.Name;
        }
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));

        var presets = vault.Data.Settings.Presets;
        var preset = presets.FirstOrDefault(p => p.Id == Id(a));
        if (preset is null)
        {
            if (presets.Count >= MaxPresets)
                throw new UserError(L.F("You can save up to {0} presets.", MaxPresets));
            preset = new LaunchPreset();
            presets.Add(preset);
        }
        preset.Name = name;
        preset.AccountIds = ids;
        preset.PlaceInput = place;
        preset.GameName = gameName;
        preset.Server = server;
        preset.JobId = job;
        preset.DelaySeconds = delay;
        Persist();
        PushState();
        return preset.Id;
    }

    object? DeletePreset(Guid id)
    {
        RequireVault().Data.Settings.Presets.RemoveAll(p => p.Id == id);
        Persist();
        PushState();
        return null;
    }

    async Task<object?> RunPresetAsync(Guid id, bool force) => await StartPresetAsync(id, force);

    /// <summary>
    /// Opens the preset's accounts that still exist, in its order, through the same launch as the dock's Play button
    /// (same waits, warnings, Stop button and progress text).
    /// </summary>
    async Task<LaunchResult> StartPresetAsync(Guid id, bool force)
    {
        var vault = RequireVault();
        var preset = vault.Data.Settings.Presets.FirstOrDefault(p => p.Id == id) ?? throw new UserError(L.T("Preset not found."));
        // Accounts deleted since the preset was saved are left out.
        var accounts = preset.AccountIds.Distinct().Select(FindAccount).OfType<Account>().ToList();
        if (accounts.Count == 0)
            throw new UserError(L.T("This preset has no accounts left."));
        CheckCanLaunch(accounts);
        GameTarget? target = null;
        if (preset.PlaceInput.Length > 0
            && !GameTarget.TryParse(preset.PlaceInput, preset.Server == "job" ? preset.JobId : "", out target, out string error))
            throw new UserError(error);

        // A link that already names a server (or a private one) keeps it.
        string server = preset.Server;
        bool pick = target is { JobId: null, LinkCode: null, ShareCode: null } && server is "most" or "fewest" or "ping" or "random";
        string? notice = null;
        var result = await LaunchAccountsAsync(vault, accounts, target, force, preset.DelaySeconds ?? vault.Data.Settings.LaunchDelaySeconds,
            pick
                ? async (t, party, ct) =>
                {
                    var (picked, note) = await PickServerAsync(t, server, party, ct);
                    notice = note;
                    return picked;
                }
                : null);
        return result with { Notice = notice };
    }

    /// <summary>
    /// A public server with room for all the accounts: the fullest or emptiest (Roblox's own order), the lowest average
    /// ping over two pages, or one at random. With none, or no answer from Roblox, Roblox picks as it normally does.
    /// </summary>
    async Task<(GameTarget Target, string? Note)> PickServerAsync(GameTarget target, string choice, int party, CancellationToken ct)
    {
        if (party == 0)
            return (target, null); // every account is already open; nothing will join
        Progress(L.T("Finding a server…"));
        var servers = new List<ServerInfo>();
        try
        {
            var (first, next) = await RobloxApi.GetServersAsync(target.PlaceId, choice == "fewest", null, ct);
            servers.AddRange(first);
            if (choice == "ping" && next is not null)
                servers.AddRange((await RobloxApi.GetServersAsync(target.PlaceId, false, next, ct)).Servers);
        }
        catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
        {
            // Rate-limited or offline: whatever loaded is used, else Roblox picks.
        }
        var fits = servers.Where(s => s.MaxPlayers - s.Playing >= party).ToList();
        var chosen = choice switch
        {
            "ping" => fits.Where(s => s.Ping > 0).MinBy(s => s.Ping) ?? fits.FirstOrDefault(),
            "random" => fits.Count > 0 ? fits[Random.Shared.Next(fits.Count)] : null,
            _ => fits.FirstOrDefault(),
        };
        if (chosen is not null)
            return (target with { JobId = chosen.JobId }, null);
        string note = L.F("No server has room for {0} accounts right now, so Roblox picks one.", party);
        Toast(note, "warn");
        return (target, note);
    }

    // ───────────────────────────── tray ─────────────────────────────

    /// <summary>The presets for the tray icon's menu; none while locked.</summary>
    public List<(Guid Id, string Name)> TrayPresets() =>
        _vault?.Data.Settings.Presets.Select(p => (p.Id, p.Name)).ToList() ?? [];

    /// <summary>
    /// Runs a preset from the tray icon's menu without opening the window; a warning that needs an answer is asked in
    /// a Windows dialog. Returns a short summary for the tray balloon, or null when there is nothing to say.
    /// </summary>
    public async Task<string?> RunPresetFromTrayAsync(Guid id)
    {
        string text;
        try
        {
            string name = _vault?.Data.Settings.Presets.FirstOrDefault(p => p.Id == id)?.Name ?? "";
            var result = await StartPresetAsync(id, force: false);
            if (result.NeedsConfirm)
            {
                var owner = _owner.Visible ? _owner : null;
                if (MessageBox.Show(owner, result.Message, Ui.Title, MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes)
                    return null;
                result = await StartPresetAsync(id, force: true);
            }
            var lines = new List<string>();
            if (result.Launched > 0)
                lines.Add(L.F("{0}: opened {1} account(s).", name, result.Launched));
            if (result.Notice is { } notice)
                lines.Add(notice);
            lines.AddRange(result.Errors ?? []);
            if (lines.Count == 0)
                return null;
            text = string.Join("\n", lines);
        }
        // Same boundary as a page command: any failure becomes a message instead of a crash.
        catch (Exception ex)
        {
            bool expected = ex is UserError or RobloxApiException;
            if (!expected && !RobloxApi.IsExpectedError(ex))
                Log.Write(ex);
            text = expected ? ex.Message : L.F("Error: {0}", ex.Message);
        }
        // A balloon holds 255 characters.
        return text.Length > 250 ? text[..249] + "…" : text;
    }
}
