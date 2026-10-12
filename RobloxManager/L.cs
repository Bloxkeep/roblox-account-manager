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

    public static string T(string english) =>
        Language == "vi" && Vi.TryGetValue(english, out var vi) ? vi : english;

    public static string F(string english, params object?[] args) => string.Format(T(english), args);

    static readonly Dictionary<string, string> Vi = new()
    {
        ["Wrong master password."] = "Sai mật khẩu chủ.",
        ["Open Bloxkeep"] = "Mở Bloxkeep",
        ["Quit Bloxkeep"] = "Thoát Bloxkeep",
        ["Bloxkeep is still running. Right-click this icon to quit."] = "Bloxkeep vẫn đang chạy. Bấm chuột phải vào biểu tượng này để thoát hẳn.",
        ["The data file is not in the right format or is damaged."] = "File dữ liệu không đúng định dạng hoặc đã bị hỏng.",
        ["Roblox returned HTTP {0}: {1}"] = "Roblox trả lỗi HTTP {0}: {1}",
        ["This account was signed out — sign in to it again."] = "Tài khoản này đã bị đăng xuất — hãy đăng nhập lại.",
        ["Couldn't get a sign-in ticket (HTTP {0}). {1}"] = "Không lấy được vé đăng nhập (HTTP {0}). {1}",
        ["Couldn't open the private server link (HTTP {0}). {1}"] = "Không mở được link server riêng (HTTP {0}). {1}",
        ["The private server link is invalid or has expired."] = "Link server riêng không hợp lệ hoặc đã hết hạn.",
        ["Roblox is rate-limiting requests; try again shortly."] = "Roblox đang giới hạn tần suất, thử lại sau.",
        ["RobloxPlayerBeta.exe was not found. Pick another option under Settings › Roblox client, or reinstall Roblox."] =
            "Không tìm thấy RobloxPlayerBeta.exe. Hãy chọn trình mở khác trong Cài đặt › Trình mở Roblox, hoặc cài lại Roblox.",
        ["{0} is not installed."] = "Chưa cài {0}.",
        ["The official Roblox client is not installed."] = "Chưa cài Roblox bản chính thức.",
        ["Roblox is not installed (the roblox-player protocol is missing)."] = "Máy chưa cài Roblox (không tìm thấy giao thức roblox-player).",
        ["Enter a Place ID or paste a Roblox game link."] = "Hãy nhập Place ID hoặc dán link game Roblox.",
        ["Only private-server share links (type=Server) are supported. For a regular game, paste a roblox.com/games/… link."] =
            "Chỉ hỗ trợ link chia sẻ server riêng (type=Server). Với game thường hãy dán link dạng roblox.com/games/…",
        ["Couldn't find a Place ID in this link."] = "Không đọc được Place ID từ link này.",
        ["Invalid Place ID."] = "Place ID không hợp lệ.",
        ["Invalid Job ID (expected xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)."] = "Job ID không hợp lệ (dạng xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).",
        ["Bloxkeep is locked."] = "Bloxkeep đang khóa.",
        ["Account not found."] = "Không tìm thấy tài khoản.",
        ["Couldn't save data: {0}"] = "Không lưu được dữ liệu: {0}",
        ["Unknown command: {0}"] = "Lệnh không hợp lệ: {0}",
        ["Couldn't reach Roblox: {0}"] = "Lỗi kết nối tới Roblox: {0}",
        ["Error: {0}"] = "Lỗi: {0}",
        ["Bloxkeep is already set up."] = "Bloxkeep đã được thiết lập rồi.",
        ["The master password needs at least 6 characters."] = "Mật khẩu chủ cần ít nhất 6 ký tự.",
        ["Enter your master password."] = "Hãy nhập mật khẩu chủ.",
        ["The new password needs at least 6 characters."] = "Mật khẩu mới cần ít nhất 6 ký tự.",
        ["The current master password is incorrect."] = "Mật khẩu chủ hiện tại không đúng.",
        ["Getting account details…"] = "Đang lấy thông tin tài khoản…",
        ["Roblox didn't accept that sign-in. Please try again."] = "Roblox không chấp nhận phiên đăng nhập vừa rồi. Hãy thử lại.",
        ["Roblox didn't return a sign-in code."] = "Roblox không trả về mã đăng nhập.",
        ["Enter the username and password."] = "Nhập tên tài khoản và mật khẩu.",
        ["Incorrect username or password."] = "Sai tên tài khoản hoặc mật khẩu.",
        ["Too many sign-in attempts. Wait a few minutes and try again."] = "Đăng nhập sai quá nhiều lần. Chờ vài phút rồi thử lại.",
        ["Enter the verification code."] = "Nhập mã xác minh.",
        ["That verification code is wrong or expired."] = "Mã xác minh sai hoặc đã hết hạn.",
        ["Roblox didn't return a verification token."] = "Roblox không trả về mã xác thực.",
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
        ["{0} is a Roblox VNG account. Opening Roblox VNG isn't supported yet."] =
            "{0} là acc Roblox VNG. Bloxkeep chưa mở được Roblox VNG.",
        ["Roblox wants an extra check for this sign-in. Use Sign in with browser instead."] =
            "Roblox cần xác minh thêm cho lần đăng nhập này. Hãy dùng Đăng nhập bằng trình duyệt.",
        ["Paste the .ROBLOSECURITY cookie."] = "Hãy dán cookie .ROBLOSECURITY.",
        ["The cookie is invalid or has expired."] = "Cookie không hợp lệ hoặc đã hết hạn.",
        ["The new cookie is invalid or has expired."] = "Cookie mới không hợp lệ hoặc đã hết hạn.",
        ["Bloxkeep was locked."] = "Bloxkeep vừa bị khóa.",
        ["Invalid field."] = "Trường không hợp lệ.",
        ["Already checking — please wait."] = "Đang kiểm tra rồi, chờ chút nhé.",
        ["Checking sign-ins… {0}/{1}"] = "Đang kiểm tra đăng nhập… {0}/{1}",
        ["This account has no saved username."] = "Tài khoản này chưa có username.",
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
        ["Only {0} of RAM is free, and {1} more Roblox window(s) need about {2}. The PC may slow down or Roblox may crash."] =
            "Máy chỉ còn trống {0} RAM, trong khi mở thêm {1} cửa sổ Roblox cần khoảng {2}. Máy có thể bị lag hoặc Roblox bị văng.",
        ["Open anyway?"] = "Vẫn mở?",
        ["This file is too large to be an account list."] = "File này quá lớn, không phải danh sách tài khoản.",
        ["This file is still encrypted. Decrypt it first with RAM's official RAMDecrypt tool, then import the decrypted file."] =
            "File này vẫn đang được mã hóa. Hãy giải mã bằng công cụ RAMDecrypt chính thức của RAM trước, rồi nhập file đã giải mã.",
        ["This file is not an account list from Roblox Account Manager."] = "File này không phải danh sách tài khoản của Roblox Account Manager.",
        ["Choose the decrypted AccountData.json from Roblox Account Manager"] = "Chọn file AccountData.json (đã giải mã) của Roblox Account Manager",
        ["Imported accounts checked: {0} valid, {1} expired."] = "Đã kiểm tra tài khoản vừa nhập: {0} hợp lệ, {1} hết hạn.",
        ["{0} left the game — rejoining in {1}s…"] = "{0} đã thoát game — tự vào lại sau {1} giây…",
        ["{0} seems disconnected — restarting Roblox…"] = "{0} có vẻ đã mất kết nối — đang mở lại Roblox…",
        ["Auto-rejoin for {0} was turned off after {1} tries in 15 minutes."] = "Đã tắt tự vào lại game cho {0} sau {1} lần thử trong 15 phút.",
        ["Couldn't rejoin {0}: {1}"] = "Không vào lại được {0}: {1}",
        ["Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow."] =
            "Máy đang dùng {0}% RAM. Hãy đóng bớt cửa sổ Roblox nếu thấy lag.",
        ["Roblox refused memory trimming, so it was skipped."] = "Roblox không cho phép giảm RAM nên bước này được bỏ qua.",
        ["Already launching — wait a moment or press Stop."] = "Đang mở tài khoản khác — chờ chút hoặc bấm Dừng.",
        ["Select at least one account."] = "Hãy chọn ít nhất một tài khoản.",
        ["Reading the private server link…"] = "Đang đọc link server riêng…",
        ["{0}: already running — skipped."] = "{0}: đang chạy rồi — bỏ qua.",
        ["Launching {0} ({1}/{2})…"] = "Đang mở {0} ({1}/{2})…",
        ["Waiting {0}s before the next account…"] = "Chờ {0} giây trước tài khoản tiếp theo…",
        ["Waiting for Roblox to open {0}… (Roblox is updating to {1})"] = "Chờ Roblox mở {0}… (Roblox đang cập nhật lên {1})",
        ["Waiting for Roblox to open {0}… (Roblox may be updating)"] = "Chờ Roblox mở {0}… (Roblox có thể đang cập nhật)",
        ["{0} was kicked for being AFK. Anti-AFK doesn't work while the PC is locked."] = "{0} bị kick vì AFK. Chống AFK không chạy khi máy đang khóa.",
        ["Wait until Bloxkeep finishes opening Roblox."] = "Chờ Bloxkeep mở Roblox xong đã.",
        ["The PC woke up — auto-rejoin waits until the connection is back."] = "Máy vừa thức dậy — tự vào lại game sẽ chờ có mạng trở lại.",
        ["Couldn't turn it off: {0}"] = "Không tắt được: {0}",
        ["Sign in to Roblox"] = "Đăng nhập Roblox",
        ["Sign in to each account in turn. Press Done when finished."] = "Lần lượt đăng nhập từng acc. Xong thì bấm Xong.",
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
        ["Bloxkeep needs Microsoft Edge WebView2, which isn't installed. Install it now? (about 2 MB)"] = "Bloxkeep cần Microsoft Edge WebView2 nhưng máy chưa có. Cài ngay? (khoảng 2 MB)",
        ["The downloaded WebView2 installer is not signed by Microsoft, so it was not run."] =
            "Bộ cài WebView2 vừa tải không có chữ ký của Microsoft nên đã không chạy.",
        ["Couldn't install WebView2: {0}"] = "Không cài được WebView2: {0}",
    };
}
