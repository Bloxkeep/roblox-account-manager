using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Cryptography.X509Certificates;
using Microsoft.Web.WebView2.Core;

namespace RobloxManager;

/// <summary>
/// Installs Microsoft Edge WebView2 Runtime when it is missing, using Microsoft's official bootstrapper.
/// The download only runs after Windows confirms a valid signature from Microsoft Corporation.
/// </summary>
static class WebView2Setup
{
    const string BootstrapperUrl = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
    const string SafeRenderingArgs = "--disable-gpu --disable-gpu-compositing";

    /// <summary>
    /// Switches for every browser the app opens (the window, sign-in, captcha): drawn on the CPU when the graphics
    /// driver proved broken (<see cref="Prefs.SafeRendering"/>), otherwise null for the usual GPU drawing.
    /// </summary>
    public static CoreWebView2EnvironmentOptions? EnvironmentOptions() =>
        Prefs.SafeRendering ? new CoreWebView2EnvironmentOptions(SafeRenderingArgs) : null;

    public static async Task<bool> TryInstallAsync(IWin32Window owner)
    {
        if (MessageBox.Show(owner,
                L.T("Bloxkeep needs Microsoft Edge WebView2, which isn't installed. Install it now? (about 2 MB)"),
                Ui.Title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes)
            return false;

        string dir = Path.Combine(Path.GetTempPath(), "Bloxkeep-webview2-" + Guid.NewGuid().ToString("N"));
        string path = Path.Combine(dir, "MicrosoftEdgeWebview2Setup.exe");
        try
        {
            Directory.CreateDirectory(dir);
            using (var http = new HttpClient { Timeout = TimeSpan.FromMinutes(2) })
                await File.WriteAllBytesAsync(path, await http.GetByteArrayAsync(BootstrapperUrl));
            if (!IsSignedByMicrosoft(path))
            {
                Ui.Error(owner, L.T("The downloaded WebView2 installer is not signed by Microsoft, so it was not run."));
                return false;
            }
            using var setup = Process.Start(new ProcessStartInfo(path, "/silent /install") { UseShellExecute = true });
            if (setup is null)
                return false;
            await setup.WaitForExitAsync();
            return setup.ExitCode == 0;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or UnauthorizedAccessException or Win32Exception)
        {
            Ui.Error(owner, L.F("Couldn't install WebView2: {0}", ex.Message));
            return false;
        }
        finally
        {
            try { Directory.Delete(dir, recursive: true); }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
        }
    }

    public static bool IsSignedByMicrosoft(string path)
    {
        if (!HasValidSignature(path))
            return false;
        try
        {
#pragma warning disable SYSLIB0057 // reads the signer of an Authenticode-signed file; there is no loader-based replacement
            using var signer = new X509Certificate2(X509Certificate.CreateFromSignedFile(path));
#pragma warning restore SYSLIB0057
            return signer.Subject.Contains("O=Microsoft Corporation", StringComparison.Ordinal);
        }
        catch (System.Security.Cryptography.CryptographicException)
        {
            return false;
        }
    }

    [StructLayout(LayoutKind.Sequential)]
    struct WINTRUST_FILE_INFO
    {
        public uint cbStruct;
        [MarshalAs(UnmanagedType.LPWStr)] public string pcwszFilePath;
        public IntPtr hFile;
        public IntPtr pgKnownSubject;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct WINTRUST_DATA
    {
        public uint cbStruct;
        public IntPtr pPolicyCallbackData;
        public IntPtr pSIPClientData;
        public uint dwUIChoice;
        public uint fdwRevocationChecks;
        public uint dwUnionChoice;
        public IntPtr pFile;
        public uint dwStateAction;
        public IntPtr hWVTStateData;
        public IntPtr pwszURLReference;
        public uint dwProvFlags;
        public uint dwUIContext;
        public IntPtr pSignatureSettings;
    }

    [DllImport("wintrust.dll", ExactSpelling = true, CharSet = CharSet.Unicode)]
    static extern int WinVerifyTrust(IntPtr hwnd, [MarshalAs(UnmanagedType.LPStruct)] Guid action, ref WINTRUST_DATA data);

    static readonly Guid GenericVerifyV2 = new("00AAC56B-CD44-11d0-8CC2-00C04FC295EE");

    /// <summary>Authenticode check of the file's embedded signature (chain and integrity), without any UI.</summary>
    static bool HasValidSignature(string path)
    {
        const uint WTD_UI_NONE = 2, WTD_REVOKE_NONE = 0, WTD_CHOICE_FILE = 1, WTD_STATEACTION_IGNORE = 0;
        IntPtr file = Marshal.AllocHGlobal(Marshal.SizeOf<WINTRUST_FILE_INFO>());
        try
        {
            Marshal.StructureToPtr(new WINTRUST_FILE_INFO { cbStruct = (uint)Marshal.SizeOf<WINTRUST_FILE_INFO>(), pcwszFilePath = path }, file, false);
            var data = new WINTRUST_DATA
            {
                cbStruct = (uint)Marshal.SizeOf<WINTRUST_DATA>(),
                dwUIChoice = WTD_UI_NONE,
                fdwRevocationChecks = WTD_REVOKE_NONE,
                dwUnionChoice = WTD_CHOICE_FILE,
                pFile = file,
                dwStateAction = WTD_STATEACTION_IGNORE,
            };
            return WinVerifyTrust(IntPtr.Zero, GenericVerifyV2, ref data) == 0;
        }
        finally
        {
            Marshal.DestroyStructure<WINTRUST_FILE_INFO>(file);
            Marshal.FreeHGlobal(file);
        }
    }
}
