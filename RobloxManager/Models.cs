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
    /// <summary>Auto-rejoin was switched off because Roblox ended the sign-in: on again once it is signed in again.</summary>
    public bool RejoinOffBySignOut { get; set; }
    /// <summary>Opened with the shared Roblox settings when those apply to chosen accounts only.</summary>
    public bool UseRobloxConfig { get; set; }
    /// <summary>The authenticator (2-step) key, null when none. A secret like <see cref="Password"/>: only its current code leaves the app.</summary>
    public TotpKey? Authenticator { get; set; }
    /// <summary>What Roblox's public profile of the account said at the last check (see AppController.Health.cs); null until asked.</summary>
    public bool? Banned { get; set; }
    /// <summary>When the ban was first noticed; null while not banned.</summary>
    public DateTime? BannedSince { get; set; }
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
    /// <summary>With Multi-Roblox on, open every client in a window (Roblox's fullscreen switch is shared by all of them). On by default.</summary>
    public bool MultiInstanceWindowed { get; set; } = true;
    /// <summary>With Multi-Roblox on, tile the Roblox windows once a launch has opened them.</summary>
    public bool MultiInstanceTile { get; set; }
    /// <summary>Maximise each Roblox window Bloxkeep opens (still a window, not Roblox's shared fullscreen); not with <see cref="MultiInstanceTile"/>.</summary>
    public bool MaximizeOnOpen { get; set; }
    public bool RenameWindows { get; set; } = true;
    /// <summary>The account's picture as its Roblox window's icon; null (saved before it had its own switch): as <see cref="RenameWindows"/>.</summary>
    public bool? WindowPictures { get; set; }
    /// <summary>Taskbar buttons: "roblox" (one for every Roblox window, Windows' own), "account" (one each) or "preset" (one group per preset).</summary>
    public string TaskbarGroups { get; set; } = "roblox";
    /// <summary>How Arrange windows lays the Roblox windows out: "fill" (the whole screen), or fixed tiles from the top-left, "small" / "medium" / "large".</summary>
    public string WindowLayout { get; set; } = "fill";
    /// <summary>Replaced by <see cref="LaunchVia"/>; only read to migrate older vaults.</summary>
    public bool DirectLaunch { get; set; }
    public string LaunchVia { get; set; } = "";
    /// <summary>The launcher picked by hand (Settings › Open Roblox with › Choose…), used when LaunchVia is "custom".</summary>
    public string CustomLauncher { get; set; } = "";
    /// <summary>Account list as "cards" or "list", and its order; "" until first chosen.</summary>
    public string Layout { get; set; } = "";
    public string Sort { get; set; } = "";
    public int LaunchDelaySeconds { get; set; } = 8;
    /// <summary>
    /// Minutes without a person's keyboard or mouse input (anywhere on the PC) before Bloxkeep shows its lock screen
    /// again; 0 never. Roblox windows and what runs in the background carry on (see AppController.ScreenLock).
    /// </summary>
    public int AutoLockMinutes { get; set; }
    public bool MemoryWarnings { get; set; } = true;
    public bool TrimBackground { get; set; }
    /// <summary>
    /// Roblox windows in the background run on the CPU's efficiency cores in power-saving mode; the one in front gets
    /// every core (AppController.Cores.cs). Off until the person turns it on.
    /// </summary>
    public bool SplitCores { get; set; }
    /// <summary>Off until the user opts in: it briefly brings Roblox windows to the front while the PC is idle.</summary>
    public bool AntiAfk { get; set; }
    public RobloxConfig RobloxConfig { get; set; } = new();
    public List<RecentGame> RecentGames { get; set; } = new();
    public List<RecentGame> PinnedGames { get; set; } = new();
    /// <summary>Servers saved to join again (the game picker's Saved tab), newest first.</summary>
    public List<SavedServer> SavedServers { get; set; } = new();
    /// <summary>Names last looked up with "Join a player…", newest first (see AppController.JoinPlayer.cs).</summary>
    public List<string> RecentPlayers { get; set; } = new();
    public List<LaunchPreset> Presets { get; set; } = new();
    /// <summary>Accounts in the order Play opens them; accounts missing here (added later) come after, as listed.</summary>
    public List<Guid> LaunchOrder { get; set; } = new();
    /// <summary>
    /// Ids this vault had before its password changes (see <see cref="Vault.Id"/>): backups made under them are still
    /// this vault's to prune.
    /// </summary>
    public List<string> PreviousVaultIds { get; set; } = new();
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
    /// <summary>Runs by itself at set times; null (older presets) or Enabled = false when it doesn't.</summary>
    public PresetSchedule? Schedule { get; set; }
    /// <summary>When it last opened accounts (by hand or by schedule), UTC; null if it hasn't since this was added.</summary>
    public DateTime? LastRunAt { get; set; }
    // Options for the Roblox windows this preset opens (missing in older vaults: they follow the usual settings).
    /// <summary>Anti-AFK for its windows; null follows Settings.</summary>
    public bool? AntiAfk { get; set; }
    /// <summary>Auto-rejoin for its windows; null follows each account's own switch.</summary>
    public bool? AutoRejoin { get; set; }
    /// <summary>"" (Roblox as set up) or "afk": lowest graphics, 30 FPS, no sound, minimised once in the game.</summary>
    public string Profile { get; set; } = "";
    /// <summary>Each of its windows in a game is closed and opened again after this many hours; 0 = never.</summary>
    public int RestartHours { get; set; }
    /// <summary>Its windows are arranged (Settings' layout) once it has opened them; null follows Settings.</summary>
    public bool? Arrange { get; set; }
    /// <summary>
    /// Its own Discord webhook (see AppController.Discord.cs): its schedule messages, the summary after its scheduled
    /// stop and the alerts about the windows it opened go there. "" sends them to the one in Settings.
    /// </summary>
    public string DiscordWebhook { get; set; } = "";
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>When a preset runs on its own (see AppController.Schedule.cs). Times are local wall-clock "HH:mm".</summary>
public sealed class PresetSchedule
{
    public bool Enabled { get; set; }
    public string Time { get; set; } = "";
    /// <summary>Weekdays it repeats on, as <see cref="DayOfWeek"/> numbers (0 = Sunday); empty when it runs once.</summary>
    public List<int> Days { get; set; } = new();
    /// <summary>"yyyy-MM-dd" of a one-off run; empty when it repeats.</summary>
    public string Date { get; set; } = "";
    /// <summary>"never", "after" (<see cref="StopHours"/>) or "at" (<see cref="StopTime"/>).</summary>
    public string Stop { get; set; } = "never";
    public double StopHours { get; set; }
    public string StopTime { get; set; } = "";
    /// <summary>The scheduled minute that last ran, "yyyy-MM-dd HH:mm", so a clock set back can't run it again.</summary>
    public string LastRun { get; set; } = "";
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>
/// A server saved to join again (AppController.SavedServers.cs): a Roblox private server's link, or a code a game asks
/// for inside it to join a server of its own (Bloxkeep opens the game and copies the code; it never types it in).
/// </summary>
public sealed class SavedServer
{
    public Guid Id { get; set; } = Guid.NewGuid();
    public string Name { get; set; } = "";
    public long PlaceId { get; set; }
    public long UniverseId { get; set; }
    public string GameName { get; set; } = "";
    /// <summary>The private server's link as pasted (privateServerLinkCode or a share link); empty for a code only.</summary>
    public string Link { get; set; } = "";
    /// <summary>What the game asks for inside it; empty for a link only.</summary>
    public string Code { get; set; } = "";
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
    /// <summary>Stays in games for the Statistics view (see AppController.Stats.cs); missing in older vaults.</summary>
    public List<PlaySession> PlaySessions { get; set; } = new();
    /// <summary>Statistics › Activity, oldest first (see AppController.Activity.cs); missing in older vaults.</summary>
    public List<ActivityEntry> Activity { get; set; } = new();
    /// <summary>Scheduled runs whose stop is still to come when the app closes; armed again at the next unlock.</summary>
    public List<ScheduledRunState> ScheduledRuns { get; set; } = new();
    /// <summary>Settings › Discord (see AppController.Discord.cs); kept here because the webhook link is a secret.</summary>
    public DiscordSettings Discord { get; set; } = new();
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>Posting to a Discord channel through a webhook: off until the person turns it on.</summary>
public sealed class DiscordSettings
{
    public bool Enabled { get; set; }
    /// <summary>https://discord.com/api/webhooks/{id}/{token}; anyone with it can post to the channel.</summary>
    public string WebhookUrl { get; set; } = "";
    /// <summary>"gone" after Discord said the webhook no longer exists (sending stopped); null otherwise.</summary>
    public string? Stopped { get; set; }
    public string Username { get; set; } = "Bloxkeep";
    public string AvatarUrl { get; set; } = "";
    /// <summary>"app" (follows Bloxkeep's), "en" or "vi".</summary>
    public string Language { get; set; } = "app";
    /// <summary>"embed" or "plain".</summary>
    public string Style { get; set; } = "embed";
    /// <summary>Messages name accounts "Account 1", "Account 2"…, apart from Settings › Appearance's switch for the screen.</summary>
    public bool HideNames { get; set; }
    public bool SummaryDaily { get; set; }
    public string SummaryDailyTime { get; set; } = "22:00";
    public bool SummaryWeekly { get; set; }
    /// <summary>A <see cref="DayOfWeek"/> number (0 = Sunday).</summary>
    public int SummaryWeekday { get; set; }
    public string SummaryWeeklyTime { get; set; } = "22:00";
    public bool SummaryAfterSchedule { get; set; }
    /// <summary>1 (today), 7 or 30.</summary>
    public int SummaryDays { get; set; } = 1;
    /// <summary>Of "total", "accounts", "games", "sessions", "kicks".</summary>
    public List<string> SummaryParts { get; set; } = ["total", "accounts", "games", "sessions", "kicks"];
    /// <summary>Accounts the summary covers; empty for all.</summary>
    public List<Guid> SummaryAccounts { get; set; } = new();
    /// <summary>Local "yyyy-MM-dd" the daily / weekly summary last went out, so neither goes twice (a clock set back included).</summary>
    public string LastDaily { get; set; } = "";
    public string LastWeekly { get; set; } = "";
    public bool Board { get; set; }
    public int BoardMinutes { get; set; } = 5;
    /// <summary>The status message being edited in place; empty until posted (or after it was deleted).</summary>
    public string BoardMessageId { get; set; } = "";
    /// <summary>Event kinds sent (as in AppController.Notify.cs), apart from the Windows notification switches.</summary>
    public List<string> Events { get; set; } = ["drop", "schedule", "signin", "launch"];
    /// <summary>Per message kind ("summary" or an event kind): "none", "here", "everyone", "user:&lt;id&gt;", "role:&lt;id&gt;".</summary>
    public Dictionary<string, string> Pings { get; set; } = new();
    /// <summary>Between these local "HH:mm" times messages still go out, without pings.</summary>
    public bool Quiet { get; set; }
    public string QuietFrom { get; set; } = "23:00";
    public string QuietTo { get; set; } = "07:00";
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>A scheduled preset run (see AppController.Schedule.cs) still waiting for its stop, as written to the vault.</summary>
public sealed class ScheduledRunState
{
    public Guid PresetId { get; set; }
    /// <summary>The preset's name when the run started, for its stop's notice if the preset was deleted since.</summary>
    public string Name { get; set; } = "";
    /// <summary>The accounts the run opened: the ones its stop closes.</summary>
    public List<Guid> Opened { get; set; } = new();
    /// <summary>When to stop (UTC), for "after N hours".</summary>
    public DateTime? StopAt { get; set; }
    /// <summary>Local minute ("yyyy-MM-dd HH:mm") to stop at, for "at a time".</summary>
    public string? StopAtMinute { get; set; }
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>One stay in a game by a client Bloxkeep opened; still going while <see cref="EndedAt"/> is null.</summary>
public sealed class PlaySession
{
    public Guid AccountId { get; set; }
    /// <summary>The account's name when the stay began, shown once the account is deleted; null in stays from older versions.</summary>
    public string? AccountLabel { get; set; }
    /// <summary>The client's process, to go on with the stay after Bloxkeep restarts.</summary>
    public int Pid { get; set; }
    public long PlaceId { get; set; }
    public long UniverseId { get; set; }
    /// <summary>The game's name when the stay began (or once looked up), so the view needs no lookups.</summary>
    public string GameName { get; set; } = "";
    public DateTime StartedAt { get; set; }
    public DateTime? EndedAt { get; set; }
    /// <summary>When the client was last seen still playing: the end of a stay Bloxkeep didn't see end.</summary>
    public DateTime? LastSeenAt { get; set; }
    /// <summary>"left" (by choice), "disconnected" (see <see cref="ReasonCode"/>), "closed" (the window, by anyone), "sleep" (the PC went to sleep) or "appClosed" (Bloxkeep wasn't running).</summary>
    public string? EndReason { get; set; }
    /// <summary>Roblox's disconnect code when it gave one (273 = the same account joined elsewhere, 278 = idle).</summary>
    public int? ReasonCode { get; set; }
    /// <summary>The preset whose window it was (summaries group play by preset); null opened otherwise, or before 1.0.5.</summary>
    public Guid? PresetId { get; set; }
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>One line of Statistics › Activity (see AppController.Activity.cs).</summary>
public sealed class ActivityEntry
{
    /// <summary>When it happened (UTC).</summary>
    public DateTime At { get; set; }
    /// <summary>
    /// "opened", "rejoined", "joined", "left", "disconnected", "signedOut", "closed", "closedByRoblox", or the type of
    /// a notification (AppController.Notify.cs), whose own sentence is in <see cref="Text"/>.
    /// </summary>
    public string Kind { get; set; } = "";
    public Guid? AccountId { get; set; }
    /// <summary>The account's name then, shown once the account is deleted.</summary>
    public string? AccountLabel { get; set; }
    public long? PlaceId { get; set; }
    /// <summary>The game's name, once known (filled in later for a place not looked up yet).</summary>
    public string? GameName { get; set; }
    /// <summary>Roblox's disconnect code, when it gave one.</summary>
    public int? Code { get; set; }
    /// <summary>A notification's own sentence, in English and in Vietnamese, so it shows in the language chosen later.</summary>
    public string? Text { get; set; }
    public string? TextVi { get; set; }
    /// <summary>
    /// The accounts <see cref="Text"/> names, in the order of its placeholders (U+E000, the number, U+E001): written out
    /// when the log is shown, with the name each has then ("Account N" while names are hidden), so a sentence never
    /// holds a name. Null in entries without names, and in older ones (which hold the names themselves).
    /// </summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<ActivityName>? Names { get; set; }
    /// <summary>"ok", "info", "warn" or "error", as the notification was shown; null for the kinds above.</summary>
    public string? Tone { get; set; }
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>An account an activity entry's sentence names (see <see cref="ActivityEntry.Names"/>).</summary>
public sealed class ActivityName
{
    public Guid Id { get; set; }
    /// <summary>The account's name when the entry was noted, shown once the account is deleted.</summary>
    public string Label { get; set; } = "";
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
    /// <summary>The preset that opened it, whose options still apply to it; null for one opened otherwise.</summary>
    public Guid? PresetId { get; set; }
}
