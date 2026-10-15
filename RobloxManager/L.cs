namespace RobloxManager;

/// <summary>
/// Language of messages produced by the app (the page has its own strings). English text doubles as the
/// lookup key, so untranslated text simply stays English. The choice lives in <see cref="Prefs"/>.
/// </summary>
public static class L
{
    public static string Language => Prefs.Language;

    public static void Set(string language)
    {
        Prefs.Language = language == "vi" ? "vi" : "en";
        Prefs.Save();
    }

    public static string T(string english) => T(english, Language);

    /// <summary>In a given language rather than the app's (Discord messages can have their own).</summary>
    public static string T(string english, string language) =>
        language == "vi" && Vi.TryGetValue(english, out var vi) ? vi : english;

    public static string F(string english, params object?[] args)
    {
        _lastFormat = (english, args);
        return string.Format(T(english), args);
    }

    /// <summary>
    /// <see cref="F"/> with the English singular or plural chosen by <paramref name="count"/> ("1 account", "2 accounts").
    /// Vietnamese has one form, so both keys are translated alike.
    /// </summary>
    public static string P(long count, string one, string many, params object?[] args) => F(count == 1 ? one : many, args);

    /// <summary>The last <see cref="F"/> on this thread, so <see cref="Made"/> can say the same another way.</summary>
    [ThreadStatic]
    static (string English, object?[] Args)? _lastFormat;

    /// <summary>
    /// The English sentence and values the last <see cref="F"/> on this thread made <paramref name="text"/> from, to
    /// make it again another way (a Discord message: its language, account names shown or hidden by its own switch);
    /// null for any other text.
    /// </summary>
    public static (string English, object?[] Args)? Made(string text) =>
        _lastFormat is { } last && string.Format(T(last.English), last.Args) == text ? last : null;

    /// <summary>
    /// <paramref name="text"/> in English and in Vietnamese, when the last <see cref="F"/> just made it or it is one of
    /// the sentences here; otherwise as it is, twice. The activity log keeps notifications this way, to show them in
    /// whichever language is chosen later.
    /// </summary>
    public static (string English, string Vietnamese) Both(string text)
    {
        if (Made(text) is { } made)
            return (string.Format(made.English, made.Args), string.Format(T(made.English, "vi"), made.Args));
        string english = Language == "vi" ? Vi.FirstOrDefault(p => p.Value == text).Key ?? text : text;
        return (english, T(english, "vi"));
    }

