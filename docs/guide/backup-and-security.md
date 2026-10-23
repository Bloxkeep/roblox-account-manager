[Tiếng Việt](backup-and-security.vi.md)

# Backup and security

The master password and what can't be recovered, automatic backups, restoring, moving to a new PC, importing from another Bloxkeep file, where your data lives, what leaves the PC and Hide account names.

<img src="../screenshots/settings-backup.png" width="760" alt="Settings › Backup & data: Back up to a folder with the chosen OneDrive folder and Back up now, Restore, Import accounts from a vault file, and the data location">

## Quick steps

1. Open **Settings › Backup & data**.
2. Under **Back up to a folder**, press **Choose…** and pick a folder outside Bloxkeep's own data folder. Pick a Google Drive or OneDrive folder if you can: **Use Google Drive** and **Use OneDrive** appear when Bloxkeep found one.
3. Press **Back up now**. The row then reads "Last backup: …".
4. On a new PC, choose **Restore from a backup** on the first screen and unlock with the password the backup was saved with.

## The master password

The first time Bloxkeep opens, it asks you to **Set a master password**. It encrypts everything you save in Bloxkeep.

- At least 6 characters, and not only spaces. The **Weak / OK / Strong** bar is only a guide.
- Each start shows **Welcome back**: type the master password and press **Unlock**. A warning shows while Caps Lock is on.
- To change it: **Settings › Security**. Backups made before the change still open only with the old password.
- **Lock automatically** (**Settings › Security**): when nobody has used the PC's keyboard or mouse for the time you pick (5 minutes to 2 hours), Bloxkeep shows this screen again. Roblox, auto-rejoin, scheduled presets and Discord keep running behind it. The **Lock** button still closes Bloxkeep completely: those pause until you unlock.

> [!WARNING]
> The master password can't be recovered. Bloxkeep never stores it, so if you forget it, your saved accounts are gone. The only way back is a backup whose password you remember.

<img src="../screenshots/lock.png" width="420" alt="The Welcome back screen: a Master password box and the Unlock button">

## Automatic backups

**Back up to a folder** copies the encrypted vault into a folder, so your accounts survive a dead PC.

- **Folder**: **Choose…**, or a synced folder Bloxkeep found. **Open folder** opens it. It must be outside Bloxkeep's own data folder.
- **Back up now** is greyed out until a folder is chosen. The row says "Last backup: …" or "No backups yet".
- Bloxkeep backs up after changes (at most every 10 minutes), when you lock or quit if something changed, and right after you change the master password or pick a new folder.
- It keeps the 10 newest backups, plus the newest of each day for 30 days. Your OneDrive or Google Drive app uploads them.
- If your master password is easy to guess and the folder is a cloud folder, a note suggests a longer one.

## Restoring and moving to a new PC

A backup opens with the password it was saved with.

| Way | Use it when | What happens |
|---|---|---|
| **Restore from a backup** (first screen) or **Settings › Backup & data › Restore › Restore…** | A new PC, or to go back to a backup. | Always asks first. Your current data is kept as a copy, then Bloxkeep locks: unlock with the backup's password. |
| **Settings › Backup & data › Import accounts from a vault file › Import…** | You want accounts from another vault or backup. | Adds accounts; yours stay as they are. |

## Importing from another Bloxkeep file

Bring accounts from `vault.rmv` on another PC, or from a `Bloxkeep-backup-…` file. It is in three places: **Add account › Other ways to add › Import from another Bloxkeep vault file…**, **Import from: Bloxkeep file** in **Sign in to Roblox**, and **Settings › Backup & data › Import…**.

1. Press **Choose file…** and pick the `.rmv` file.
2. Type **Master password of this file** and press **Open**.
3. Tick the accounts to bring over. Each shows **New** or **Already here**, and whether it is signed in, needs sign-in, wasn't checked, or has a password saved. **All** / **None** tick everything or nothing.
4. Optionally tick **Also import presets (N)**.
5. Press **Import**.

- The file itself isn't changed. Imported accounts are locked with this Bloxkeep's master password.
- For an account already here, only a sign-in that no longer works is replaced. Your names, groups and notes stay.

<details>
<summary>More details</summary>

Imported presets come with their schedules switched off, and accounts that aren't here are left out of them. You can keep up to 50 presets. After three wrong passwords, **Open** counts down before the next try. You can't import the vault you are using.

</details>

## Where your data lives

**Settings › Backup & data › Data location › Open folder** shows it. For the installed app it is `%LOCALAPPDATA%\Bloxkeep`; the portable copy uses a `data` folder next to `Bloxkeep.exe`.

| File | What it is |
|---|---|
| `vault.rmv` | Your accounts, encrypted. |
| `vault.rmv.bak` | The previous save. |
| `error.log` | Errors, without account details. |

> [!WARNING]
> Never send anyone `vault.rmv` or a backup file. The full list is in the [README](../../README.md#data-and-security).

## What leaves the PC

Bloxkeep has no accounts or analytics of its own. Your accounts, settings and history stay on your PC, in a vault encrypted with your master password, which is never stored. Data only leaves your PC for what you use:

| Where | What |
|---|---|
| Roblox | Signing in, status, games and servers. |
| GitHub | Bloxkeep updates, with no account data. |
| Your backup folder | The encrypted vault. |
| Your Discord webhook | Only when you turn it on. Never cookies or passwords. |
| Bloxkeep's developer | Feedback you choose to send. |
| Bloxkeep's Discord bot | While Bloxkeep is open, an anonymous "open now" note every minute (a code made from the PC that can't be traced back, the version and the language), so the developer can see how many people use Bloxkeep right now. Turn it off in **Settings › Bloxkeep updates › Privacy › Add me to the online count**. |

The full list is in the [privacy policy](../../PRIVACY.md).

## Hide account names

**Settings › Appearance › Hide account names** shows "Account 1", "Account 2"… (each account's place in the list) everywhere instead of names, Roblox window titles included.

- @usernames disappear and **Edit** hides the user ID.
- The **Nickname** box can't be edited meanwhile.
- The search still finds accounts by their real names.
- Discord messages have their own switch: **Hide account names in messages** (see [Discord](discord.md)).

> [!TIP]
> Turn it on before screenshots or streams.

← [User guide](../guide.md) · Next: [Troubleshooting](troubleshooting.md) →
