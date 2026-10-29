using System.IO.Compression;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Settings › Appearance › Custom (<see cref="Prefs.Custom"/>): the person's own background colour and accent, and a
/// picture of theirs behind the page if they like. The page works out the rest of the colours from those two (app.js
/// customVars) and shows the picture behind glass, like the scenery themes. The picture is copied into the data folder
/// and the page loads it from there (ShellForm.ServeAsset, "user/…"): the original can be moved or deleted. A video
/// (MP4, WebM) can take the picture's place; either can be blurred. Custom themes are kept by name (Your themes,
/// <see cref="Prefs.SavedThemes"/>) and shared as a .bloxkeeptheme file: a ZIP of theme.json and its picture.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Bigger than this is refused: the page holds the whole picture (or video) while it shows.</summary>
    const long MaxThemePicture = 20 * 1024 * 1024, MaxThemeVideo = 50 * 1024 * 1024;

    static readonly string[] PictureTypes = [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"], VideoTypes = [".mp4", ".webm"];

    /// <summary>A theme file's own name ending, and the most its theme.json may hold.</summary>
    const string ThemeFileType = ".bloxkeeptheme";
    const int MaxThemeJson = 16 * 1024;

    /// <summary>The custom theme for the page; its picture as the page loads it.</summary>
    static object CustomThemeDto() => LookDto(Prefs.Custom);

    static object LookDto(Prefs.CustomTheme look) => new
    {
        background = look.Background,
        accent = look.Accent,
        fade = look.Fade,
        blur = look.Blur,
        picture = look.Picture is { } picture ? "user/" + picture : null,
        video = Themes.IsVideo(look.Picture),
    };

    /// <summary>Your themes, for Settings › Appearance: each with its look, for its swatch.</summary>
    static object[] SavedThemesDto() => Prefs.SavedThemes.Select(t => (object)new { id = t.Id, name = t.Name, look = LookDto(t.Look) }).ToArray();

    /// <summary>
    /// New colours (or how much of the colour lies over the picture, how much the picture is blurred); the theme
    /// becomes the custom one.
    /// </summary>
    object? SetCustomTheme(string background, string accent, int? fade, int? blur = null)
    {
        Prefs.Custom = Prefs.Custom with
        {
            Background = Themes.Hex(background) ?? Prefs.Custom.Background,
            Accent = Themes.Hex(accent) ?? Prefs.Custom.Accent,
            Fade = fade is { } f ? Math.Clamp(f, 0, 90) : Prefs.Custom.Fade,
            Blur = blur is { } b ? Math.Clamp(b, 0, Themes.MaxBlur) : Prefs.Custom.Blur,
        };
        return UseCustomTheme();
    }

    /// <summary>"Choose a picture or video…": one from the PC, copied into the data folder, behind the page from now on.</summary>
    object? PickThemePicture()
    {
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Choose a background picture or video"),
            Filter = L.T("Pictures and videos") + " (*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp;*.mp4;*.webm)|*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp;*.mp4;*.webm",
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyPictures),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
            return null;
        string type = Path.GetExtension(dialog.FileName).ToLowerInvariant();
        CheckThemeMedia(type, new FileInfo(dialog.FileName).Length);
        string name = NewMediaName(type);
        File.Copy(dialog.FileName, Path.Combine(Vault.PicturesDir, name));
        Prefs.Custom = Prefs.Custom with { Picture = name };
        DeleteThemePictures();
        return UseCustomTheme();
    }

    /// <summary>A picture or video of a kind and size the page can show, or a message saying why not.</summary>
    static void CheckThemeMedia(string type, long length)
    {
        bool video = VideoTypes.Contains(type);
        if (!video && !PictureTypes.Contains(type))
            throw new UserError(L.T("Pick a picture (PNG, JPG, WebP, GIF, BMP) or a video (MP4, WebM)."));
        if (length > (video ? MaxThemeVideo : MaxThemePicture))
            throw new UserError(video ? L.T("That video is over 50 MB. Pick a smaller one.") : L.T("That picture is over 20 MB. Pick a smaller one."));
    }

    /// <summary>A new name each time, so the page loads the new picture rather than the one it already shows.</summary>
    static string NewMediaName(string type)
    {
        Directory.CreateDirectory(Vault.PicturesDir);
        long ticks = DateTime.UtcNow.Ticks;
        while (File.Exists(Path.Combine(Vault.PicturesDir, $"theme-background-{ticks}{type}")))
            ticks++;
        return $"theme-background-{ticks}{type}";
    }

    /// <summary>"Remove the picture": the custom theme goes back to its plain colour.</summary>
    object? ClearThemePicture()
    {
        Prefs.Custom = Prefs.Custom with { Picture = null };
        DeleteThemePictures();
        return UseCustomTheme();
    }

    /// <summary>
    /// "Save": the custom theme as it is now kept under a name. A theme of that name already kept is replaced (the
    /// same name is the same theme, changed); a new one goes first. At most <see cref="Themes.MaxSaved"/>.
    /// </summary>
    object? SaveTheme(string name)
    {
        string named = Themes.ThemeName(name) ?? throw new UserError(L.T("Give the theme a name."));
        var list = Prefs.SavedThemes;
        int same = list.FindIndex(t => string.Equals(t.Name, named, StringComparison.CurrentCultureIgnoreCase));
        if (same >= 0)
            list[same] = list[same] with { Name = named, Look = Prefs.Custom };
        else if (list.Count >= Themes.MaxSaved)
            throw new UserError(L.F("You can keep up to {0} themes. Delete one first.", Themes.MaxSaved));
        else
            list.Insert(0, new Prefs.SavedTheme(Guid.NewGuid().ToString("N"), named, Prefs.Custom));
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>A kept theme picked: it becomes the custom theme in use.</summary>
    object? UseSavedTheme(string id)
    {
        var theme = Prefs.SavedThemes.FirstOrDefault(t => t.Id == id) ?? throw new UserError(L.T("That theme isn't there any more."));
        Prefs.Custom = theme.Look;
        DeleteThemePictures();
        return UseCustomTheme();
    }

    /// <summary>A kept theme deleted, with its picture unless the theme in use (or another kept one) shows it too.</summary>
    object? DeleteSavedTheme(string id)
    {
        Prefs.SavedThemes.RemoveAll(t => t.Id == id);
        DeleteThemePictures();
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>"Share": a kept theme written to a .bloxkeeptheme file the person picks where to put.</summary>
    object? ExportTheme(string id)
    {
        var theme = Prefs.SavedThemes.FirstOrDefault(t => t.Id == id) ?? throw new UserError(L.T("That theme isn't there any more."));
        using var dialog = new SaveFileDialog
        {
            Title = L.T("Save the theme as a file"),
            Filter = L.T("Bloxkeep theme") + $" (*{ThemeFileType})|*{ThemeFileType}",
            FileName = string.Concat(theme.Name.Select(ch => Path.GetInvalidFileNameChars().Contains(ch) ? '-' : ch)) + ThemeFileType,
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
            return null;
        WriteThemeFile(theme, dialog.FileName);
        return new { saved = Path.GetFileName(dialog.FileName) };
    }

    /// <summary>"Add from a file…": a .bloxkeeptheme file from the PC, kept and put in use.</summary>
    object? ImportTheme()
    {
        if (Prefs.SavedThemes.Count >= Themes.MaxSaved)
            throw new UserError(L.F("You can keep up to {0} themes. Delete one first.", Themes.MaxSaved));
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Add a theme from a file"),
            Filter = L.T("Bloxkeep theme") + $" (*{ThemeFileType})|*{ThemeFileType}",
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
            return null;
        return AddThemeFile(dialog.FileName);
    }

    /// <summary>A theme file read, kept first among Your themes and put in use.</summary>
    object? AddThemeFile(string path)
    {
        var theme = ReadThemeFile(path);
        Prefs.SavedThemes.Insert(0, theme);
        Prefs.Custom = theme.Look;
        DeleteThemePictures();
        return UseCustomTheme();
    }

    /// <summary>A theme's file: theme.json (its name and look) and its picture or video, if it has one.</summary>
    static void WriteThemeFile(Prefs.SavedTheme theme, string path)
    {
        string? media = theme.Look.Picture is { } picture && File.Exists(Path.Combine(Vault.PicturesDir, picture)) ? picture : null;
        string? inFile = media is null ? null : "background" + Path.GetExtension(media).ToLowerInvariant();
        string temp = path + ".tmp";
        using (var zip = ZipFile.Open(temp, ZipArchiveMode.Create))
        {
            var json = new
            {
                bloxkeepTheme = 1, name = theme.Name, background = theme.Look.Background, accent = theme.Look.Accent,
                fade = theme.Look.Fade, blur = theme.Look.Blur, picture = inFile,
            };
            using (var writer = new StreamWriter(zip.CreateEntry("theme.json").Open()))
                writer.Write(JsonSerializer.Serialize(json));
            if (media is not null)
                zip.CreateEntryFromFile(Path.Combine(Vault.PicturesDir, media), inFile!, CompressionLevel.NoCompression);
        }
        File.Move(temp, path, overwrite: true);
    }

    /// <summary>
    /// A theme file read, someone else's maybe, so nothing in it is trusted: its colours checked, its numbers kept in
    /// range, its name cleaned; its picture only of a kind and size the page shows, copied out under a name of
    /// Bloxkeep's own (never a path from the file), read no further than that size whatever the file claims.
    /// </summary>
    static Prefs.SavedTheme ReadThemeFile(string path)
    {
        UserError NotTheme() => new(L.T("That file isn't a Bloxkeep theme."));
        if (new FileInfo(path).Length > MaxThemeVideo + MaxThemeJson + 64 * 1024)
            throw NotTheme();
        ZipArchive zip;
        try
        {
            zip = ZipFile.OpenRead(path);
        }
        catch (InvalidDataException)
        {
            throw NotTheme();
        }
        using (zip)
        {
            if (zip.Entries.Count > 4 || zip.GetEntry("theme.json") is not { } entry || entry.Length > MaxThemeJson)
                throw NotTheme();
            JsonElement root;
            try
            {
                using var reader = new StreamReader(entry.Open());
                var buffer = new char[MaxThemeJson + 1];
                int read = reader.ReadBlock(buffer, 0, buffer.Length);
                if (read > MaxThemeJson)
                    throw NotTheme();
                using var document = JsonDocument.Parse(new string(buffer, 0, read));
                root = document.RootElement.Clone();
            }
            catch (Exception ex) when (ex is JsonException or InvalidDataException)
            {
                throw NotTheme();
            }
            if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("bloxkeepTheme", out _))
                throw NotTheme();
            string? Text(string name) => root.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
            int? Number(string name) => root.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt32(out int n) ? n : null;
            string? media = null;
            // Only a file at the top of the ZIP, by a name with no folder in it.
            if (Text("picture") is { } picture && picture.IndexOfAny(['/', '\\', ':']) < 0 && zip.GetEntry(picture) is { } file)
            {
                string type = Path.GetExtension(picture).ToLowerInvariant();
                CheckThemeMedia(type, file.Length);
                long limit = VideoTypes.Contains(type) ? MaxThemeVideo : MaxThemePicture;
                media = NewMediaName(type);
                string to = Path.Combine(Vault.PicturesDir, media);
                try
                {
                    using var from = file.Open();
                    using var output = File.Create(to);
                    var buffer = new byte[81920];
                    long total = 0;
                    for (int n; (n = from.Read(buffer, 0, buffer.Length)) > 0;)
                    {
                        total += n;
                        if (total > limit)
                            throw NotTheme();
                        output.Write(buffer, 0, n);
                    }
                }
                catch (Exception ex) when (ex is UserError or InvalidDataException or IOException)
                {
                    File.Delete(to);
                    throw ex as UserError ?? NotTheme();
                }
            }
            var look = new Prefs.CustomTheme(Themes.Hex(Text("background")) ?? Prefs.Custom.Background, Themes.Hex(Text("accent")) ?? Prefs.Custom.Accent,
                Math.Clamp(Number("fade") ?? 45, 0, 90), media, Math.Clamp(Number("blur") ?? 0, 0, Themes.MaxBlur));
            string name = Themes.ThemeName(Text("name")) ?? Themes.ThemeName(Path.GetFileNameWithoutExtension(path)) ?? L.T("Theme");
            return new Prefs.SavedTheme(Guid.NewGuid().ToString("N"), name, look);
        }
    }

    object? UseCustomTheme()
    {
        Prefs.Theme = Themes.Custom;
        Prefs.Save();
        AppearanceChanged?.Invoke();
        PushState();
        return null;
    }

    /// <summary>The theme pictures (and videos) neither the theme in use nor a kept one shows any more, deleted.</summary>
    static void DeleteThemePictures()
    {
        var used = Prefs.SavedThemes.Select(t => t.Look.Picture).Append(Prefs.Custom.Picture).OfType<string>().ToHashSet(StringComparer.OrdinalIgnoreCase);
        try
        {
            // The top of the data folder too: one Vault.Tidy couldn't move yet.
            foreach (string file in new[] { Vault.PicturesDir, Vault.DataDir }.Where(Directory.Exists)
                .SelectMany(dir => Directory.EnumerateFiles(dir, "theme-background-*")).ToList())
            {
                if (!used.Contains(Path.GetFileName(file)) && Themes.IsThemeMediaName(Path.GetFileName(file)))
                    File.Delete(file);
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex); // an old picture left behind: replaced or removed next time
        }
    }
}
