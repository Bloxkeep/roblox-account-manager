using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// Shows the captcha Roblox put in front of a password sign-in, for the user to solve, exactly as roblox.com would show
/// it. The puzzle is Arkose Labs' FunCaptcha, loaded from arkoselabs.roblox.com. The page that hosts it is served by
/// the app itself under www.roblox.com, because that site is unreachable from some networks (Vietnam) while the
/// captcha and sign-in hosts are not. After solving, <see cref="Token"/> holds the proof for Roblox.
/// </summary>
public sealed class CaptchaForm : Form
{
    const string ProfilePrefix = "Bloxkeep-captcha-";
    const string PageUrl = "https://www.roblox.com/bloxkeep/captcha";
    /// <summary>A blocked connection hangs instead of failing, so don't wait for the puzzle forever.</summary>
    static readonly TimeSpan SlowAfter = TimeSpan.FromSeconds(20);

    readonly string _page;
    readonly System.Windows.Forms.Timer _slow = new() { Interval = (int)SlowAfter.TotalMilliseconds };
    WebView2? _web;
    string? _profileDir;
    bool _closing;

    public string? Token { get; private set; }

    public CaptchaForm(string publicKey, string blob)
    {
        _page = BuildPage(publicKey, blob);
        Text = L.T("Solve the puzzle");
        Icon = Ui.AppIcon;
        BackColor = Color.FromArgb(10, 12, 17);
        StartPosition = FormStartPosition.CenterParent;
        ShowInTaskbar = false;
        MinimizeBox = false;
        MaximizeBox = false;
        Size = new Size(LogicalToDeviceUnits(500), LogicalToDeviceUnits(640));
        _slow.Tick += (_, _) =>
        {
            _slow.Stop();
            Say(L.T("Couldn't load the puzzle. Check your internet connection and try again."));
        };
        Load += async (_, _) => await OpenAsync();
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseDarkTitleBar(this, BackColor);
    }

