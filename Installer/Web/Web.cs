// The web installer: Setup.cs built with this file and /define:WEB (Installer\Web\make.bat). It carries no version of
// Bloxkeep: when installing, it downloads the newest release's Bloxkeep-Setup-Offline.exe from GitHub, checks it
// against that release's SHA256SUMS.txt, and has it hand over the Velopack installer it carries (--extract-payload).
// So the Bloxkeep-Setup.exe people download stays the same file from one release to the next, and the reputation
// Windows SmartScreen builds up for an unsigned file (by its hash) isn't lost at every release.
using System;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace BloxkeepSetup
{
    static class Web
    {
        const string Repo = "https://github.com/minhhoang0122/bloxkeep";
        public const string OfflineName = "Bloxkeep-Setup-Offline.exe";

        sealed class Release
        {
            public string Tag, Version;
            public long PackageBytes;
        }

        static Release _latest;
        static readonly object FetchGate = new object(), CancelGate = new object();
        static HttpWebRequest _current;
        static bool _downloading, _cancelled;

        static Web()
        {
            // GitHub only accepts TLS 1.2 and later; a program built like this one starts with older ones only.
            ServicePointManager.SecurityProtocol |= (SecurityProtocolType)3072;
        }

        /// <summary>The newest release's version once <see cref="Latest"/> has found it, else null.</summary>
        public static string Version
        {
            get
            {
                Release r = _latest;
                return r == null ? null : r.Version;
            }
        }

        /// <summary>
        /// The space the newest release takes, from its full package's size (the program files are about 2.2 times that,
        /// plus the package itself and 64 MB, as info.ps1 measures it); 0 while it isn't known.
        /// </summary>
        public static long NeededBytes
        {
            get
            {
                Release r = _latest;
                return r == null || r.PackageBytes <= 0 ? 0 : (long)(r.PackageBytes * 3.25) + 64L * 1024 * 1024;
            }
        }

        /// <summary>Finds the newest release (once); an IOException that says why when GitHub can't be reached.</summary>
        public static void Latest()
        {
            lock (FetchGate)
            {
                if (_latest == null)
                    _latest = Fetch();
            }
        }

        static Release Fetch()
        {
            // github.com/<repo>/releases/latest answers with a redirect to the newest release's page, named by its tag.
            HttpWebRequest ask = Request(Repo + "/releases/latest");
            ask.AllowAutoRedirect = false;
            string location;
            using (HttpWebResponse answer = Get(ask))
                location = answer.Headers["Location"];
            Match tag = Regex.Match(location ?? "", "/releases/tag/([^/?#]+)$");
            if (!tag.Success)
                throw new IOException(T.S("GitHub didn't say which version of Bloxkeep is the newest. Try again later.",
                    "GitHub không cho biết phiên bản Bloxkeep mới nhất. Hãy thử lại sau."));
            var r = new Release { Tag = Uri.UnescapeDataString(tag.Groups[1].Value) };
            r.Version = r.Tag.TrimStart('v');
            try
            {
                Match full = Regex.Match(Text(Asset(r.Tag, "releases.win.json")),
                    "\"Version\"\\s*:\\s*\"" + Regex.Escape(r.Version) + "\"\\s*,\\s*\"Type\"\\s*:\\s*\"Full\"[^}]*?\"Size\"\\s*:\\s*(\\d+)");
                if (full.Success)
                    r.PackageBytes = long.Parse(full.Groups[1].Value);
            }
            catch (IOException) { } // only the space shown is a guess then
            return r;
        }

        /// <summary>
        /// Downloads the newest offline installer to <paramref name="path"/> and checks it. <paramref name="progress"/>
        /// gets the bytes so far and the total (0 when GitHub doesn't say). An OperationCanceledException when
        /// <see cref="Cancel"/> stopped it.
        /// </summary>
        public static void DownloadOffline(string path, Action<long, long> progress)
        {
            lock (CancelGate)
            {
                _downloading = true;
                _cancelled = false;
            }
            try
            {
                Latest();
                Release r = _latest;
                string expected = null;
                try
                {
                    Match line = Regex.Match(Text(Asset(r.Tag, "SHA256SUMS.txt")), "^([0-9a-fA-F]{64}) [ *]?" + Regex.Escape(OfflineName) + "\\s*$", RegexOptions.Multiline);
                    if (line.Success)
                        expected = line.Groups[1].Value;
                }
                catch (FileNotFoundException) { } // a release without the list: the length alone is checked

                using (HttpWebResponse answer = Get(Request(Asset(r.Tag, OfflineName))))
                using (Stream body = answer.GetResponseStream())
                using (FileStream file = File.Create(path))
                using (SHA256 sha = SHA256.Create())
                {
                    long total = Math.Max(0, answer.ContentLength), have = 0, shown = -1;
                    var buffer = new byte[81920];
                    int n;
                    while ((n = Read(body, buffer)) > 0)
                    {
                        file.Write(buffer, 0, n);
                        sha.TransformBlock(buffer, 0, n, null, 0);
                        have += n;
                        if (progress != null && (have - shown >= 256 * 1024 || have == total))
                        {
                            shown = have;
                            progress(have, total);
                        }
                    }
                    sha.TransformFinalBlock(buffer, 0, 0);
                    if ((total > 0 && have != total) || (expected != null && !string.Equals(Hex(sha.Hash), expected, StringComparison.OrdinalIgnoreCase)))
                        throw new IOException(T.S("The download arrived damaged. Try again.", "File tải về bị lỗi. Hãy thử lại."));
                }
            }
            catch (Exception ex)
            {
                // Cancel aborts the request in flight, which surfaces as whatever error the read was in.
                if (Cancelled)
                    throw new OperationCanceledException();
                if (ex is WebException)
                    throw Offline((WebException)ex);
                throw;
            }
            finally
            {
                lock (CancelGate)
                    _downloading = false;
            }
            if (Cancelled)
                throw new OperationCanceledException();
        }

        static bool Cancelled { get { lock (CancelGate) return _cancelled; } }

        /// <summary>Stops a download in progress (closing the window during it); false when none is, as installing can't be stopped.</summary>
        public static bool Cancel()
        {
            lock (CancelGate)
            {
                if (!_downloading)
                    return false;
                _cancelled = true;
                if (_current != null)
                    _current.Abort();
                return true;
            }
        }

        /// <summary>Has the downloaded offline installer write the Velopack installer it carries to <paramref name="exe"/>.</summary>
        public static void Unpack(string offline, string exe)
        {
            var info = new ProcessStartInfo(offline, "--extract-payload \"" + Path.GetFullPath(exe) + "\"");
            info.UseShellExecute = false;
            info.CreateNoWindow = true;
            info.WorkingDirectory = Path.GetDirectoryName(offline);
            using (Process p = Process.Start(info))
            {
                if (!p.WaitForExit(120000))
                {
                    try { p.Kill(); } catch (InvalidOperationException) { } catch (System.ComponentModel.Win32Exception) { }
                    throw new IOException(T.S("The downloaded installer didn't respond. Try again.", "Bộ cài vừa tải không phản hồi. Hãy thử lại."));
                }
                if (p.ExitCode != 0 || !File.Exists(exe))
                    throw new IOException(string.Format(T.S("The downloaded installer stopped with code {0}. Try again.", "Bộ cài vừa tải dừng lại với mã {0}. Hãy thử lại."), p.ExitCode));
            }
        }

        static string Asset(string tag, string name)
        {
            return Repo + "/releases/download/" + Uri.EscapeDataString(tag) + "/" + name;
        }

        static HttpWebRequest Request(string url)
        {
            var r = (HttpWebRequest)WebRequest.Create(url);
            r.UserAgent = "Bloxkeep-Setup";
            r.Timeout = 30000;
            r.ReadWriteTimeout = 60000;
            lock (CancelGate)
                _current = r;
            return r;
        }

        /// <summary>The answer; FileNotFoundException for a 404, an IOException that says why for any other failure.</summary>
        static HttpWebResponse Get(HttpWebRequest request)
        {
            try
            {
                return (HttpWebResponse)request.GetResponse();
            }
            catch (WebException ex)
            {
                var answer = ex.Response as HttpWebResponse;
                if (answer != null && answer.StatusCode == HttpStatusCode.NotFound)
                {
                    answer.Dispose();
                    throw new FileNotFoundException(T.S("GitHub doesn't have this file in Bloxkeep's newest release yet. Try again in a few minutes.",
                        "Bản phát hành mới nhất của Bloxkeep trên GitHub chưa có file này. Hãy thử lại sau vài phút."), request.RequestUri.ToString());
                }
                if (Cancelled)
                    throw new OperationCanceledException();
                throw Offline(ex);
            }
        }

        static int Read(Stream body, byte[] buffer)
        {
            try { return body.Read(buffer, 0, buffer.Length); }
            catch (WebException ex) { throw Offline(ex); }
        }

        static string Text(string url)
        {
            using (HttpWebResponse answer = Get(Request(url)))
            using (var reader = new StreamReader(answer.GetResponseStream(), Encoding.UTF8))
                return reader.ReadToEnd();
        }

        static IOException Offline(WebException ex)
        {
            return new IOException(T.S("Couldn't download Bloxkeep from GitHub. Check the internet connection and try again.",
                "Không tải được Bloxkeep từ GitHub. Hãy kiểm tra kết nối mạng rồi thử lại.") + " (" + ex.Message + ")", ex);
        }

        static string Hex(byte[] bytes)
        {
            var s = new StringBuilder(bytes.Length * 2);
            foreach (byte b in bytes)
                s.Append(b.ToString("x2"));
            return s.ToString();
        }
    }
}
