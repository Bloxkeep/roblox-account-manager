# Asking Microsoft to look at the installer (SmartScreen)

The files aren't code-signed, so Windows SmartScreen shows "Windows protected your PC" for them until they have built up
reputation. `Bloxkeep-Setup.exe` (the web installer, `Installer\Web`) is the same file in every release, so its
reputation keeps growing; submitting it to Microsoft as its developer asks them to look at it now. There's no promise:
the answer may only be "no malware detected" while SmartScreen still warns until enough people have run it.

Submit after the release is published, so the links below work. Submit it again only if `Installer\Web\make.bat` ever
built a new one (its SHA-256 changes).

## Steps

1. Open <https://www.microsoft.com/en-us/wdsi/filesubmission>, choose **Software developer**, **Continue**, and sign in
   with a Microsoft account (a personal one works).
2. Fill in the form:

   | Field | Value |
   |---|---|
   | Company name | `minhhoang0122` (or your own name) |
   | Security product used to scan the file | the **SmartScreen** entry (Microsoft Defender SmartScreen) |
   | File | `Installer\Web\Bloxkeep-Setup.exe` from the repo (the same file as on the release page) |
   | What do you believe this file is? | **Incorrectly detected as malware/malicious** |
   | Detection name | `SmartScreen: Windows protected your PC (unrecognized app)` |
   | Additional information | the text below |

3. Submit, and keep the submission ID it shows. The result arrives by email and on the "Submission history" page,
   usually within a few days.

## Additional information (paste)

```text
Bloxkeep is a free, open-source Roblox account manager for Windows, made by an independent student developer:
https://github.com/minhhoang0122/bloxkeep

Bloxkeep-Setup.exe is its web installer. It is published unchanged in every release:
https://github.com/minhhoang0122/bloxkeep/releases/latest/download/Bloxkeep-Setup.exe
SHA-256: 4aad1b9b49ffeefe947f56caf71102ceb934ec9e064a17fb9788128a5d5f4548

It downloads the newest Bloxkeep-Setup-Offline.exe from the project's GitHub releases over HTTPS, checks it against
that release's SHA256SUMS.txt and installs Bloxkeep for the current Windows user, without admin rights, into a folder
the user picks. It is built from the source in the repository (Installer/Setup/Setup.cs and Installer/Web/Web.cs).

The files are not code-signed yet, so Microsoft Defender SmartScreen shows "Windows protected your PC" for this
unrecognized app. Please review the file and clear the warning if you find it clean. Thank you.
```

## The offline installer

`Bloxkeep-Setup-Offline.exe` is a new file at every release (it carries that version), so its reputation starts over
each time. It can be submitted the same way (Detection name and text as above, with its own SHA-256 from
`SHA256SUMS.txt`), but the web installer is the one worth the effort.