    async Task OpenAsync()
    {
        _profileDir = Path.Combine(Path.GetTempPath(), ProfilePrefix + Guid.NewGuid().ToString("N"));
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = BackColor };
        Controls.Add(web);
        _web = web;
        try
        {
            var env = await CoreWebView2Environment.CreateAsync(null, _profileDir);
            await web.EnsureCoreWebView2Async(env);
            if (_closing || web.IsDisposed)
                return;
            var core = web.CoreWebView2;
            core.Settings.IsPasswordAutosaveEnabled = false;
            core.Settings.IsGeneralAutofillEnabled = false;
            core.Settings.AreDevToolsEnabled = false;
            core.NewWindowRequested += (_, e) => e.Handled = true;
            // Only the host page is answered here; the captcha's own files come from Arkose over the network.
            core.AddWebResourceRequestedFilter(PageUrl, CoreWebView2WebResourceContext.Document);
            core.WebResourceRequested += (_, e) =>
            {
                if (!e.Request.Uri.Equals(PageUrl, StringComparison.Ordinal))
                    return;
                var stream = new MemoryStream(System.Text.Encoding.UTF8.GetBytes(_page));
                e.Response = env.CreateWebResourceResponse(stream, 200, "OK", "Content-Type: text/html; charset=utf-8\r\nCache-Control: no-store");
            };
            core.WebMessageReceived += (_, e) => OnPageMessage(e.WebMessageAsJson);
            _slow.Start();
            core.Navigate(PageUrl);
        }
        catch (WebView2RuntimeNotFoundException)
        {
            Ui.Error(this, L.T("Microsoft Edge WebView2 isn't installed. Install it, or add the account with a cookie."));
            Close();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or UnauthorizedAccessException or ObjectDisposedException)
        {
            if (!_closing)
            {
                Ui.Error(this, L.F("Couldn't open the captcha: {0}", ex.Message));
                Close();
            }
        }
    }

    void OnPageMessage(string json)
    {
        if (_closing)
            return;
        try
        {
            using var doc = JsonDocument.Parse(json);
            var m = doc.RootElement;
            if (m.TryGetProperty("ready", out _))
                _slow.Stop();
            else if (m.TryGetProperty("token", out var token) && token.GetString() is { Length: > 0 } value)
            {
                Token = value;
                DialogResult = DialogResult.OK;
                Close();
            }
            else if (m.TryGetProperty("error", out var error))
            {
                _slow.Stop();
                Log.Note($"Captcha error: {error.GetString()}");
                Say(L.T("The puzzle couldn't load. Close this window and sign in again."));
            }
        }
        catch (JsonException)
        {
        }
    }

    /// <summary>Puts a message in place of the puzzle.</summary>
    void Say(string text)
    {
        if (_web?.CoreWebView2 is { } core)
            core.PostWebMessageAsJson(JsonSerializer.Serialize(new { say = text }));
    }

    string BuildPage(string publicKey, string blob)
    {
        string Js(string s) => JsonSerializer.Serialize(s);
        string key = Uri.EscapeDataString(publicKey);
        return $$"""
            <!doctype html>
            <html lang="{{(L.Language == "vi" ? "vi" : "en")}}">
            <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>Captcha</title>
            <style>
              html, body { margin: 0; height: 100%; background: #0a0c11; color: #b6bccc; font: 14px/1.5 "Segoe UI", system-ui, sans-serif; }
              #msg { position: fixed; inset: 0; display: grid; place-items: center; padding: 24px; text-align: center; }
              button { margin-top: 14px; height: 38px; padding: 0 18px; border: 0; border-radius: 11px; background: #e9ecf3; color: #0b0d12; font: 600 13.5px "Segoe UI", sans-serif; cursor: pointer; }
              [hidden] { display: none !important; }
            </style>
            </head>
            <body>
            <div id="msg"><div><div id="text">{{Esc(L.T("Loading the puzzle…"))}}</div><button id="again" hidden>{{Esc(L.T("Show the puzzle again"))}}</button></div></div>
            <script>
              const post = m => window.chrome.webview.postMessage(m);
              const text = document.getElementById('text'), again = document.getElementById('again');
              let enforcement = null;
              window.chrome.webview.addEventListener('message', e => { if (e.data && e.data.say) { text.textContent = e.data.say; again.hidden = true; } });
              again.onclick = () => { again.hidden = true; enforcement && enforcement.run(); };
              window.setupEnforcement = e => {
                enforcement = e;
                e.setConfig({
                  data: { blob: {{Js(blob)}} },
                  language: {{Js(L.Language == "vi" ? "vi" : "en")}},
                  onReady: () => { post({ ready: true }); text.textContent = ''; e.run(); },
                  onHide: () => { text.textContent = {{Js(L.T("The puzzle was closed."))}}; again.hidden = false; },
                  onCompleted: r => post({ token: r.token }),
                  onError: r => post({ error: String((r && r.error && r.error.error) || 'unknown') }),
                });
              };
            </script>
            <script src="https://arkoselabs.roblox.com/v2/{{key}}/api.js" data-callback="setupEnforcement" async defer></script>
            </body>
            </html>
            """;
    }

    static string Esc(string s) => System.Net.WebUtility.HtmlEncode(s);

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        _closing = true;
        _slow.Stop();
        _slow.Dispose();
        if (_web is not null)
        {
            Controls.Remove(_web);
            _web.Dispose();
            _web = null;
        }
        if (_profileDir is { } dir)
            _ = Task.Run(() => DeleteProfileAsync(dir));
        base.OnFormClosed(e);
    }

    /// <summary>The browser process exits a moment after the control is disposed, so deletion is retried.</summary>
    static async Task DeleteProfileAsync(string dir)
    {
        for (int i = 0; i < 20 && Directory.Exists(dir); i++)
        {
            try
            {
                Directory.Delete(dir, recursive: true);
                return;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                await Task.Delay(500);
            }
        }
    }

    /// <summary>Removes profiles left behind if the app was closed before deletion finished.</summary>
    public static void CleanupLeftoverProfiles()
    {
        try
        {
            foreach (var dir in Directory.EnumerateDirectories(Path.GetTempPath(), ProfilePrefix + "*"))
            {
                try { Directory.Delete(dir, recursive: true); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }
}
