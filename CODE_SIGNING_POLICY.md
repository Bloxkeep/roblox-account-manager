# Code signing policy

Bloxkeep's Windows files are not code-signed yet. Bloxkeep is applying to [SignPath Foundation](https://signpath.org/)
for a free code signing certificate for open source projects. Once the project is accepted, every signed release
carries this notice:

> Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

Until then, check a download against `SHA256SUMS.txt` in the same release (see
[Download and install](README.md#download-and-install)).

## What gets signed

Only files built from this repository's source, by its own build scripts, on GitHub's hosted runners:

- `Bloxkeep.exe` and `BloxkeepUninstall.exe` (the app, inside every package)
- `Bloxkeep-Setup-Offline.exe` (the full installer)
- the update packages and `Bloxkeep-Portable.zip`, which contain the files above

Files from other projects that ship inside Bloxkeep (Microsoft's WebView2 loader, Velopack's updater and installer) keep
their own publishers' signatures or are left unsigned; they are never signed with Bloxkeep's certificate.

`Bloxkeep-Setup.exe`, the small web installer, is kept the same in every release and committed to the repository
(`Installer/Web`); it is built by `Installer/Web/make.bat`, not by the release workflow, so it is not part of what is
sent for signing until it is built there.

## How a release is built and signed

1. The version's notes are added to `RobloxManager/whatsnew.json` and a tag `vX.Y.Z` is pushed to `main`.
2. The [Release workflow](.github/workflows/release.yml) builds everything on a fresh GitHub-hosted Windows machine with
   `build.bat`, after Bloxkeep's checks (`Tests/Bloxkeep.Tests`) pass. The same script builds it on a developer's PC,
   so anyone can rebuild a release from its tag ([Build from source](README.md#build-from-source)).
3. The built files are kept on the workflow run, sent to SignPath for signing, and every signing request is approved by
   hand by an approver below before it is signed.
4. The workflow publishes the signed files as the GitHub release, with `SHA256SUMS.txt`.

Nothing built on a personal machine is ever sent for signing.

## Team roles

Bloxkeep is maintained by one person, who holds every role:

| Role | Who |
|---|---|
| Committers and reviewers (changes from anyone else, such as pull requests, are reviewed before they are merged) | [minhhoang0122](https://github.com/minhhoang0122) |
| Approvers (approve each signing request) | [minhhoang0122](https://github.com/minhhoang0122) |

Everyone with one of these roles signs in to GitHub and SignPath with multi-factor authentication.

Much of Bloxkeep's code is written with Claude, Anthropic's AI coding assistant, so many commits list Claude as their
author or co-author. Claude has no access of its own to the repository, GitHub or SignPath: every change goes in through
the maintainer, who reviews and commits it, and only the maintainer approves signing requests.

## Privacy

This program will not transfer any information to other networked systems unless specifically requested by the user or
the person installing or operating it. What it sends, where and when, is listed in the
[privacy policy](PRIVACY.md).

## Reporting a problem

If a signed file from Bloxkeep's releases behaves in a way this page or the privacy policy doesn't describe, please
[open an issue](https://github.com/minhhoang0122/bloxkeep/issues). Signing will be paused while it is looked into.
