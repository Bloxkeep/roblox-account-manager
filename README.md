# Bloxkeep

Quản lý nhiều tài khoản Roblox, mã hóa và lưu **chỉ trên máy này**, mở nhiều Roblox cùng lúc.

## Cài đặt, cập nhật, gỡ

- **Cài**: chạy `Bloxkeep-Setup.exe` — cài trong vài giây cho tài khoản Windows hiện tại (không cần quyền admin), tạo shortcut ở Start Menu và Desktop. Máy thiếu Microsoft Edge WebView2 thì bộ cài tự cài. Không cần cài .NET. Windows 10 trở lên, 64-bit.
- **Không muốn cài**: giải nén `Bloxkeep-Portable.zip` rồi chạy `Bloxkeep.exe`; dữ liệu nằm trong thư mục `data` bên cạnh.
- **Cập nhật**: app tự xem trên GitHub vài lần mỗi ngày, tải bản mới ở nền (thường chỉ vài trăm KB nhờ chỉ tải phần thay đổi) rồi hiện nút **Restart to update** kèm số phiên bản; mở lại app sẽ thấy bảng *What's new* ghi những gì đã đổi. Không bấm thì bản mới được cài khi tắt app. Tắt tự kiểm tra trong *Settings › Updates*.
- **Gỡ**: Windows *Settings › Apps › Bloxkeep › Uninstall*. Nếu app đang mở, nó tự đóng trước. Tài khoản đã lưu **được giữ lại** trong `%LOCALAPPDATA%\Bloxkeep` (cài lại là dùng tiếp); muốn xóa hẳn thì xóa thư mục đó.
- File cài chưa có chữ ký số nên lần đầu chạy Windows SmartScreen sẽ cảnh báo: bấm *More info › Run anyway*. Có thể đối chiếu mã SHA-256 trong `SHA256SUMS.txt`. Các bản cập nhật do app tự tải không bị cảnh báo lại.

## Dùng

1. **Lần đầu**: đặt mật khẩu chủ. Quên mật khẩu này thì không khôi phục được dữ liệu.
2. **Thêm tài khoản**: bấm *Add account*. Một cửa sổ đăng nhập Roblox chính thức mở ra; đăng nhập xong app tự lấy phiên đăng nhập và xóa sạch phiên trình duyệt tạm. Hoặc chọn mũi tên ▾ để dán cookie `.ROBLOSECURITY`.
3. **Vào game**: dán link game / Place ID / link server riêng vào thanh dưới, bấm chọn các thẻ tài khoản (Shift để chọn liền nhau), bấm **Play**.
4. Mỗi thẻ hiển thị: online / offline / đang chơi game gì (tự cập nhật mỗi 30 giây), RAM và thời gian chơi của cửa sổ Roblox trên máy này.

Phím tắt: `Ctrl+F` tìm, `Ctrl+A` chọn tất cả, `Enter` vào game, `F2` sửa, `Delete` xóa, `Esc` bỏ chọn. Chuột phải vào thẻ để xem thêm.

Ngôn ngữ mặc định là tiếng Anh; đổi sang tiếng Việt ở màn hình khóa (EN / VI) hoặc trong *Settings*.

## Tính năng thêm

