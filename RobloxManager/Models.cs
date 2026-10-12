using System.Text.Json.Serialization;

namespace RobloxManager;

public sealed class Account
{
    public Guid Id { get; set; } = Guid.NewGuid();
    public long UserId { get; set; }
    public string Username { get; set; } = "";
    public string DisplayName { get; set; } = "";
    public string Alias { get; set; } = "";
    public string Group { get; set; } = "";
    public string Note { get; set; } = "";
    public string Password { get; set; } = "";
    public string Cookie { get; set; } = "";
    /// <summary><see cref="Realm.Global"/> (roblox.com) or <see cref="Realm.Vng"/> (Roblox VNG, Vietnam).</summary>
    public string Realm { get; set; } = RobloxManager.Realm.Global;
    public DateTime CreatedAt { get; set; } = DateTime.Now;
    public DateTime? LastUsed { get; set; }
    public DateTime? LastChecked { get; set; }
    public bool? CookieValid { get; set; }
    public bool AutoRejoin { get; set; }
    /// <summary>Opened with the shared Roblox settings when those apply to chosen accounts only.</summary>
    public bool UseRobloxConfig { get; set; }

    [JsonIgnore]
    public string Label => !string.IsNullOrWhiteSpace(Alias) ? Alias.Trim() : Username;
}

public sealed class AppSettings
{
    public string LastPlaceInput { get; set; } = "";
    public string LastJobId { get; set; } = "";
    /// <summary>Off until the user opts in: Roblox does not officially support several clients.</summary>
    public bool MultiInstance { get; set; }
    public bool RenameWindows { get; set; } = true;
    /// <summary>Replaced by <see cref="LaunchVia"/>; only read to migrate older vaults.</summary>
    public bool DirectLaunch { get; set; }
    public string LaunchVia { get; set; } = "";
    public int LaunchDelaySeconds { get; set; } = 8;
    public bool MemoryWarnings { get; set; } = true;
    public bool TrimBackground { get; set; }
    /// <summary>Off until the user opts in: it briefly brings Roblox windows to the front while the PC is idle.</summary>
    public bool AntiAfk { get; set; }
    public RobloxConfig RobloxConfig { get; set; } = new();
    public List<RecentGame> RecentGames { get; set; } = new();
    public List<RecentGame> PinnedGames { get; set; } = new();
}

public sealed class RecentGame
{
    public long PlaceId { get; set; }
    public long UniverseId { get; set; }
    public string Name { get; set; } = "";
}

public sealed class VaultData
{
    public int Version { get; set; } = 1;
    public List<Account> Accounts { get; set; } = new();
    public AppSettings Settings { get; set; } = new();
    public List<OpenSession> OpenSessions { get; set; } = new();
}

/// <summary>A Roblox client that was still open when Bloxkeep closed; the next unlock picks it up again.</summary>
public sealed class OpenSession
{
    public Guid AccountId { get; set; }
    public int Pid { get; set; }
    public DateTime StartedAt { get; set; }
    public GameTarget? Target { get; set; }
}
