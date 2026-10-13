using System.Text.Json;
using System.Text.Json.Serialization;

namespace RobloxManager;

// Each vault object keeps fields it doesn't know (Extra), so an older Bloxkeep run after a newer one saves them
// back untouched instead of silently dropping them.

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
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }

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
    /// <summary>The launcher picked by hand (Settings › Open Roblox with › Choose…), used when LaunchVia is "custom".</summary>
    public string CustomLauncher { get; set; } = "";
    /// <summary>Account list as "cards" or "list", and its order; "" until first chosen.</summary>
    public string Layout { get; set; } = "";
    public string Sort { get; set; } = "";
    public int LaunchDelaySeconds { get; set; } = 8;
    public bool MemoryWarnings { get; set; } = true;
    public bool TrimBackground { get; set; }
    /// <summary>Off until the user opts in: it briefly brings Roblox windows to the front while the PC is idle.</summary>
    public bool AntiAfk { get; set; }
    public RobloxConfig RobloxConfig { get; set; } = new();
    public List<RecentGame> RecentGames { get; set; } = new();
    public List<RecentGame> PinnedGames { get; set; } = new();
    public List<LaunchPreset> Presets { get; set; } = new();
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>A saved launch: these accounts, in this order, into this game and kind of server.</summary>
public sealed class LaunchPreset
{
    public Guid Id { get; set; } = Guid.NewGuid();
    public string Name { get; set; } = "";
    public List<Guid> AccountIds { get; set; } = new();
    /// <summary>As typed in the dock (place id or link); empty opens Roblox without a game.</summary>
    public string PlaceInput { get; set; } = "";
    /// <summary>The game's name when the preset was saved, for menus.</summary>
    public string GameName { get; set; } = "";
    /// <summary>"any" (Roblox picks), "most", "fewest", "ping", "random", or "job" (<see cref="JobId"/>).</summary>
    public string Server { get; set; } = "any";
    public string JobId { get; set; } = "";
    /// <summary>Seconds between accounts; null uses the global setting.</summary>
    public int? DelaySeconds { get; set; }
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class RecentGame
{
    public long PlaceId { get; set; }
    public long UniverseId { get; set; }
    public string Name { get; set; } = "";
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

public sealed class VaultData
{
    public int Version { get; set; } = 1;
    public List<Account> Accounts { get; set; } = new();
    public AppSettings Settings { get; set; } = new();
    public List<OpenSession> OpenSessions { get; set; } = new();
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>A Roblox client that was still open when Bloxkeep closed; the next unlock picks it up again.</summary>
public sealed class OpenSession
{
    public Guid AccountId { get; set; }
    public int Pid { get; set; }
    public DateTime StartedAt { get; set; }
    public GameTarget? Target { get; set; }
}
