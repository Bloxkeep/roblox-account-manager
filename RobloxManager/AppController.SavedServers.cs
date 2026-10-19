using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Servers saved to join again, in the game picker's Saved tab: a Roblox private server's link (picking it puts the
/// link in the game box, so Play sends the selected accounts there), or a code a game asks for inside it to join a
/// server of its own (picking it puts the game in the box and copies the code; Bloxkeep never types into Roblox).
/// </summary>
public sealed partial class AppController
{
    const int MaxSavedServers = 100;
    const int MaxServerName = 60;
    const int MaxServerCode = 100;

    object SavedServerDto(SavedServer s) => new
    {
        id = s.Id,
        name = s.Name,
        placeId = s.PlaceId,
        universeId = s.UniverseId,
        gameName = s.GameName,
        link = s.Link,
        code = s.Code,
        iconUrl = _games.GetValueOrDefault(s.PlaceId)?.IconUrl ?? _iconByUniverse.GetValueOrDefault(s.UniverseId),
    };

    /// <summary>
    /// Saves a server: a private server's link, a code the game asks for, or both. The same link saved again updates the
    /// one already there instead of adding a second.
    /// </summary>
    object SaveServer(JsonElement a)
    {
        var list = RequireVault().Data.Settings.SavedServers;
        string link = Str(a, "link").Trim(), code = Str(a, "code").Trim(), name = Str(a, "name").Trim(), gameName = Str(a, "gameName").Trim();
        long placeId = Long(a, "placeId"), universeId = Long(a, "universeId");
        if (link.Length > 0)
        {
            if (!GameTarget.TryParse(link, "", out var target, out string error))
                throw new UserError(error);
            if (target!.LinkCode is null && target.ShareCode is null)
                throw new UserError(L.T("That link isn't a private server's. Paste the private server's link, or enter the code the game asks for."));
            if (target.PlaceId > 0)
                placeId = target.PlaceId;
        }
        if (link.Length == 0 && code.Length == 0)
            throw new UserError(L.T("Enter the code the game asks for, or put a private server's link in the game box first."));
        if (code.Length > MaxServerCode)
            throw new UserError(L.T("That code is too long."));
        if (link.Length == 0 && placeId <= 0)
            throw new UserError(L.T("Put the game in the game box first, then save its code."));
        if (name.Length == 0)
            name = gameName.Length > 0 ? gameName : L.T("Private server");
        if (name.Length > MaxServerName)
            name = name[..MaxServerName];

        var server = link.Length > 0 ? list.FirstOrDefault(s => s.Link == link) : null;
        if (server is null)
        {
            if (list.Count >= MaxSavedServers)
                throw new UserError(L.F("You can save up to {0} servers. Delete one first.", MaxSavedServers));
            server = new SavedServer();
            list.Insert(0, server);
        }
        server.Name = name;
        server.Link = link;
        server.Code = code;
        server.PlaceId = placeId;
        server.UniverseId = universeId;
        server.GameName = gameName;
        Persist();
        PushState();
        return SavedServerDto(server);
    }

    object? DeleteServer(Guid id)
    {
        if (RequireVault().Data.Settings.SavedServers.RemoveAll(s => s.Id == id) == 0)
            throw new UserError(L.T("That saved server isn't there any more."));
        Persist();
        PushState();
        return null;
    }

    /// <summary>The code a saved server's game asks for, onto the clipboard: the person pastes it in the game.</summary>
    object CopyServerCode(Guid id)
    {
        var server = RequireVault().Data.Settings.SavedServers.FirstOrDefault(s => s.Id == id)
                     ?? throw new UserError(L.T("That saved server isn't there any more."));
        if (server.Code.Length == 0)
            throw new UserError(L.T("This saved server has no code."));
        if (!Ui.CopyToClipboard(server.Code, false))
            throw new UserError(L.T("Couldn't access the clipboard; try again."));
        return new { code = server.Code };
    }
}