    static readonly Dictionary<string, string> Vi = new()
    {
        ["Wrong master password."] = "Sai mật khẩu chủ.",
        ["The launcher you picked is gone. Choose it again under Settings › Open Roblox with."] =
            "Không còn thấy ứng dụng mở Roblox bạn đã chọn. Hãy chọn lại trong Cài đặt › Mở Roblox bằng.",
        ["Choose the program that opens Roblox"] = "Chọn chương trình dùng để mở Roblox",
        ["Programs"] = "Chương trình",
        ["Pick the program that opens Roblox, not Bloxkeep itself."] = "Hãy chọn chương trình mở Roblox, không phải chính Bloxkeep.",
        ["Open Bloxkeep"] = "Mở Bloxkeep",
        ["Quit Bloxkeep"] = "Thoát Bloxkeep",
        ["Mini window"] = "Cửa sổ nhỏ",
        ["Show"] = "Hiện",
        ["Close"] = "Đóng",
        ["Close all"] = "Đóng tất cả",
        ["Closing…"] = "Đang đóng…",
        ["Place {0}"] = "Place ID {0}",
        ["No game"] = "Chưa vào game",
        ["Disconnected"] = "Mất kết nối",
        ["No Roblox window is open."] = "Chưa mở cửa sổ Roblox nào.",
        ["Minimised — no live picture"] = "Đang thu nhỏ — không có hình trực tiếp",
        ["Close Roblox"] = "Đóng Roblox",
        ["Copy PID"] = "Chép PID",
        ["Unlock Bloxkeep to see the accounts."] = "Mở khóa Bloxkeep để xem các tài khoản.",
        ["Close ALL Roblox running on this PC (including Roblox in the system tray)?"] =
            "Đóng TẤT CẢ Roblox đang chạy trên máy (kể cả Roblox chạy ngầm ở khay hệ thống)?",
        ["{0} account needs signing in again: {1}"] = "{0} tài khoản cần đăng nhập lại: {1}",
        ["{0} accounts need signing in again: {1}"] = "{0} tài khoản cần đăng nhập lại: {1}",
        [" and {0} more"] = " và {0} tài khoản nữa",
        ["Bloxkeep is still running. Right-click this icon to quit."] = "Bloxkeep vẫn đang chạy. Bấm chuột phải vào biểu tượng này để thoát hẳn.",
        ["The data file is not in the right format or is damaged."] = "File dữ liệu không đúng định dạng hoặc đã bị hỏng.",
        ["Roblox refused the request (HTTP {0}). Try again in a moment."] = "Roblox từ chối yêu cầu (HTTP {0}). Thử lại sau một lát.",
        ["Roblox isn't working properly right now (HTTP {0}). Try again in a moment."] = "Roblox đang gặp trục trặc (HTTP {0}). Thử lại sau một lát.",
        ["This account was signed out — sign in to it again."] = "Tài khoản này đã bị đăng xuất — hãy đăng nhập lại.",
        ["Couldn't get a sign-in ticket (HTTP {0}). Try again in a moment."] = "Không lấy được vé đăng nhập (HTTP {0}). Thử lại sau một lát.",
        ["Couldn't open the private server link (HTTP {0}). Check the link, or try again in a moment."] = "Không mở được link server riêng (HTTP {0}). Kiểm tra lại link hoặc thử lại sau một lát.",
        ["The private server link is invalid or has expired."] = "Link server riêng không hợp lệ hoặc đã hết hạn.",
        ["Roblox is rate-limiting requests; try again shortly."] = "Roblox đang giới hạn tần suất, thử lại sau.",
        ["RobloxPlayerBeta.exe was not found. Pick another option under Settings › Open Roblox with, or reinstall Roblox."] =
            "Không tìm thấy RobloxPlayerBeta.exe. Hãy chọn ứng dụng mở Roblox khác trong Cài đặt › Mở Roblox bằng, hoặc cài lại Roblox.",
        ["{0} is not installed."] = "Chưa cài {0}.",
        ["The official Roblox client is not installed."] = "Chưa cài Roblox bản chính thức.",
        ["Roblox is not installed (the roblox-player protocol is missing)."] = "Máy chưa cài Roblox (không tìm thấy giao thức roblox-player).",
        ["Enter a Place ID or paste a Roblox game link."] = "Hãy nhập Place ID hoặc dán link game Roblox.",
        ["Only private-server share links (type=Server) are supported. For a regular game, paste a roblox.com/games/… link."] =
            "Chỉ hỗ trợ link chia sẻ server riêng (type=Server). Với game thường hãy dán link dạng roblox.com/games/…",
        ["Couldn't find a Place ID in this link."] = "Không đọc được Place ID từ link này.",
        ["Pick the game from the search list, or paste a game link or Place ID."] = "Chọn game trong danh sách tìm kiếm, hoặc dán link game hay Place ID.",
        ["Invalid Place ID."] = "Place ID không hợp lệ.",
        ["Invalid Job ID (expected xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)."] = "Job ID không hợp lệ (dạng xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).",
        ["Bloxkeep is locked."] = "Bloxkeep đang khóa.",
        ["Account not found."] = "Không tìm thấy tài khoản.",
        ["Couldn't save data: {0}"] = "Không lưu được dữ liệu: {0}",
        ["Unknown command: {0}"] = "Lệnh không hợp lệ: {0}",
        ["Unknown notification kind."] = "Loại thông báo không hợp lệ.",
        ["Couldn't reach Roblox. Check your internet connection and try again."] = "Không kết nối được tới Roblox. Kiểm tra mạng rồi thử lại.",
        ["Windows denied access to the file."] = "Windows không cho phép truy cập file.",
        ["Windows couldn't read or write the file (another program may be using it)."] = "Windows không đọc hoặc ghi được file (có thể chương trình khác đang dùng nó).",
        ["Roblox sent an answer Bloxkeep didn't expect. Try again in a moment."] = "Roblox trả về dữ liệu Bloxkeep không hiểu. Thử lại sau một lát.",
        ["Something went wrong. Try again in a moment."] = "Đã xảy ra lỗi. Thử lại sau một lát.",
        ["Couldn't open the data file: {0}"] = "Không mở được file dữ liệu: {0}",
        ["Windows couldn't open File Explorer."] = "Windows không mở được File Explorer.",
        ["Bloxkeep is already set up."] = "Bloxkeep đã được thiết lập rồi.",
        ["The master password needs at least 6 characters."] = "Mật khẩu chủ cần ít nhất 6 ký tự.",
        ["The master password can't be only spaces."] = "Mật khẩu chủ không thể chỉ có dấu cách.",
        ["The new password is the same as the current one."] = "Mật khẩu mới trùng với mật khẩu hiện tại.",
        ["Enter your master password."] = "Hãy nhập mật khẩu chủ.",
        ["The new password needs at least 6 characters."] = "Mật khẩu mới cần ít nhất 6 ký tự.",
        ["The current master password is incorrect."] = "Mật khẩu chủ hiện tại không đúng.",
        ["Getting account details…"] = "Đang lấy thông tin tài khoản…",
        ["Roblox didn't accept that sign-in. Please try again."] = "Roblox không chấp nhận phiên đăng nhập vừa rồi. Hãy thử lại.",
        ["Roblox didn't return a sign-in code."] = "Roblox không trả về mã đăng nhập.",
        ["Enter the username and password."] = "Nhập tên đăng nhập và mật khẩu.",
        ["Incorrect username or password."] = "Sai tên đăng nhập hoặc mật khẩu.",
        ["Roblox has locked this account to protect it. Reset its password on roblox.com (Forgot password) and sign in again, or sign in with your phone."] = "Roblox đã khóa tài khoản này để bảo vệ nó. Hãy đặt lại mật khẩu trên roblox.com (Quên mật khẩu) rồi đăng nhập lại, hoặc đăng nhập bằng điện thoại.",
        ["Roblox reports a problem with this account (it may be restricted or banned). Check it on roblox.com."] = "Roblox báo tài khoản này đang có vấn đề (có thể bị hạn chế hoặc bị cấm). Hãy kiểm tra trên roblox.com.",
        ["Roblox doesn't allow a password sign-in for this account here. Sign in with your phone or the browser instead."] = "Roblox không cho đăng nhập tài khoản này bằng mật khẩu ở đây. Hãy đăng nhập bằng điện thoại hoặc trình duyệt.",
        ["Roblox wants a captcha Bloxkeep didn't get. Try again, or sign in with your phone or the browser."] = "Roblox yêu cầu captcha nhưng Bloxkeep không nhận được. Thử lại, hoặc đăng nhập bằng điện thoại hay trình duyệt.",
        ["Roblox didn't accept the sign-in (HTTP {0}). Try again, or sign in with your phone or the browser."] = "Roblox không chấp nhận lần đăng nhập này (HTTP {0}). Thử lại, hoặc đăng nhập bằng điện thoại hay trình duyệt.",
        ["Too many sign-in attempts. Wait a few minutes and try again."] = "Đăng nhập sai quá nhiều lần. Chờ vài phút rồi thử lại.",
        ["Enter the verification code."] = "Nhập mã xác minh.",
        ["That verification code is wrong or expired."] = "Mã xác minh sai hoặc đã hết hạn.",
        ["Start the sign-in again."] = "Hãy đăng nhập lại từ đầu.",
        ["Roblox didn't say which captcha to show."] = "Roblox không cho biết cần hiện captcha nào.",
        ["Roblox didn't accept the sign-in check. Please try again."] = "Roblox không chấp nhận bước kiểm tra đăng nhập. Hãy thử lại.",
        ["Solve the puzzle"] = "Giải câu đố",
        ["Loading the puzzle…"] = "Đang tải câu đố…",
        ["Show the puzzle again"] = "Hiện lại câu đố",
        ["Try again"] = "Thử lại",
        ["Press and hold"] = "Nhấn và giữ",
        ["Press and hold the button until it fills up."] = "Nhấn và giữ nút cho tới khi đầy.",
        ["Checking…"] = "Đang kiểm tra…",
        ["That didn't work. Try again."] = "Chưa được. Thử lại nhé.",
        ["The puzzle was closed."] = "Câu đố đã bị đóng.",
        ["Couldn't load the puzzle. Check your internet connection and try again."] =
            "Không tải được câu đố. Kiểm tra kết nối mạng rồi thử lại.",
        ["The puzzle couldn't load. Close this window and sign in again."] = "Không tải được câu đố. Đóng cửa sổ này rồi đăng nhập lại.",
        ["Couldn't open the captcha: {0}"] = "Không mở được captcha: {0}",
        ["Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone."] =
            "Roblox yêu cầu một bước kiểm tra Bloxkeep chưa hiện được ({0}). Thử lại sau, hoặc đăng nhập bằng điện thoại.",
        ["Roblox asked for a sign-in check Bloxkeep doesn't support yet. Sign in with your phone or the browser instead. The details were saved in error.log (Settings › Data has its folder)."] =
            "Roblox yêu cầu một bước kiểm tra đăng nhập Bloxkeep chưa hỗ trợ. Hãy đăng nhập bằng điện thoại hoặc trình duyệt. Chi tiết đã được lưu trong error.log (thư mục có ở Cài đặt › Dữ liệu).",
        ["Roblox wants this account to turn on 2-step verification before it signs in. Sign in on roblox.com or in the Roblox app, turn it on there, then try again, or sign in with your phone."] =
            "Roblox muốn acc này bật xác minh 2 bước trước khi đăng nhập. Hãy đăng nhập trên roblox.com hoặc app Roblox, bật nó ở đó rồi thử lại, hoặc đăng nhập bằng điện thoại.",
        ["Roblox blocked this sign-in. Sign in with your phone or the browser instead."] =
            "Roblox đã chặn lần đăng nhập này. Hãy đăng nhập bằng điện thoại hoặc trình duyệt.",
        ["This account confirms sign-ins with a security key, a passkey or another device, which Bloxkeep can't do. Sign in with your phone or the browser instead."] =
            "Acc này xác nhận đăng nhập bằng khóa bảo mật, passkey hoặc thiết bị khác, Bloxkeep chưa làm được. Hãy đăng nhập bằng điện thoại hoặc trình duyệt.",
        ["That code was used already. Wait for the next one in the authenticator app."] = "Mã này đã được dùng rồi. Chờ mã tiếp theo trong app xác thực.",
        ["This sign-in has expired. Sign in again."] = "Lần đăng nhập này đã hết hạn. Hãy đăng nhập lại.",
        ["Enter the code from the authenticator app."] = "Nhập mã từ app xác thực.",
        ["{0} is a Roblox VNG account. Opening Roblox VNG isn't supported yet."] =
            "{0} là acc Roblox VNG. Bloxkeep chưa mở được Roblox VNG.",
        ["Roblox wants an extra check for this sign-in. Use Sign in with browser instead."] =
            "Roblox cần xác minh thêm cho lần đăng nhập này. Hãy dùng Đăng nhập bằng trình duyệt.",
        ["Roblox wants an extra check for this sign-in. Sign in with the username and password instead."] =
            "Roblox cần xác minh thêm cho lần đăng nhập này. Hãy đăng nhập bằng tên đăng nhập và mật khẩu.",
        ["Roblox keeps asking for a new puzzle. Sign in with your phone instead, or try again later."] =
            "Roblox cứ đòi giải đố mới. Hãy đăng nhập bằng điện thoại, hoặc thử lại sau.",
        ["Too many tries. Wait {0} s, then try again."] = "Thử quá nhiều lần. Chờ {0} giây rồi thử lại.",
        ["Paste the .ROBLOSECURITY cookie."] = "Hãy dán cookie .ROBLOSECURITY.",
        ["The cookie is invalid or has expired."] = "Cookie không hợp lệ hoặc đã hết hạn.",
        ["The new cookie is invalid or has expired."] = "Cookie mới không hợp lệ hoặc đã hết hạn.",
        ["Bloxkeep was locked."] = "Bloxkeep vừa bị khóa.",
        ["Invalid field."] = "Trường không hợp lệ.",
        ["Already checking — please wait."] = "Đang kiểm tra rồi, chờ chút nhé.",
        ["Checking sign-ins… {0}/{1}"] = "Đang kiểm tra đăng nhập… {0}/{1}",
        ["This account has no saved username."] = "Tài khoản này chưa lưu tên đăng nhập.",
        ["This account has no saved password."] = "Tài khoản này chưa lưu mật khẩu.",
        ["This account has no saved cookie."] = "Tài khoản này chưa lưu cookie.",
        ["Couldn't access the clipboard; try again."] = "Không truy cập được clipboard, thử lại sau.",
        ["Multi-Roblox isn't working yet: Roblox was already open before it was turned on."] =
            "Multi-Roblox chưa hoạt động vì Roblox đã mở từ trước khi bật tính năng này.",
        ["Roblox also starts by itself in the background when Windows starts."] =
            "Roblox cũng đang tự chạy ngầm mỗi khi bật máy.",
        ["A new Roblox window may close the old one."] = "Cửa sổ Roblox mới có thể đóng cửa sổ cũ.",
        ["Multi-Roblox is off, so each new Roblox window closes the previous one."] =
            "Multi-Roblox đang tắt nên mỗi lần mở Roblox mới, cửa sổ cũ sẽ bị đóng.",
        ["Only {0} of RAM is free, and {1} more Roblox window needs about {2}. The PC may slow down or Roblox may crash."] =
            "Máy chỉ còn trống {0} RAM, trong khi mở thêm {1} cửa sổ Roblox cần khoảng {2}. Máy có thể bị lag hoặc Roblox bị văng.",
        ["Only {0} of RAM is free, and {1} more Roblox windows need about {2}. The PC may slow down or Roblox may crash."] =
            "Máy chỉ còn trống {0} RAM, trong khi mở thêm {1} cửa sổ Roblox cần khoảng {2}. Máy có thể bị lag hoặc Roblox bị văng.",
        ["Open anyway?"] = "Vẫn mở?",
        ["This file is too large to be an account list."] = "File này quá lớn, không phải danh sách tài khoản.",
        ["This file is still encrypted. Decrypt it first with RAM's official RAMDecrypt tool, then import the decrypted file."] =
            "File này vẫn đang được mã hóa. Hãy giải mã bằng công cụ RAMDecrypt chính thức của RAM trước, rồi nhập file đã giải mã.",
        ["Imported accounts checked: {0} valid, {1} expired."] = "Đã kiểm tra các tài khoản vừa nhập: {0} hợp lệ, {1} hết hạn.",
        ["Choose another Bloxkeep data file"] = "Chọn một file dữ liệu Bloxkeep khác",
        ["Bloxkeep data files"] = "File dữ liệu Bloxkeep",
        ["This file is too large to be a Bloxkeep data file."] = "File này quá lớn, không phải file dữ liệu Bloxkeep.",
        ["This is the vault you're using."] = "Đây chính là dữ liệu Bloxkeep bạn đang dùng.",
        ["This file is not a Bloxkeep data file (.rmv)."] = "File này không phải file dữ liệu Bloxkeep (.rmv).",
        ["Choose the file first."] = "Hãy chọn file trước.",
        ["Enter the master password of that file."] = "Nhập mật khẩu chủ của file đó.",
        ["Still opening the file — please wait."] = "Đang mở file, chờ chút nhé.",
        ["Too many wrong tries. Wait {0} seconds, then try again."] = "Sai quá nhiều lần. Chờ {0} giây rồi thử lại.",
        ["Wrong master password for this file."] = "Sai mật khẩu chủ của file này.",
        ["The file was closed because it was open too long. Open it again."] = "File đã được đóng vì để mở quá lâu. Hãy mở lại.",
        ["Imported preset"] = "Preset đã nhập",
        [" (imported)"] = " (đã nhập)",
        [" (imported {0})"] = " (đã nhập {0})",
        ["{0} left the game — rejoining in {1}s…"] = "{0} đã thoát game — tự vào lại sau {1} giây…",
        ["{0} seems disconnected — restarting Roblox…"] = "{0} có vẻ đã mất kết nối — đang mở lại Roblox…",
        ["{0} keeps getting disconnected — trying again every few minutes."] = "{0} cứ bị văng khỏi game — Bloxkeep sẽ thử vào lại sau mỗi vài phút.",
        ["{0} left the game again — rejoining in {1} min."] = "{0} lại thoát game — tự vào lại sau {1} phút.",
        ["Restarting {0}'s Roblox (its preset restarts it every {1} h)…"] = "Đang mở lại Roblox của {0} (preset đặt mở lại mỗi {1} giờ)…",
        ["Couldn't rejoin {0}: {1}"] = "Không vào lại được {0}: {1}",
        ["Didn't rejoin {0}: Multi-Roblox isn't ready, so a new Roblox window would close the one that's open."] =
            "Chưa vào lại {0}: Multi-Roblox chưa sẵn sàng, mở Roblox mới sẽ đóng mất cửa sổ đang mở.",
        ["Waiting for the previous Roblox window to open…"] = "Chờ cửa sổ Roblox trước mở xong…",
        ["Couldn't restart to update. It installs by itself when you quit Bloxkeep."] = "Không khởi động lại để cập nhật được. Bản cập nhật sẽ tự cài khi bạn thoát Bloxkeep.",
        ["Bloxkeep was updated to {0}. Unlock it to carry on."] = "Bloxkeep đã cập nhật lên {0}. Mở khóa để tiếp tục.",
        ["The update to {0} didn't install, so Bloxkeep started again as it was. Unlock it to carry on."] = "Bản cập nhật {0} chưa cài được nên Bloxkeep đã mở lại như cũ. Mở khóa để tiếp tục.",
        ["The update is still downloading. Try again in a moment."] = "Bản cập nhật vẫn đang tải. Thử lại sau một lát.",
        ["No update is ready to install. Check for updates again."] = "Chưa có bản cập nhật nào sẵn sàng để cài. Hãy kiểm tra cập nhật lại.",
        ["Couldn't reach the update server. Check your internet connection."] = "Không kết nối được máy chủ cập nhật. Kiểm tra mạng.",
        ["The update couldn't be checked or downloaded. Bloxkeep tries again later."] = "Không kiểm tra hoặc tải được bản cập nhật. Bloxkeep sẽ thử lại sau.",
        ["Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow."] =
            "Máy đang dùng {0}% RAM. Hãy đóng bớt cửa sổ Roblox nếu thấy lag.",
        ["Roblox refused memory trimming, so it was skipped."] = "Roblox không cho phép giảm RAM nên bước này được bỏ qua.",
        ["Already launching — wait a moment or press Stop."] = "Đang mở tài khoản khác — chờ chút hoặc bấm Dừng.",
        ["Select at least one account."] = "Hãy chọn ít nhất một tài khoản.",
        ["Reading the private server link…"] = "Đang đọc link server riêng…",
        ["{0}: already running — skipped."] = "{0}: đang chạy rồi — bỏ qua.",
        ["{0}: already in this game — skipped."] = "{0}: đang ở game này rồi — bỏ qua.",
        ["{0}: already on the home screen — skipped."] = "{0}: đang ở trang chủ rồi — bỏ qua.",
        ["{0} will leave the game and go to the Roblox home screen."] = "{0} sẽ thoát game và về trang chủ Roblox.",
        ["{0} will leave what they're playing now and open this game instead."] = "{0} sẽ thoát game đang chơi và vào game này.",
        ["Closing {0}'s Roblox to switch games…"] = "Đang đóng Roblox của {0} để chuyển game…",
        ["Launching {0} ({1}/{2})…"] = "Đang mở {0} ({1}/{2})…",
        ["Give the preset a name."] = "Đặt tên cho preset đã.",
        ["Enter the server's Job ID."] = "Hãy nhập Job ID của server.",
        ["You can save up to {0} presets."] = "Chỉ lưu được tối đa {0} preset.",
        ["Preset not found."] = "Không tìm thấy preset.",
        ["This preset has no accounts left."] = "Preset này không còn tài khoản nào.",
        [" (copy)"] = " (bản sao)",
        [" (copy {0})"] = " (bản sao {0})",
        ["This preset has no schedule. Edit it to add one."] = "Preset này chưa hẹn giờ. Bấm Sửa để thêm.",
        ["Finding a server…"] = "Đang tìm server…",
        ["No server has room for {0} accounts right now, so Roblox picks one."] =
            "Không có server nào đủ chỗ cho {0} acc, nên để Roblox tự chọn server.",
        ["Waiting {0}s before the next account…"] = "Chờ {0} giây trước tài khoản tiếp theo…",
        ["Waiting for Roblox to open {0}… (Roblox is updating to {1})"] = "Chờ Roblox mở {0}… (Roblox đang cập nhật lên {1})",
        ["Waiting for Roblox to open {0}… (Roblox may be updating)"] = "Chờ Roblox mở {0}… (Roblox có thể đang cập nhật)",
        ["{0} was kicked for being AFK. Anti-AFK doesn't work while the PC is locked."] = "{0} bị kick vì AFK. Chống AFK không chạy khi máy đang khóa.",
        ["Wait until Bloxkeep finishes opening Roblox."] = "Chờ Bloxkeep mở Roblox xong đã.",
        ["The PC woke up — auto-rejoin waits until the connection is back."] = "Máy vừa thức dậy — tự vào lại game sẽ chờ có mạng trở lại.",
        ["Couldn't turn it off: {0}"] = "Không tắt được: {0}",
        ["Sign in to Roblox"] = "Đăng nhập Roblox",
        ["Sign in to each account in turn. Press Done when finished."] = "Lần lượt đăng nhập từng tài khoản. Xong thì bấm Xong.",
        ["Done"] = "Xong",
        ["Sign in to the next account, or press Done."] = "Đăng nhập tài khoản tiếp theo, hoặc bấm Xong.",
        ["{0} saved."] = "Đã lưu {0}.",
        ["{0} was already here — its sign-in was updated."] = "{0} đã có — đã cập nhật phiên đăng nhập mới.",
        ["Waiting for Roblox to finish closing…"] = "Chờ Roblox đóng hẳn…",
        ["Microsoft Edge WebView2 isn't installed. Install it, or add the account with a cookie."] =
            "Máy chưa có Microsoft Edge WebView2. Hãy cài nó, hoặc thêm acc bằng cookie.",
        ["Couldn't open the sign-in browser: {0}"] = "Không mở được trình duyệt đăng nhập: {0}",
        ["Retry"] = "Thử lại",
        ["Opening the Roblox sign-in page…"] = "Đang mở trang đăng nhập Roblox…",
        ["Can't reach roblox.com. Check your internet connection, then press Retry."] =
            "Không kết nối được roblox.com. Kiểm tra mạng rồi bấm Thử lại.",
        ["Microsoft Edge WebView2 isn't installed. Install it from Microsoft's website, then reopen Bloxkeep."] = "Máy chưa có Microsoft Edge WebView2. Hãy cài từ trang của Microsoft rồi mở lại Bloxkeep.",
        ["Unexpected error: {0}"] = "Lỗi không mong muốn: {0}",
        ["Couldn't restart the interface: {0}"] = "Không khởi động lại được giao diện: {0}",
        ["Couldn't open the interface: {0}"] = "Không mở được giao diện: {0}",
        ["Couldn't save your changes. Quit anyway and lose them?"] = "Không lưu được thay đổi của bạn. Vẫn thoát và mất chúng?",
        ["Couldn't save your changes, so Bloxkeep stays unlocked. Try again in a moment."] =
            "Không lưu được thay đổi của bạn nên Bloxkeep vẫn mở khóa. Thử lại sau một lát.",
        ["Couldn't save your changes, so the update waits. Try again in a moment."] =
            "Không lưu được thay đổi của bạn nên bản cập nhật tạm chờ. Thử lại sau một lát.",
        ["This data file was made by a newer Bloxkeep. Update Bloxkeep to open it."] =
            "File dữ liệu này do bản Bloxkeep mới hơn tạo ra. Hãy cập nhật Bloxkeep để mở.",
        ["{0} didn't rejoin: no Roblox window appeared."] = "{0} chưa vào lại được: không thấy cửa sổ Roblox mở lên.",
        ["Bloxkeep needs Microsoft Edge WebView2, which isn't installed. Install it now? (about 2 MB)"] = "Bloxkeep cần Microsoft Edge WebView2 nhưng máy chưa có. Cài ngay? (khoảng 2 MB)",
        ["The downloaded WebView2 installer is not signed by Microsoft, so it was not run."] =
            "Bộ cài WebView2 vừa tải không có chữ ký của Microsoft nên đã không chạy.",
        ["Couldn't install WebView2: {0}"] = "Không cài được WebView2: {0}",
        ["{0} account didn't open."] = "{0} tài khoản không mở được.",
        ["{0} accounts didn't open."] = "{0} tài khoản không mở được.",
        ["Couldn't open the backup folder: {0}"] = "Không mở được thư mục sao lưu: {0}",
        ["Roblox updated to {0}."] = "Roblox đã cập nhật lên {0}.",
        ["Roblox closed {0} window(s) when another one opened — Roblox may have blocked running several windows. Multi-Roblox may need an update."] =
            "Roblox đã đóng {0} cửa sổ khi một cửa sổ khác mở ra — có thể Roblox đã chặn chạy nhiều cửa sổ. Multi-Roblox có thể cần cập nhật.",
        ["This file is not a Bloxkeep backup."] = "File này không phải bản sao lưu của Bloxkeep.",
        ["Choose a backup folder first."] = "Hãy chọn thư mục sao lưu trước.",
        ["Couldn't back up: {0}"] = "Không sao lưu được: {0}",
        ["Choose a folder on this PC or a synced drive."] = "Hãy chọn thư mục trên máy này hoặc ổ đồng bộ.",
        ["Pick a folder outside Bloxkeep's own data folder."] = "Hãy chọn thư mục nằm ngoài thư mục dữ liệu của Bloxkeep.",
        ["Choose a folder for backups"] = "Chọn thư mục để sao lưu",
        ["Unlock Bloxkeep first, then restore from Settings › Backup."] = "Hãy mở khóa Bloxkeep trước, rồi khôi phục trong Cài đặt › Sao lưu.",
        ["Choose a Bloxkeep backup"] = "Chọn bản sao lưu của Bloxkeep",
        ["Bloxkeep backups"] = "Bản sao lưu Bloxkeep",
        ["Couldn't restore: {0}"] = "Không khôi phục được: {0}",
        ["Backup restored. Unlock it with the master password it was saved with."] = "Đã khôi phục bản sao lưu. Mở khóa bằng mật khẩu chủ lúc sao lưu.",
        ["This account has no Roblox window open."] = "Tài khoản này chưa mở cửa sổ Roblox nào.",
        ["Enter the start time as HH:mm."] = "Nhập giờ bắt đầu dạng HH:mm.",
        ["Pick the date to run on."] = "Chọn ngày chạy.",
        ["That date and time have already passed."] = "Ngày giờ đó đã qua rồi.",
        ["Pick at least one weekday, or a date."] = "Chọn ít nhất một ngày trong tuần, hoặc một ngày cụ thể.",
        ["Enter how many hours to play (up to {0})."] = "Nhập số giờ chơi (tối đa {0}).",
        ["Enter the stop time as HH:mm."] = "Nhập giờ dừng dạng HH:mm.",
        ["{0} didn't run at {1}: Bloxkeep was locked."] = "{0} không chạy lúc {1}: Bloxkeep đang khóa.",
        ["{0} didn't run at {1}: the PC was asleep or busy."] = "{0} không chạy lúc {1}: máy đang ngủ hoặc bận.",
        ["(deleted)"] = "(đã xóa)",
        ["Scheduled preset {0} didn't run: {1}"] = "Preset hẹn giờ {0} không chạy: {1}",
        ["Scheduled preset {0} started: {1} account opened."] = "Preset hẹn giờ {0} đã chạy: mở {1} tài khoản.",
        ["Scheduled preset {0} started: {1} accounts opened."] = "Preset hẹn giờ {0} đã chạy: mở {1} tài khoản.",
        ["Scheduled preset {0}: its accounts were already open."] = "Preset hẹn giờ {0}: các tài khoản đã mở sẵn rồi.",
        ["Scheduled preset {0}: {1} account didn't open. {2}"] = "Preset hẹn giờ {0}: {1} tài khoản không mở được. {2}",
        ["Scheduled preset {0}: {1} accounts didn't open. {2}"] = "Preset hẹn giờ {0}: {1} tài khoản không mở được. {2}",
        ["Scheduled preset {0} finished: closing {1} account."] = "Preset hẹn giờ {0} đã hết giờ: đang đóng {1} tài khoản.",
        ["Scheduled preset {0} finished: closing {1} accounts."] = "Preset hẹn giờ {0} đã hết giờ: đang đóng {1} tài khoản.",
        // Discord (AppController.Discord.cs)
        ["Enter the time as HH:mm."] = "Nhập giờ dạng HH:mm.",
        ["Discord doesn't allow that name (up to 80 characters, without \"discord\" or \"clyde\")."] =
            "Discord không cho dùng tên này (tối đa 80 ký tự, không chứa \"discord\" hay \"clyde\").",
        ["The picture link must start with https://."] = "Link ảnh phải bắt đầu bằng https://.",
        ["Unknown language."] = "Ngôn ngữ không hợp lệ.",
        ["Unknown message style."] = "Kiểu tin nhắn không hợp lệ.",
        ["Pick at least one thing for the summary."] = "Chọn ít nhất một mục cho bản tổng kết.",
        ["Unknown period."] = "Khoảng thời gian không hợp lệ.",
        ["Unknown weekday."] = "Ngày trong tuần không hợp lệ.",
        ["Update the status message every 2 to 60 minutes."] = "Cập nhật tin trạng thái mỗi 2 đến 60 phút.",
        ["A role ID is 17 to 20 digits. Copy it with Developer Mode on: right-click the role › Copy Role ID."] =
            "ID vai trò gồm 17 đến 20 chữ số. Bật Chế độ nhà phát triển rồi bấm chuột phải vào vai trò › Sao chép ID vai trò.",
        ["A user ID is 17 to 20 digits. Copy it with Developer Mode on: right-click the person › Copy User ID."] =
            "ID người dùng gồm 17 đến 20 chữ số. Bật Chế độ nhà phát triển rồi bấm chuột phải vào người đó › Sao chép ID người dùng.",
        ["Paste a webhook link first."] = "Hãy dán link webhook trước.",
        ["That isn't a Discord webhook link. It looks like https://discord.com/api/webhooks/… (Channel settings › Integrations › Webhooks › Copy Webhook URL)."] =
            "Đây không phải link webhook Discord. Link đúng có dạng https://discord.com/api/webhooks/… (Cài đặt kênh › Tích hợp › Webhook › Sao chép URL webhook).",
        ["Discord says the webhook no longer exists, so Bloxkeep stopped sending to it. Paste a new link in Settings › Discord."] =
            "Discord báo webhook không còn nữa nên Bloxkeep đã ngừng gửi. Hãy dán link mới trong Cài đặt › Discord.",
        ["Discord says this webhook doesn't exist: it was deleted, or the link is wrong. Paste a new link."] =
            "Discord báo webhook này không tồn tại: đã bị xóa hoặc link sai. Hãy dán link mới.",
        ["Discord asks to wait {0} s before the next message."] = "Discord yêu cầu chờ {0} giây trước tin tiếp theo.",
        ["Couldn't reach Discord: {0}"] = "Không kết nối được tới Discord: {0}",
        ["Couldn't reach Discord. Check your internet connection."] = "Không kết nối được tới Discord. Kiểm tra mạng.",
        ["Discord refused the message: {0}"] = "Discord từ chối tin nhắn: {0}",
        ["Discord refused a message: {0}"] = "Discord từ chối một tin nhắn: {0}",
        ["Account dropped"] = "Tài khoản bị văng",
        ["Scheduled preset"] = "Preset hẹn giờ",
        ["Sign-in needs renewing"] = "Cần đăng nhập lại",
        ["Low RAM"] = "Sắp hết RAM",
        ["Launch problem"] = "Lỗi khi mở game",
        ["{0} events"] = "{0} sự kiện",
        ["{0} min"] = "{0} phút",
        ["{0} h {1} min"] = "{0} giờ {1} phút",
        ["today"] = "hôm nay",
        ["last 7 days"] = "7 ngày qua",
        ["last 24 hours"] = "24 giờ qua",
        ["24 hours to {0}"] = "24 giờ tính đến {0}",
        ["last 30 days"] = "30 ngày qua",
        ["Play time"] = "Thời gian chơi",
        ["Sessions"] = "Số lượt chơi",
        ["Disconnects"] = "Số lần mất kết nối",
        ["Deleted account"] = "Tài khoản đã xóa",
        // Hide account names, bans, authenticator keys
        ["Account {0}"] = "Tài khoản {0}",
        ["{0} is banned on Roblox."] = "{0} đã bị Roblox ban.",
        ["{0} account is banned on Roblox: {1}"] = "{0} tài khoản đã bị Roblox ban: {1}",
        ["{0} accounts are banned on Roblox: {1}"] = "{0} tài khoản đã bị Roblox ban: {1}",
        ["{0} is banned on Roblox, so it wasn't opened. If the ban is over, Check sign-in clears this."] =
            "{0} đã bị Roblox ban nên không được mở. Nếu đã hết ban, bấm Kiểm tra đăng nhập để bỏ dấu này.",
        ["Account banned"] = "Tài khoản bị ban",
        ["This account has no authenticator key."] = "Tài khoản này chưa lưu khóa xác thực.",
        ["The key can't contain \"{0}\": it only has the letters A–Z and the digits 2–7. Check it for a typo."] =
            "Khóa không thể có \"{0}\": khóa chỉ gồm chữ A–Z và số 2–7. Kiểm tra xem có gõ nhầm không.",
        ["The key is too short. Check that you copied all of it."] = "Khóa quá ngắn. Kiểm tra xem đã sao chép đủ chưa.",
        ["The key looks cut off or has an extra character. Check that you copied all of it."] =
            "Khóa có vẻ bị thiếu hoặc thừa một ký tự. Kiểm tra xem đã sao chép đủ chưa.",
        ["This link makes counter-based codes. Bloxkeep only makes time-based codes, the kind Roblox uses."] =
            "Link này tạo mã theo bộ đếm. Bloxkeep chỉ tạo mã theo thời gian, loại mà Roblox dùng.",
        ["The link has no key in it."] = "Link này không có khóa.",
        ["Bloxkeep can't make codes with this setting from the link: {0}."] = "Bloxkeep không tạo được mã với thiết lập này trong link: {0}.",
        ["That isn't an authenticator key. Paste the key (letters A–Z and digits 2–7) or the otpauth:// link."] =
            "Đây không phải khóa xác thực. Hãy dán khóa (chữ A–Z và số 2–7) hoặc link otpauth://.",
        ["Per account"] = "Theo tài khoản",
        ["Top games"] = "Game chơi nhiều nhất",
        ["Nothing played in this period."] = "Chưa chơi gì trong khoảng này.",
        // Statistics › Activity (AppController.Activity.cs)
        ["Opened Roblox to play {0}."] = "Đã mở Roblox để chơi {0}.",
        ["Opened Roblox on its home screen."] = "Đã mở Roblox ở màn hình chính.",
        ["Auto-rejoin opened Roblox again to play {0}."] = "Tự vào lại: đã mở lại Roblox để chơi {0}.",
        ["Auto-rejoin opened Roblox again."] = "Tự vào lại: đã mở lại Roblox.",
        ["Closed for its preset's planned restart."] = "Đã đóng để mở lại theo lịch của preset.",
        ["Opened again after its planned restart, to play {0}."] = "Đã mở lại theo lịch của preset để chơi {0}.",
        ["Opened again after its planned restart."] = "Đã mở lại theo lịch của preset.",
        ["Joined {0}."] = "Đã vào {0}.",
        ["Left {0}."] = "Đã rời {0}.",
        ["Disconnected from {0}: {1}."] = "Bị văng khỏi {0}: {1}.",
        ["Disconnected from {0}."] = "Bị văng khỏi {0}.",
        ["Signed out in the Roblox window."] = "Đã đăng xuất trong cửa sổ Roblox.",
        ["Roblox window closed."] = "Đã đóng cửa sổ Roblox.",
        ["Roblox closed this window when another one opened."] = "Roblox đã đóng cửa sổ này khi một cửa sổ khác mở ra.",
        ["kicked for being idle"] = "bị kick vì AFK",
        ["kicked by the game"] = "bị game kick",
        ["signed in on another device"] = "đăng nhập ở thiết bị khác",
        ["connection lost"] = "rớt mạng",
        ["couldn't join the server"] = "không vào được server",
        ["Roblox code {0}"] = "mã lỗi Roblox {0}",
        ["Bloxkeep {0} is ready to install: it installs when Bloxkeep restarts."] = "Bloxkeep {0} đã sẵn sàng: sẽ được cài khi Bloxkeep mở lại.",
        ["There is nothing to export."] = "Không có gì để xuất.",
        ["Export the activity log"] = "Xuất nhật ký hoạt động",
        ["CSV file"] = "File CSV",
        ["Couldn't save the file: {0}"] = "Không lưu được file: {0}",
        ["Time"] = "Thời gian",
        ["Account"] = "Tài khoản",
        ["Kind"] = "Loại",
        ["Event"] = "Sự kiện",
        ["Game"] = "Game",
        ["Games"] = "Vào/ra game",
        ["Schedules"] = "Hẹn giờ",
        ["Sign-in"] = "Đăng nhập",
        ["Updates"] = "Cập nhật",
        ["Disconnects and problems"] = "Văng và lỗi",
        // Join a player (AppController.JoinPlayer.cs)
        ["Type the player's Roblox username or user ID."] = "Nhập tên đăng nhập Roblox hoặc ID người dùng của người chơi.",
        ["A Roblox username is 3 to 20 letters, digits or _ (the username, not the display name). A user ID or a profile link works too."] =
            "Tên đăng nhập Roblox gồm 3 đến 20 chữ cái, chữ số hoặc dấu _ (tên đăng nhập, không phải tên hiển thị). Cũng có thể dùng ID người dùng hoặc link trang cá nhân.",
        ["None of the selected accounts is signed in, so Bloxkeep can't ask Roblox where this player is. Sign them in again first."] =
            "Không acc nào đã chọn còn đăng nhập, nên Bloxkeep không hỏi Roblox được người chơi này đang ở đâu. Hãy đăng nhập lại trước.",
        ["Roblox didn't accept the selected accounts' sign-in. Check their sign-in, then try again."] =
            "Roblox không nhận phần đăng nhập của các acc đã chọn. Hãy kiểm tra đăng nhập rồi thử lại.",
        ["Roblox didn't say where this player is. Try again in a moment."] = "Roblox không cho biết người chơi này đang ở đâu. Thử lại sau giây lát.",
        ["That server can't be joined. Look the player up again."] = "Không vào được server đó. Hãy tìm lại người chơi.",
        ["{0} chosen account."] = "{0} tài khoản đã chọn.",
        ["{0} chosen accounts."] = "{0} tài khoản đã chọn.",
        ["Play time — {0}"] = "Thời gian chơi — {0}",
        ["{0} (late)"] = "{0} (gửi muộn)",
        ["Bloxkeep was closed."] = "Bloxkeep đã đóng.",
        ["In game"] = "Trong game",
        ["Joining"] = "Đang vào",
        ["In the menu"] = "Ở màn hình chính",
        ["and {0} more"] = "và {0} tài khoản nữa",
        ["{0} account open."] = "{0} tài khoản đang mở.",
        ["{0} accounts open."] = "{0} tài khoản đang mở.",
        ["Bloxkeep status"] = "Trạng thái Bloxkeep",
        ["Updated every {0} min"] = "Cập nhật mỗi {0} phút",
        ["({0} older message couldn't be sent while offline and was dropped.)"] = "({0} tin cũ không gửi được lúc mất mạng nên đã bỏ.)",
        ["({0} older messages couldn't be sent while offline and were dropped.)"] = "({0} tin cũ không gửi được lúc mất mạng nên đã bỏ.)",
        ["OK"] = "OK",
        ["Cancel"] = "Hủy",
        ["Quit anyway"] = "Vẫn thoát",
        ["Keep Bloxkeep open"] = "Không thoát",
        ["Install"] = "Cài đặt",
        ["Not now"] = "Để sau",
        ["Uninstall Bloxkeep"] = "Gỡ cài đặt Bloxkeep",
        ["Delete your accounts too?"] = "Xóa luôn tài khoản?",
        ["Bloxkeep is being removed from this PC. Your accounts and settings stay unless you delete them."] =
            "Bloxkeep đang được gỡ khỏi máy. Tài khoản và cài đặt vẫn được giữ, trừ khi bạn chọn xóa.",
        ["Keep my accounts and settings"] = "Giữ tài khoản và cài đặt",
        ["Installing Bloxkeep again picks up where you left off."] = "Cài lại Bloxkeep là dùng tiếp như cũ.",
        ["Delete my accounts and settings"] = "Xóa tài khoản và cài đặt",
        ["Deletes the vault with your saved accounts, your settings and logs in {0}. This can't be undone."] =
            "Xóa vault chứa các tài khoản đã lưu, cài đặt và nhật ký trong {0}. Không khôi phục lại được.",
        ["Kept if you don't choose within a minute."] = "Không chọn trong vòng một phút thì sẽ giữ lại.",
        ["Test message"] = "Tin nhắn thử",
        ["Bloxkeep can post to this channel. Choose what it sends under Settings › Discord."] =
            "Bloxkeep gửi được vào kênh này. Chọn những gì sẽ gửi trong Cài đặt › Discord.",
    };
}
