//! Roblox's own connection error codes (Enum.ConnectionError, 113 items with OK): each one's name as Roblox
//! names it, and the words Bloxkeep says for it. The numbers and names are the client's API dump; which codes Roblox's
//! error box offers no Reconnect after, and which it reconnects by itself, are Connection.lua's two lists (CoreScripts,
//! read 2026-11-11). One table for the core (activity, alerts, rejoin) and for the pages (an account's tile).

#[cfg_attr(not(feature = "core"), allow(dead_code))]
pub struct Code {
    pub code: i32,
    /// Enum.ConnectionError's name for it.
    pub name: &'static str,
    /// The page's words are "kick.<key>" (assets/i18n.json, assets/words.json).
    pub key: &'static str,
    /// The core's English words (their Vietnamese is in core/l.rs).
    pub words: &'static str,
}

/// Every code but 0 (OK), by number.
pub static CODES: [Code; 112] = [
    Code { code: 1, name: "Unknown", key: "error", words: "connection error" },
    Code { code: 2, name: "ConnectErrors", key: "connect", words: "couldn't join the server" },
    Code { code: 3, name: "AlreadyConnected", key: "connect", words: "couldn't join the server" },
    Code { code: 4, name: "NoFreeIncomingConnections", key: "connect", words: "couldn't join the server" },
    Code { code: 5, name: "ConnectionBanned", key: "blocked", words: "connection blocked by Roblox" },
    Code { code: 6, name: "InvalidPassword", key: "update", words: "Roblox needs an update" },
    Code { code: 7, name: "IncompatibleProtocolVersion", key: "update", words: "Roblox needs an update" },
    Code { code: 8, name: "IPRecentlyConnected", key: "connect", words: "couldn't join the server" },
    Code { code: 9, name: "OurSystemRequiresSecurity", key: "update", words: "Roblox needs an update" },
    Code { code: 10, name: "SecurityKeyMismatch", key: "error", words: "connection error" },
    Code { code: 256, name: "DisconnectErrors", key: "error", words: "connection error" },
    Code { code: 257, name: "DisconnectBadhash", key: "error", words: "connection error" },
    Code { code: 258, name: "DisconnectSecurityKeyMismatch", key: "error", words: "connection error" },
    Code { code: 259, name: "DisconnectProtocolMismatch", key: "update", words: "Roblox needs an update" },
    Code { code: 260, name: "DisconnectReceivePacketError", key: "network", words: "connection lost" },
    Code { code: 261, name: "DisconnectReceivePacketStreamError", key: "network", words: "connection lost" },
    Code { code: 262, name: "DisconnectSendPacketError", key: "network", words: "connection lost" },
    Code { code: 263, name: "DisconnectIllegalTeleport", key: "badTeleport", words: "invalid teleport" },
    Code { code: 264, name: "DisconnectDuplicatePlayer", key: "duplicate", words: "already in another game" },
    Code { code: 265, name: "DisconnectDuplicateTicket", key: "duplicate", words: "already in another game" },
    Code { code: 266, name: "DisconnectTimeout", key: "network", words: "connection lost" },
    Code { code: 267, name: "DisconnectLuaKick", key: "game", words: "kicked by the game" },
    Code { code: 268, name: "DisconnectOnRemoteSysStats", key: "client", words: "kicked for unexpected client behaviour" },
    Code { code: 269, name: "DisconnectHashTimeout", key: "network", words: "connection lost" },
    Code { code: 270, name: "DisconnectCloudEditKick", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 271, name: "DisconnectPlayerless", key: "closing", words: "server was shutting down" },
    Code { code: 272, name: "DisconnectNewSecurityKeyMismatch", key: "error", words: "connection error" },
    Code { code: 273, name: "DisconnectEvicted", key: "elsewhere", words: "signed in on another device" },
    Code { code: 274, name: "DisconnectDevMaintenance", key: "devShutdown", words: "server shut down by the developer" },
    Code { code: 275, name: "DisconnectRobloxMaintenance", key: "maintenance", words: "server shut down by Roblox for maintenance" },
    Code { code: 276, name: "DisconnectRejoin", key: "elsewhere", words: "signed in on another device" },
    Code { code: 277, name: "DisconnectConnectionLost", key: "network", words: "connection lost" },
    Code { code: 278, name: "DisconnectIdle", key: "idle", words: "kicked for being idle" },
    Code { code: 279, name: "DisconnectRaknetErrors", key: "connect", words: "couldn't join the server" },
    Code { code: 280, name: "DisconnectWrongVersion", key: "update", words: "Roblox needs an update" },
    Code { code: 281, name: "DisconnectBySecurityPolicy", key: "blocked", words: "connection blocked by Roblox" },
    Code { code: 282, name: "DisconnectBlockedIP", key: "blocked", words: "connection blocked by Roblox" },
    Code { code: 284, name: "DisconnectClientFailure", key: "clientFailure", words: "Roblox client failure" },
    Code { code: 285, name: "DisconnectClientRequest", key: "left", words: "left the game" },
    Code { code: 286, name: "DisconnectPrivateServerKickout", key: "private", words: "no access to the private server any more" },
    Code { code: 287, name: "DisconnectModeratedGame", key: "moderated", words: "server closed by Roblox moderation" },
    Code { code: 288, name: "ServerShutdown", key: "shutdown", words: "server shut down" },
    Code { code: 290, name: "ReplicatorTimeout", key: "network", words: "connection lost" },
    Code { code: 291, name: "PlayerRemoved", key: "removed", words: "removed from the game" },
    Code { code: 292, name: "DisconnectOutOfMemoryKeepPlayingLeave", key: "memory", words: "device memory is low" },
    Code { code: 293, name: "DisconnectRomarkEndOfTest", key: "error", words: "connection error" },
    Code { code: 294, name: "DisconnectCollaboratorPermissionRevoked", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 295, name: "DisconnectCollaboratorUnderage", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 296, name: "NetworkInternal", key: "network", words: "connection lost" },
    Code { code: 297, name: "NetworkSend", key: "network", words: "connection lost" },
    Code { code: 298, name: "NetworkTimeout", key: "network", words: "connection lost" },
    Code { code: 299, name: "NetworkMisbehavior", key: "error", words: "connection error" },
    Code { code: 300, name: "NetworkSecurity", key: "security", words: "security check failed" },
    Code { code: 301, name: "ReplacementReady", key: "replaced", words: "server replaced by a new one" },
    Code { code: 302, name: "ServerEmpty", key: "empty", words: "server was empty" },
    Code { code: 303, name: "PhantomFreeze", key: "frozen", words: "connection frozen" },
    Code { code: 304, name: "AndroidAnticheatKick", key: "device", words: "this device isn't allowed" },
    Code { code: 305, name: "AndroidEmulatorKick", key: "device", words: "this device isn't allowed" },
    Code { code: 306, name: "AndroidRootedKick", key: "device", words: "this device isn't allowed" },
    Code { code: 307, name: "ScreentimeLockoutKick", key: "screenTime", words: "screen time limit reached" },
    Code { code: 308, name: "DisconnectionNotification", key: "byServer", words: "disconnected by the server" },
    Code { code: 309, name: "DisconnectVerboselyModeratedGame", key: "moderated", words: "server closed by Roblox moderation" },
    Code { code: 310, name: "DisconnectCollaboratorNotAgeVerified", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 311, name: "DisconnectCollaboratorTrustedConnectionsRequired", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 312, name: "DisconnectCollaboratorOwnerActionRequired", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 313, name: "DisconnectCollaboratorTooManyCollaborators", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 314, name: "DisconnectCollaboratorUnknownError", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 315, name: "DisconnectCollaboratorRequestedEviction", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 316, name: "DisconnectCollaboratorTrustedConnectionsRequiredPC", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 317, name: "DisconnectRemoteAttestationUnsupported", key: "device", words: "this device isn't allowed" },
    Code { code: 318, name: "DisconnectRemoteAttestationGeneralFailure", key: "device", words: "this device isn't allowed" },
    Code { code: 319, name: "DisconnectRemoteAttestationTimeout", key: "deviceCheck", words: "device check timed out" },
    Code { code: 320, name: "DisconnectRemoteAttestationOSOutOfDate", key: "device", words: "this device isn't allowed" },
    Code { code: 321, name: "DisconnectRemoteAttestationBootValidationFailure", key: "device", words: "this device isn't allowed" },
    Code { code: 512, name: "PlacelaunchErrors", key: "join", words: "couldn't join the game" },
    Code { code: 515, name: "PlacelaunchDisabled", key: "restricted", words: "the place is restricted" },
    Code { code: 516, name: "PlacelaunchError", key: "noServer", words: "server not found" },
    Code { code: 517, name: "PlacelaunchGameEnded", key: "ended", words: "server closed" },
    Code { code: 518, name: "PlacelaunchGameFull", key: "full", words: "game is full" },
    Code { code: 522, name: "PlacelaunchUserLeft", key: "followLeft", words: "the player you joined has left" },
    Code { code: 523, name: "PlacelaunchRestricted", key: "accessGone", words: "no access to the game any more" },
    Code { code: 524, name: "PlacelaunchUnauthorized", key: "noPermission", words: "no permission to join" },
    Code { code: 525, name: "PlacelaunchFlooded", key: "busy", words: "server busy" },
    Code { code: 526, name: "PlacelaunchHashExpired", key: "unavailable", words: "game unavailable right now" },
    Code { code: 527, name: "PlacelaunchHashException", key: "unavailable", words: "game unavailable right now" },
    Code { code: 528, name: "PlacelaunchPartyCannotFit", key: "party", words: "party too large" },
    Code { code: 529, name: "PlacelaunchHttpError", key: "http", words: "HTTP error from Roblox" },
    Code { code: 533, name: "PlacelaunchUserPrivacyUnauthorized", key: "privacy", words: "privacy settings block this server" },
    Code { code: 541, name: "PlacelaunchVipOwnerNotPresent", key: "ownerAway", words: "the private server's owner isn't in it" },
    Code { code: 542, name: "PlacelaunchAgeVerificationRequired", key: "ageCheck", words: "age check required" },
    Code { code: 543, name: "PlacelaunchParentalApprovalRequired", key: "parent", words: "a parent's approval is required" },
    Code { code: 544, name: "PlacelaunchCoreGated", key: "ageLocked", words: "locked by age" },
    Code { code: 545, name: "PlacelaunchCollaborationCoreGated", key: "teamCreate", words: "Team Create access lost" },
    Code { code: 600, name: "PlacelaunchCreatorBan", key: "gameBan", words: "banned by the game" },
    Code { code: 601, name: "PlacelaunchDeviceBlock", key: "device", words: "this device isn't allowed" },
    Code { code: 610, name: "PlacelaunchCustomMessage", key: "refused", words: "join refused by the game" },
    Code { code: 611, name: "PlacelaunchOtherError", key: "trouble", words: "Roblox is having technical trouble" },
    Code { code: 768, name: "TeleportErrors", key: "teleport", words: "teleport failed" },
    Code { code: 769, name: "TeleportFailure", key: "teleport", words: "teleport failed" },
    Code { code: 770, name: "TeleportGameNotFound", key: "teleportNotFound", words: "teleport failed: place not found" },
    Code { code: 771, name: "TeleportGameEnded", key: "teleportEnded", words: "teleport failed: server no longer available" },
    Code { code: 772, name: "TeleportGameFull", key: "teleportFull", words: "teleport failed: server full" },
    Code { code: 773, name: "TeleportUnauthorized", key: "teleportRestricted", words: "teleport failed: place is restricted" },
    Code { code: 774, name: "TeleportFlooded", key: "teleportFlooded", words: "teleport failed: too many requests" },
    Code { code: 775, name: "TeleportIsTeleporting", key: "teleportBusy", words: "teleport failed: already teleporting" },
    Code { code: 1024, name: "TransportErrors", key: "error", words: "connection error" },
    Code { code: 1025, name: "DisconnectTransportIoError", key: "error", words: "connection error" },
    Code { code: 1026, name: "DisconnectTransportIoInternetError", key: "network", words: "connection lost" },
    Code { code: 1027, name: "DisconnectTransportProtocolError", key: "error", words: "connection error" },
    Code { code: 1028, name: "DisconnectTransportNgtcp2Error", key: "error", words: "connection error" },
    Code { code: 1029, name: "DisconnectTransportQuicError", key: "error", words: "connection error" },
    Code { code: 1030, name: "DisconnectTransportRnaError", key: "error", words: "connection error" },
];

