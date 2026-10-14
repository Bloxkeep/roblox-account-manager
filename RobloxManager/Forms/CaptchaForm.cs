using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// Shows the captcha Roblox put in front of a password sign-in, for the user to solve, as roblox.com would show it:
/// <list type="bullet">
/// <item>Arkose Labs' FunCaptcha ("captcha"), loaded from arkoselabs.roblox.com; <see cref="Token"/> is its token.</item>
/// <item>HUMAN Security's press-and-hold ("captchav2"), fetched from apis.roblox.com/v2/captcha; <see cref="Token"/> is
/// Roblox's redemption token.</item>
/// </list>
/// The page that hosts the puzzle is served by the app itself under the realm's site (www.roblox.com), because that site is unreachable
/// from some networks (Vietnam) while the captcha and API hosts are not. HUMAN's "first-party" requests, which would go
/// to www.roblox.com as well, are passed on to HUMAN's own hosts.
/// </summary>
public sealed class CaptchaForm : Form
{
    const string ProfilePrefix = "Bloxkeep-captcha-";
    /// <summary>A blocked connection hangs instead of failing, so don't wait for the puzzle forever.</summary>
    static readonly TimeSpan SlowAfter = TimeSpan.FromSeconds(20);
    static readonly HttpClient Relay = new(new SocketsHttpHandler { UseCookies = false, AutomaticDecompression = System.Net.DecompressionMethods.All })
    {
        Timeout = TimeSpan.FromSeconds(20),
    };
    /// <summary>HUMAN's first-party routes: /{app id without "PX"}/init.js, /xhr/…, /captcha/….</summary>
    static readonly Regex FirstParty = new(@"^/(?<app>[A-Za-z0-9]{6,12})/(?<kind>init\.js|xhr|captcha)(?<rest>/.*)?$", RegexOptions.Compiled);

    readonly string _site, _pageUrl, _page;
    readonly bool _relayHuman;
    readonly System.Windows.Forms.Timer _slow = new() { Interval = (int)SlowAfter.TotalMilliseconds };
    readonly HashSet<string> _unknownPaths = new();
    WebView2? _web;
    string? _profileDir;
    bool _closing;
    /// <summary>From opening the window to the puzzle being ready, and to it being solved: written to the log.</summary>
    readonly System.Diagnostics.Stopwatch _clock = System.Diagnostics.Stopwatch.StartNew();
    bool _readyNoted;

    /// <summary>
    /// One browser profile kept for the puzzle windows (not a fresh one each time): Arkose's scripts stay cached and it
    /// sees a browser it already knows, so its "Verifying browser…" step doesn't start from nothing every time. It
    /// only ever holds the puzzle pages' own data — never a Roblox sign-in, which goes through the app.
    /// </summary>
    public static string ProfileDir => Path.Combine(Vault.DataDir, "captcha");

    public string? Token { get; private set; }

    CaptchaForm(string site, string page, bool relayHuman)
    {
        _site = site;
        _pageUrl = site + "/bloxkeep/captcha";
        _page = page;
        _relayHuman = relayHuman;
        Text = L.T("Solve the puzzle");
        Icon = Ui.AppIcon;
        BackColor = Themes.Current.Background;
        StartPosition = FormStartPosition.CenterParent;
        ShowInTaskbar = false;
        MinimizeBox = false;
        MaximizeBox = false;
        Size = relayHuman ? new Size(LogicalToDeviceUnits(460), LogicalToDeviceUnits(340)) : new Size(LogicalToDeviceUnits(500), LogicalToDeviceUnits(640));
        _slow.Tick += (_, _) =>
        {
            _slow.Stop();
            Say(L.T("Couldn't load the puzzle. Check your internet connection and try again."));
        };
        Load += async (_, _) => await OpenAsync();
    }

    /// <summary>Arkose Labs' FunCaptcha, on the site of the realm whose key it is (as the press-and-hold is).</summary>
    public static CaptchaForm Arkose(string realm, string publicKey, string blob) =>
        new(Realm.Website(realm), ArkosePage(publicKey, blob), relayHuman: false);

