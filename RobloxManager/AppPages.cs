using System.Text;
using Microsoft.Web.WebView2.Core;

namespace RobloxManager;

/// <summary>
/// What every window showing one of the app's pages shares: the pages' own address, the one browser profile, the files
/// served from the app (and the custom theme's picture from the data folder), and the browser locked down to them.
/// The main window (ShellForm), the small page windows (WebToolWindow, TrayWindow) and the sign-in browser's bar (ChromeView).
/// </summary>
internal static class AppPages
{
    /// <summary>The pages' address; nothing else is ever shown in the app's browsers.</summary>
    internal const string Origin = "https://bloxkeep.local/";

    /// <summary>The main window's browser profile, which every page window shares (one browser and GPU process).</summary>
    internal static Task<CoreWebView2Environment> CreateEnvironmentAsync()
    {
        // Separate profile folders, because a running browser can't be reopened with different switches.
        return CoreWebView2Environment.CreateAsync(null, Vault.CacheFolder(Prefs.SafeRendering ? "webview-safe" : "webview"),
            WebView2Setup.MainWindowOptions());
    }

    /// <summary>No browser parts: dev tools, status bar, zoom, its own keys, saved passwords and forms, swipes. Only our own
    /// embedded pages are ever shown, so SmartScreen lookups are wasted work.</summary>
    internal static void Settings(CoreWebView2Settings settings)
    {
        settings.AreDevToolsEnabled = false;
        settings.IsStatusBarEnabled = false;
        settings.IsZoomControlEnabled = false;
        settings.AreBrowserAcceleratorKeysEnabled = false;
        settings.IsPasswordAutosaveEnabled = false;
        settings.IsGeneralAutofillEnabled = false;
        settings.IsSwipeNavigationEnabled = false;
        settings.IsReputationCheckingRequired = false;
    }

    /// <summary>A small page's browser: <see cref="Settings"/> and no right-click menu, the app's files served, only its own
    /// pages, never a new window.</summary>
    internal static void Configure(CoreWebView2 core, CoreWebView2Environment env)
    {
        Settings(core.Settings);
        core.Settings.AreDefaultContextMenusEnabled = false;
        core.AddWebResourceRequestedFilter(Origin + "*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += (_, e) => ServeAsset(env, e);
        core.NavigationStarting += (_, e) => e.Cancel = !e.Uri.StartsWith(Origin, StringComparison.OrdinalIgnoreCase);
        core.NewWindowRequested += (_, e) => e.Handled = true;
    }

    internal static void ServeAsset(CoreWebView2Environment env, CoreWebView2WebResourceRequestedEventArgs e)
    {
        string path = new Uri(e.Request.Uri).AbsolutePath.TrimStart('/');
        // The custom theme's picture: only a file of that name, read whole so no handle stays open (a new picture deletes the old one).
        string? picture = path.StartsWith("user/", StringComparison.Ordinal) && Themes.IsPictureName(path[5..]) ? Path.Combine(Vault.PicturesDir, path[5..]) : null;
        // One Vault.Tidy couldn't move yet is still at the top.
        if (picture is not null && !File.Exists(picture))
            picture = Path.Combine(Vault.DataDir, path[5..]);
        Stream? stream;
        try
        {
            stream = path.StartsWith("user/", StringComparison.Ordinal)
                ? picture is not null && File.Exists(picture) ? new MemoryStream(File.ReadAllBytes(picture)) : null
                : typeof(AppPages).Assembly.GetManifestResourceStream("wwwroot/" + path);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            stream = null; // the picture went away meanwhile: the theme's colour shows instead
        }
        if (stream is null)
        {
            e.Response = env.CreateWebResourceResponse(null, 404, "Not Found", "");
            return;
        }
        if (path == "index.html")
            stream = WithTheme(stream);
        string type = Path.GetExtension(path) switch
        {
            ".html" => "text/html; charset=utf-8",
            ".css" => "text/css; charset=utf-8",
            ".js" => "text/javascript; charset=utf-8",
            ".svg" => "image/svg+xml",
            ".png" => "image/png",
            ".jpg" or ".jpeg" => "image/jpeg",
            ".webp" => "image/webp",
            ".gif" => "image/gif",
            ".bmp" => "image/bmp",
            ".mp4" => "video/mp4",
            ".webm" => "video/webm",
            _ => "application/octet-stream",
        };
        e.Response = env.CreateWebResourceResponse(stream, 200, "OK", $"Content-Type: {type}\r\nCache-Control: no-store");
    }

    /// <summary>The page starts in the right theme, before its first state arrives, so it never flashes dark.</summary>
    static Stream WithTheme(Stream page)
    {
        using var reader = new StreamReader(page);
        var theme = Themes.Current;
        // A light custom theme takes Light's words and lines (app.css): from the first frame, not once its colours come.
        string tone = theme is { Id: Themes.Custom, Dark: false } ? " data-tone=\"light\"" : "";
        string html = reader.ReadToEnd().Replace("<html ", $"<html data-theme=\"{theme.Id}\"{tone} ");
        return new MemoryStream(Encoding.UTF8.GetBytes(html));
    }
}
