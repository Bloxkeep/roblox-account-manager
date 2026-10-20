namespace RobloxManager;

/// <summary>
/// Settings › Appearance › Custom (<see cref="Prefs.Custom"/>): the person's own background colour and accent, and a
/// picture of theirs behind the page if they like. The page works out the rest of the colours from those two (app.js
/// customVars) and shows the picture behind glass, like the scenery themes. The picture is copied into the data folder
/// and the page loads it from there (ShellForm.ServeAsset, "user/…"): the original can be moved or deleted.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Bigger than this is refused: the page holds the whole picture while it shows.</summary>
    const long MaxThemePicture = 20 * 1024 * 1024;

    static readonly string[] PictureTypes = [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"];

    /// <summary>The custom theme for the page; its picture as the page loads it.</summary>
    static object CustomThemeDto() => new
    {
        background = Prefs.Custom.Background,
        accent = Prefs.Custom.Accent,
        fade = Prefs.Custom.Fade,
        picture = Prefs.Custom.Picture is { } picture ? "user/" + picture : null,
    };

    /// <summary>New colours (or how much of the colour lies over the picture); the theme becomes the custom one.</summary>
    object? SetCustomTheme(string background, string accent, int? fade)
    {
        Prefs.Custom = Prefs.Custom with
        {
            Background = Themes.Hex(background) ?? Prefs.Custom.Background,
            Accent = Themes.Hex(accent) ?? Prefs.Custom.Accent,
            Fade = fade is { } f ? Math.Clamp(f, 0, 90) : Prefs.Custom.Fade,
        };
        return UseCustomTheme();
    }

    /// <summary>"Choose a picture…": one from the PC, copied into the data folder, behind the page from now on.</summary>
    object? PickThemePicture()
    {
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Choose a background picture"),
            Filter = L.T("Pictures") + " (*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp)|*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp",
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyPictures),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
            return null;
        string type = Path.GetExtension(dialog.FileName).ToLowerInvariant();
        if (!PictureTypes.Contains(type))
            throw new UserError(L.T("Pick a picture: a PNG, JPG, WebP, GIF or BMP file."));
        if (new FileInfo(dialog.FileName).Length > MaxThemePicture)
            throw new UserError(L.T("That picture is over 20 MB. Pick a smaller one."));
        Directory.CreateDirectory(Vault.DataDir);
        // A new name each time, so the page loads the new picture rather than the one it already shows.
        string name = $"theme-background-{DateTime.UtcNow.Ticks}{type}";
        File.Copy(dialog.FileName, Path.Combine(Vault.DataDir, name));
        DeleteThemePictures(except: name);
        Prefs.Custom = Prefs.Custom with { Picture = name };
        return UseCustomTheme();
    }

    /// <summary>"Remove the picture": the custom theme goes back to its plain colour.</summary>
    object? ClearThemePicture()
    {
        DeleteThemePictures(except: null);
        Prefs.Custom = Prefs.Custom with { Picture = null };
        return UseCustomTheme();
    }

    object? UseCustomTheme()
    {
        Prefs.Theme = Themes.Custom;
        Prefs.Save();
        AppearanceChanged?.Invoke();
        PushState();
        return null;
    }

    static void DeleteThemePictures(string? except)
    {
        try
        {
            foreach (string file in Directory.EnumerateFiles(Vault.DataDir, "theme-background-*"))
            {
                if (Path.GetFileName(file) != except && Themes.IsPictureName(Path.GetFileName(file)))
                    File.Delete(file);
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex); // an old picture left behind: replaced or removed next time
        }
    }
}