    /// <summary>HUMAN's press-and-hold, for the "captchav2" challenge with this id.</summary>
    public static CaptchaForm Human(string realm, string captchaChallengeId) =>
        new(Realm.Website(realm), HumanPage(Realm.Api(realm, "apis"), captchaChallengeId), relayHuman: true);

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, Themes.Current);
    }

    async Task OpenAsync()
    {
        _profileDir = ProfileDir;
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = BackColor };
        Controls.Add(web);
        _web = web;
        try
        {
            var env = await CoreWebView2Environment.CreateAsync(null, _profileDir, WebView2Setup.EnvironmentOptions());
            await web.EnsureCoreWebView2Async(env);
            if (_closing || web.IsDisposed)
                return;
            var core = web.CoreWebView2;
            core.Settings.IsPasswordAutosaveEnabled = false;
            core.Settings.IsGeneralAutofillEnabled = false;
            core.Settings.AreDevToolsEnabled = false;
            core.NewWindowRequested += (_, e) => e.Handled = true;
            // The page itself, and (for HUMAN) anything else asked of the site, is answered here; the rest goes out.
            core.AddWebResourceRequestedFilter(_relayHuman ? _site + "/*" : _pageUrl, CoreWebView2WebResourceContext.All);
            core.WebResourceRequested += (_, e) => OnSiteRequest(env, e);
            core.WebMessageReceived += (_, e) => OnPageMessage(e.WebMessageAsJson);
            _slow.Start();
            core.Navigate(_pageUrl);
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

    void OnSiteRequest(CoreWebView2Environment env, CoreWebView2WebResourceRequestedEventArgs e)
    {
        var uri = new Uri(e.Request.Uri);
        if (uri.AbsolutePath == "/bloxkeep/captcha")
        {
            var stream = new MemoryStream(Encoding.UTF8.GetBytes(_page));
            e.Response = env.CreateWebResourceResponse(stream, 200, "OK", "Content-Type: text/html; charset=utf-8\r\nCache-Control: no-store");
            return;
        }
        if (!_relayHuman)
            return;
        var m = FirstParty.Match(uri.AbsolutePath);
        if (!m.Success)
        {
            // Not something this window knows how to serve; it goes to the network (and fails where the site is blocked).
            if (_unknownPaths.Add(uri.AbsolutePath))
                Log.Note($"Captcha page asked the site for {uri.AbsolutePath}");
            return;
        }
        string app = "PX" + m.Groups["app"].Value;
        string rest = m.Groups["rest"].Value + uri.Query;
        string target = m.Groups["kind"].Value switch
        {
            "init.js" => $"https://client.px-cloud.net/{app}/main.min.js",
            "xhr" => $"https://collector-{app.ToLowerInvariant()}.px-cloud.net{rest}",
            _ => $"https://captcha.px-cdn.net/{app}{rest}",
        };
        _ = RelayAsync(env, e, target);
    }

    /// <summary>Passes one of HUMAN's first-party requests on to HUMAN's own host and hands the answer back to the page.</summary>
    async Task RelayAsync(CoreWebView2Environment env, CoreWebView2WebResourceRequestedEventArgs e, string target)
    {
        var deferral = e.GetDeferral();
        try
        {
            using var req = new HttpRequestMessage(new HttpMethod(e.Request.Method), target);
            byte[]? body = null;
            if (e.Request.Content is { } content)
            {
                using var ms = new MemoryStream();
                await content.CopyToAsync(ms);
                body = ms.ToArray();
            }
            string? contentType = null;
            foreach (var h in e.Request.Headers)
            {
                if (h.Key.Equals("Content-Type", StringComparison.OrdinalIgnoreCase))
                    contentType = h.Value;
                else if (!h.Key.Equals("Host", StringComparison.OrdinalIgnoreCase) && !h.Key.Equals("Content-Length", StringComparison.OrdinalIgnoreCase))
                    req.Headers.TryAddWithoutValidation(h.Key, h.Value);
            }
            if (body is not null)
            {
                req.Content = new ByteArrayContent(body);
                if (contentType is not null)
                    req.Content.Headers.TryAddWithoutValidation("Content-Type", contentType);
            }
            using var res = await Relay.SendAsync(req);
            byte[] answer = await res.Content.ReadAsByteArrayAsync();
            string type = res.Content.Headers.ContentType?.ToString() ?? "application/octet-stream";
            if (!_closing)
                e.Response = env.CreateWebResourceResponse(new MemoryStream(answer), (int)res.StatusCode, res.ReasonPhrase ?? "",
                    $"Content-Type: {type}\r\nCache-Control: no-store");
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or COMException or InvalidOperationException)
        {
            Log.Note($"Captcha relay to {new Uri(target).Host} failed: {ex.Message}");
            if (!_closing)
                e.Response = env.CreateWebResourceResponse(null, 502, "Bad Gateway", "");
        }
        finally
        {
            deferral.Complete();
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
            {
                _slow.Stop();
                if (!_readyNoted)
                    Log.Note($"Captcha ready after {_clock.Elapsed.TotalSeconds:0.0} s");
                _readyNoted = true;
            }
            else if (m.TryGetProperty("log", out var note))
                Log.Note($"Captcha: {note.GetString()}");
            else if (m.TryGetProperty("token", out var token) && token.GetString() is { Length: > 0 } value)
            {
                Token = value;
                Log.Note($"Captcha solved after {_clock.Elapsed.TotalSeconds:0.0} s");
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

    static string Js(string s) => JsonSerializer.Serialize(s);
    static string Esc(string s) => System.Net.WebUtility.HtmlEncode(s);
    static string Lang => L.Language == "vi" ? "vi" : "en";

    static string Hex(Color c) => $"#{c.R:x2}{c.G:x2}{c.B:x2}";

    /// <summary>The page around the puzzle, in the app's theme.</summary>
    static string Style => $$"""
        html, body { margin: 0; height: 100%; background: {{Hex(Themes.Current.Background)}}; color: {{Hex(Themes.Current.Text2)}}; font: 14px/1.5 "Segoe UI", system-ui, sans-serif; }
        #msg { position: fixed; inset: 0; display: grid; place-items: center; padding: 24px; text-align: center; }
        #msg > div { width: min(340px, 100%); }
        #px-captcha { margin-top: 16px; }
        button { margin-top: 14px; height: 38px; padding: 0 18px; border: 0; border-radius: 11px; background: {{Hex(Themes.Current.Text)}}; color: {{Hex(Themes.Current.Background)}}; font: 600 13.5px "Segoe UI", sans-serif; cursor: pointer; }
        [hidden] { display: none !important; }
        """;

    static string ArkosePage(string publicKey, string blob) => $$"""
        <!doctype html>
        <html lang="{{Lang}}">
        <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>Captcha</title>
        <style>{{Style}}</style>
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
              language: {{Js(Lang)}},
              onReady: () => { post({ ready: true }); text.textContent = ''; e.run(); },
              onHide: () => { text.textContent = {{Js(L.T("The puzzle was closed."))}}; again.hidden = false; },
              onCompleted: r => post({ token: r.token }),
              onError: r => post({ error: String((r && r.error && r.error.error) || 'unknown') }),
            });
          };
        </script>
        <script src="https://arkoselabs.roblox.com/v2/{{Uri.EscapeDataString(publicKey)}}/api.js" data-callback="setupEnforcement" async defer></script>
        </body>
        </html>
        """;

    /// <summary>
    /// Mirrors roblox.com's captchav2 flow (Challenge.js, Oct 2026): load HUMAN's sensor and wait briefly for its _px3
    /// cookie, POST {challenge_id} to /v2/captcha. 200 carries the redemption token; 403 carries the press-and-hold
    /// puzzle (window._px* settings and a block script); once it's solved, HUMAN calls _pxOnCaptchaSuccess and the POST
    /// is sent again.
    /// </summary>
    static string HumanPage(string apiBase, string challengeId) => $$"""
        <!doctype html>
        <html lang="{{Lang}}">
        <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>Captcha</title>
        <style>{{Style}}</style>
        </head>
        <body>
        <div id="msg"><div><div id="text">{{Esc(L.T("Loading the puzzle…"))}}</div><div id="px-captcha"></div><button id="again" hidden>{{Esc(L.T("Try again"))}}</button></div></div>
        <script>
          const API = {{Js(apiBase + "/v2/captcha")}}, CHALLENGE = {{Js(challengeId)}};
          const SENSOR = 'https://client.px-cloud.net/PXbf8PROpW/main.min.js';
          const post = m => window.chrome.webview.postMessage(m);
          const log = t => post({ log: t });
          const text = document.getElementById('text'), again = document.getElementById('again');
          const sleep = ms => new Promise(r => setTimeout(r, ms));
          let csrf = '', busy = false;
          window.chrome.webview.addEventListener('message', e => { if (e.data && e.data.say) { text.textContent = e.data.say; again.hidden = true; } });
          again.onclick = () => { again.hidden = true; text.textContent = {{Js(L.T("Loading the puzzle…"))}}; attempt(); };

          const loadScript = (id, src) => new Promise(done => {
            document.getElementById(id)?.remove();
            const s = document.createElement('script');
            s.id = id; s.src = src; s.async = true;
            s.onload = () => done(true); s.onerror = () => done(false);
            document.head.appendChild(s);
          });
          const hasPx3 = () => document.cookie.split('; ').some(c => c.startsWith('_px3='));

          async function submit() {
            for (let i = 0; i < 3; i++) {
              const headers = { 'Content-Type': 'application/json', 'Accept': 'application/json' };
              if (csrf) headers['x-csrf-token'] = csrf;
              const r = await fetch(API, { method: 'POST', credentials: 'include', headers, body: JSON.stringify({ challenge_id: CHALLENGE }) });
              let body = null;
              try { body = await r.json(); } catch {}
              const fresh = r.headers.get('x-csrf-token');
              const block = body && (body.appId ? body : body.data && body.data.appId ? body.data : null);
              if (r.status === 403 && fresh && fresh !== csrf && !block) { csrf = fresh; continue; }
              return { status: r.status, body, block };
            }
            return { status: 0, body: null, block: null };
          }

          function show(b) {
            window._pxAppId = b.appId; window._pxJsClientSrc = b.jsClientSrc; window._pxFirstPartyEnabled = b.firstPartyEnabled === true;
            window._pxVid = b.vid; window._pxUuid = b.uuid; window._pxHostUrl = b.hostUrl;
            if (typeof b.blockedUrl === 'string') window._pxBlockedUrl = b.blockedUrl;
            window['_' + b.appId] = { challenge: {
              view: { width: '100%', height: 48, backgroundColor: '{{Hex(Themes.Current.Surface)}}', fillColor: '#22c55e', borderColor: '{{Hex(Themes.Current.Surface)}}', borderWidth: 0,
                innerMargin: 0, textColor: '{{Hex(Themes.Current.Text)}}', borderRadius: 12, textFont: 'Segoe UI, sans-serif', texSize: 14, fontWeight: 600,
                textTransform: 'none', animation: false },
              translation: { default: { btn: {{Js(L.T("Press and hold"))}} } },
              context: { failed: { color: 'transparent', fontSize: '0px' } },
            } };
            window[b.appId + '_asyncInit'] = px => px.Events.on('captcha', ev => log('press-and-hold ' + ev));
            window._pxOnCaptchaSuccess = ok => {
              if (ok) { text.textContent = {{Js(L.T("Checking…"))}}; attempt(); }
              else { text.textContent = {{Js(L.T("That didn't work. Try again."))}}; again.hidden = false; }
            };
            text.textContent = {{Js(L.T("Press and hold the button until it fills up."))}};
            post({ ready: true });
            loadScript('captcha-v2-block-script', b.blockScript).then(ok => { if (!ok) post({ error: 'block script failed to load' }); });
          }

          async function attempt() {
            if (busy) return;
            busy = true;
            try {
              const r = await submit();
              log('v2/captcha HTTP ' + r.status + ', fields ' + (r.body ? Object.keys(r.body).join(', ') : 'none')
                + (r.block ? ', puzzle fields ' + Object.keys(r.block).join(', ') : ''));
              if (r.status === 200 && r.body && r.body.redemption_token) return post({ token: r.body.redemption_token });
              if (r.status === 403 && r.block) return show(r.block);
              post({ error: r.status === 404 ? 'session not found' : 'HTTP ' + r.status });
            } catch (e) {
              post({ error: String(e && e.message || e) });
            } finally {
              busy = false;
            }
          }

          (async () => {
            const loaded = await loadScript('captcha-v2-sensor', SENSOR);
            if (!loaded) log('sensor failed to load');
            post({ ready: true });
            for (let t = 0; t < 40 && !hasPx3(); t++) await sleep(50);
            attempt();
          })();
        </script>
        </body>
        </html>
        """;

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
        base.OnFormClosed(e);
    }

    /// <summary>Removes the per-window profiles older versions made in %TEMP%.</summary>
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