- **Nhập từ Roblox Account Manager (RAM)**: *Add account ▾ › Import from Roblox Account Manager*. RAM lưu file `AccountData.json` ở dạng mã hóa, nên hãy giải mã trước bằng công cụ chính thức [RAMDecrypt](https://github.com/ic3w0lf22/RAMDecrypt), rồi chọn file đã giải mã. Tài khoản trùng sẽ được cập nhật cookie; cookie được kiểm tra ngay sau khi nhập.
- **Tự vào lại game**: bật cho từng tài khoản trong menu ⋯ của thẻ (biểu tượng ⇄ trên thẻ). Khi Roblox của tài khoản đó bị văng, bị kick hoặc mất kết nối (app thấy cửa sổ còn mở nhưng tài khoản không còn trong game), app vào lại đúng game đó sau 10 giây. Quá 5 lần trong 15 phút thì app tự tắt để tránh lặp. Tự tay đóng cửa sổ Roblox cũng tính là thoát game — dùng nút *Close* trong app để dừng.
- **Bảo vệ tài nguyên**: thanh bên hiện RAM máy (phần của Roblox + phần còn lại). App cảnh báo trước khi mở thêm tài khoản vượt quá RAM trống và khi máy dùng trên 92% RAM. Tùy chọn *Trim RAM of background Roblox windows* (mặc định tắt) dọn bớt RAM của các cửa sổ Roblox chạy nền mỗi phút.
- **Xếp lưới cửa sổ**: nút ⊞ ở thanh dưới xếp tất cả cửa sổ Roblox thành lưới trên một màn hình (có nhiều màn hình thì chọn màn hình).

## Bản Roblox được hỗ trợ

*Settings › Roblox client* chọn bản dùng để mở tài khoản; app tự phát hiện bản nào đã cài:

| Lựa chọn | Ghi chú |
|---|---|
| Mặc định của Windows | Đi qua trình đang mở link Roblox trên máy (Bloxstrap, Fishstrap hoặc Roblox) |
| Bloxstrap / Fishstrap | Giữ nguyên mod và FFlag của bạn |
| Roblox (chính thức) | Bản tải từ roblox.com |
| Chạy thẳng RobloxPlayerBeta | Bỏ qua mọi trình mở — thử khi trình mở chặn chạy nhiều cửa sổ |
| Roblox Microsoft Store | **Không hỗ trợ**: bản này tự đăng nhập bên trong, không mở được tài khoản đã chọn hay nhiều cửa sổ |

## Multi-Roblox

**Mặc định tắt.** Roblox không chính thức hỗ trợ chạy nhiều client, và đội chống gian lận của họ từng nói việc này có thể bị coi là đáng ngờ — bật trong *Settings* nếu chấp nhận rủi ro.

App giữ trước 2 khóa "chỉ 1 cửa sổ" của Roblox (`ROBLOX_singletonMutex`, `ROBLOX_singletonEvent`) nên các cửa sổ Roblox mở sau không đóng nhau. Không can thiệp vào tiến trình Roblox.

- Phải bật app **trước** khi mở Roblox. Nếu đèn Multi-Roblox màu vàng: đóng hết Roblox (kể cả Roblox chạy ngầm ở khay hệ thống) — app tự kích hoạt lại.
- Nếu Roblox được đặt tự chạy cùng Windows (`--launch-to-tray`), app sẽ đề nghị tắt (giống tắt trong Task Manager › Startup apps).
- Nếu Bloxstrap/Fishstrap chặn mở nhiều cửa sổ: chọn *Chạy thẳng RobloxPlayerBeta* trong *Settings › Roblox client*.

## Dữ liệu & bảo mật

- Kho: `%LOCALAPPDATA%\Bloxkeep\vault.rmv` — AES-256-GCM, khóa từ mật khẩu chủ (PBKDF2-SHA256, 600.000 vòng). Sửa file trái phép sẽ bị phát hiện. Chương trình cài ở thư mục riêng (`%LOCALAPPDATA%\BloxkeepApp`), nên gỡ app không đụng tới kho.
- **Portable**: bản zip lưu dữ liệu trong thư mục `data` bên cạnh; hoặc tạo thư mục `data` cạnh file exe.
- App chỉ kết nối tới máy chủ chính thức của Roblox (đăng nhập, vào game, trạng thái, ảnh), GitHub (kiểm tra và tải bản cập nhật — không gửi dữ liệu tài khoản, tắt được) và Microsoft (chỉ khi phải cài WebView2). Giao diện bị khóa CSP, không tự kết nối mạng.
- Mật khẩu/cookie khi sao chép không vào lịch sử clipboard và tự xóa sau 30 giây.
- Quên mật khẩu chủ: xóa `vault.rmv` (mất toàn bộ tài khoản đã lưu) rồi tạo kho mới.
- Lỗi bất ngờ được ghi vào `error.log` cùng thư mục (không chứa dữ liệu tài khoản).

## Lưu ý

- Tháng 9/2026 Roblox đổi cách cấp vé đăng nhập (`client-assertion`) và cách chặn nhiều cửa sổ; app đã làm theo cách mới nhưng **chưa được thử với tài khoản thật**. Nếu vào game báo lỗi, gửi lại nội dung lỗi để sửa.
- Roblox không chính thức hỗ trợ chạy nhiều cửa sổ; tự cân nhắc rủi ro với tài khoản quan trọng.

## Build và phát hành

Cần .NET 10 SDK. Công cụ đóng gói (Velopack `vpk`) được khai báo trong `dotnet-tools.json` và tự tải.

```
build.bat 1.0.1 https://github.com/<owner>/<repo>    build vào dist
eleases (bộ cài, zip portable, gói cập nhật)
release.bat 1.0.1 https://github.com/<owner>/<repo>  đăng lên GitHub Releases (cần biến GITHUB_TOKEN)
```

Build không kèm link repo thì bản đó không bao giờ tự kiểm tra cập nhật. Mỗi lần phát hành phải tăng số phiên bản và viết ghi chú cho phiên bản đó trong `RobloxManager\whatsnew.json` (tiếng Anh + tiếng Việt) — app hiện ghi chú này sau khi cập nhật, và nó cũng là nội dung trang phát hành trên GitHub.

## Cấu trúc

```
RobloxManager/          mã nguồn (C# .NET 10 + WebView2)
  wwwroot/              giao diện HTML/CSS/JS (nhúng vào exe)
  AppController.cs      xử lý lệnh từ giao diện, mở game, theo dõi tiến trình
  AppUpdater.cs         tự cập nhật từ GitHub Releases (Velopack)
  Vault.cs              kho mã hóa
  RobloxApi.cs          gọi API Roblox
  MultiInstance.cs      Multi-Roblox
Installer/              ảnh màn hình chờ của bộ cài
dev/                    bản xem thử giao diện với dữ liệu giả (không đóng gói)
dist/releases/          kết quả build
```
