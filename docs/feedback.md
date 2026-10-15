# Send feedback

**Settings › Bloxkeep updates › Send feedback** (or `Ctrl+K`, "feedback") sends an idea or a bug report to Bloxkeep's
developer, on Discord.

## What goes

- What it is (a bug or an idea), the message, and a way to reach the sender if they give one.
- Bloxkeep's version, the Windows version and the app's language.
- When ticked (on for a bug), the last 12 KB of Bloxkeep's `error.log`, with these taken out first: every account's
  name, display name, alias, user id, password and cookie; any Roblox cookie; Discord links; e-mail addresses; long
  tokens; the Windows user and PC names. The vault has to be unlocked for this, since the account names come from it.

Nothing else: no accounts, settings or files. **See exactly what will be sent** shows all of it before it goes.
The app sends at most one a minute (`RobloxManager/AppController.Feedback.cs`).

## The way there

The app posts it as JSON to a Cloudflare Worker (`Feedback/worker`), which posts it to a Discord channel through a
webhook it keeps to itself: the app holds no webhook, so nobody can write to the channel directly. The Worker takes one
JSON body of at most 40 KB, at most 3 sends a minute from one address and 30 from everyone, and posts with mentions
turned off, so `@everyone` in a message pings nobody. The log arrives as a file, `error-log.txt`.

## Setting it up

1. A Discord channel for feedback: Edit Channel › Integrations › Webhooks › New Webhook › Copy Webhook URL. Repository
   secret `FEEDBACK_WEBHOOK` (without it, the Roblox watch's `DISCORD_WEBHOOK` channel gets feedback too).
2. The Cloudflare account and its secrets as in [the Roblox watch's setup](roblox-watch.md#setting-it-up), and the
   account's workers.dev subdomain chosen once: Workers & Pages in the Cloudflare dashboard.
3. Actions › **Cloudflare workers** › Run workflow. Its summary says the feedback address,
   `https://bloxkeep-feedback.<subdomain>.workers.dev/feedback`.
4. That address goes in `FeedbackUrl` (`RobloxManager/AppController.Feedback.cs`). Until then the checks
   (`Tests/Bloxkeep.Tests`, suite `feedback`) fail, so no build goes out with the feature pointing nowhere.

To stop feedback for good or after spam, delete the webhook in Discord (the Worker then answers with an error, which the
app shows), or make a new one and replace `FEEDBACK_WEBHOOK`, then run the workflow again.
