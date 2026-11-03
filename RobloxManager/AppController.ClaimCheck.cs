namespace RobloxManager;

public sealed partial class AppController
{
    /// <summary>Whether the window's client isn't signed in as someone other than this account, so a late launch may take it
    /// (see ClaimWindows). Stand-in until the process tick's own check lands with it in AppController.Processes.cs: yes.</summary>
    bool ClaimUserMatches(int pid, Account account) => true;
}
