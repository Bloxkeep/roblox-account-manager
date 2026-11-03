// The app's commands by name: every name its page may send (AppController.cs, DispatchCommandAsync, holds the same list;
// the app answers any other name with "Unknown command"). Host.call and Host.ask take only these, so a misspelt command
// is a type error instead of a toast at run time. Only the type is used, so none of this is in the bundle.
export const COMMANDS = [
  'activity', 'addVault', 'addViaBrowser', 'addViaCookie', 'applyUpdate', 'arrangeWindows', 'backupNow', 'backupSuggestions',
  'cancelAddVault', 'cancelLaunch', 'cancelRejoin', 'changePassword', 'checkAccounts', 'checkUpdates', 'clearThemePicture',
  'clearVaultPicture', 'closeAccounts', 'closeAll', 'closePreset', 'closeVaultPicker', 'copy', 'copyServerCode', 'createVault',
  'deleteAccounts', 'deleteMacro', 'deletePreset', 'deleteSavedTheme', 'deleteServer', 'disableTrayAutostart', 'discordSummaryNow',
  'discordTest', 'duplicateMacro', 'duplicatePreset', 'editOnGame', 'exportActivity', 'exportMacro', 'exportTheme', 'feedbackPreview',
  'findPlayer', 'forgetPlayer', 'friendServers', 'gameDetails', 'gameIcons', 'gameLists', 'getState', 'hidePanel', 'hideRobloxClosed',
  'hideWindows', 'importMacro', 'importTheme', 'joinServer', 'keepView', 'launch', 'lock', 'loginCancel', 'loginCaptcha', 'loginFinish',
  'loginPassword', 'loginTwoStep', 'loginTwoStepCode', 'loginTwoStepSend', 'macroPicture', 'macroTemplates', 'macroTest', 'macroTryRun',
  'macroWatch', 'monitorAct', 'monitorArea', 'moveMacro', 'multiOnly', 'openBackupFolder', 'openDataFolder', 'openDiscord',
  'openDiscordApp', 'openGamePage', 'openGameWiki', 'openGitHub', 'openGuide', 'openProfile', 'openReleaseNotes', 'openRobloxWatch',
  'openWebsite', 'openWebsiteBlank', 'pageBusy', 'pageShown', 'pickBackupFolder', 'pickLauncher', 'pickOnGame', 'pickThemePicture',
  'pickVault', 'pickVaultPicture', 'pinGame', 'playStats', 'presetDiscordTest', 'presetIcons', 'privateServers', 'quickLoginCancel',
  'quickLoginPoll', 'quickLoginStart', 'recordMacro', 'refreshPresence', 'remoteAllow', 'remoteCode', 'removeRecent', 'removeVault',
  'renameVault', 'resetSkipConfirm', 'resolveGame', 'restoreBackup', 'reveal', 'revealDiscordWebhook', 'runMacro', 'runPreset',
  'saveMacro', 'savePreset', 'saveServer', 'saveTheme', 'searchGames', 'seenWhatsNew', 'selectVault', 'sendFeedback', 'servers',
  'setAskVault', 'setAutoRejoin', 'setAutoUpdate', 'setBackup', 'setBetaUpdates', 'setCloseToTray', 'setCountOnline', 'setCustomTheme',
  'setDailySignInCheck', 'setDiscord', 'setDiscordPresence', 'setDiscordWebhook', 'setFpsCap', 'setHideNames', 'setHotkeys',
  'setLanguage', 'setLaunchOrder', 'setNotifications', 'setOption', 'setPresetOrder', 'setPresetSchedule', 'setRemote', 'setRendering',
  'setSettings', 'setTheme', 'setUseRobloxConfig', 'showPanel', 'showPath', 'showWindows', 'skipConfirm', 'solveBotCheck', 'stopMacros',
  'stopRecording', 'switchServer', 'todayStrips', 'toggleMini', 'totpCheck', 'totpCode', 'unlock', 'updateAccount', 'updateAndReopen', 'updateAtLock',
  'updateRoblox', 'useSavedTheme', 'vaultImportApply', 'vaultImportCancel', 'vaultImportOpen', 'vaultImportPick', 'whatsNew', 'window',
] as const;

/** One of the app's commands. */
export type Command = typeof COMMANDS[number];