/// A code's row; None for 0 and for a number Roblox's table doesn't have.
pub fn why(code: i32) -> Option<&'static Code> {
    CODES.binary_search_by_key(&code, |c| c.code).ok().map(|i| &CODES[i])
}

/// reconnectDisabledList: Roblox's error box shows only Leave after these.
#[cfg_attr(not(feature = "core"), allow(dead_code))]
pub const NO_RECONNECT: [i32; 44] = [5, 6, 7, 8, 9, 257, 258, 259, 263, 264, 265, 267, 268, 270, 272, 280, 281, 282, 284, 286, 294, 295, 304, 305, 306, 307, 317, 318, 320, 321, 522, 523, 524, 525, 526, 527, 533, 541, 542, 543, 544, 545, 600, 601];

/// autoReconnectAllowedList: Roblox reconnects by itself after these (a lost connection).
#[cfg_attr(not(feature = "core"), allow(dead_code))]
pub const AUTO_RECONNECT: [i32; 12] = [260, 261, 262, 266, 269, 277, 279, 290, 296, 297, 298, 1026];

/// TeleportErrors and its items: the teleport failed and Roblox's box has only OK, so the player stays where they were.
#[cfg_attr(not(feature = "core"), allow(dead_code))]
pub fn teleport_failed(code: i32) -> bool {
    (768..=775).contains(&code)
}
