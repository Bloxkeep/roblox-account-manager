# Bloxkeep

Quản lý nhiều tài khoản Roblox trên Windows. Tài khoản được mã hóa và **chỉ lưu trên máy của bạn**; chọn game, chọn acc, bấm Play.

![Bloxkeep](docs/screenshots/2-restart-to-update.png)

- Thêm acc bằng điện thoại (mã / QR), trình duyệt, tên & mật khẩu, cookie, hoặc chuyển sang từ Roblox Account Manager
- Xem acc nào online, đang chơi game gì, mỗi cửa sổ Roblox tốn bao nhiêu RAM
- Tìm game theo tên, chọn server, cho nhiều acc vào cùng một server
- Đổi acc kiểu Steam ("Ai đang chơi?"), hoặc chạy nhiều Roblox cùng lúc với Multi-Roblox
- Tự vào lại game khi bị văng, xếp lưới cửa sổ, cảnh báo thiếu RAM
- Tự cập nhật ở nền

Giao diện tiếng Anh, có tiếng Việt (đổi ở màn hình khóa hoặc trong *Settings*).

## Tải về

Windows 10 trở lên, 64-bit. Không cần cài .NET.

Tải ở trang [Releases](https://github.com/minhhoang0122/bloxkeep/releases):

| File | Dùng khi |
|---|---|
| `Bloxkeep-Setup.exe` | Cài bình thường. Cài trong vài giây cho tài khoản Windows hiện tại (không cần quyền admin), tạo shortcut ở Start Menu và Desktop, tự cài Microsoft Edge WebView2 nếu máy thiếu. |
| `Bloxkeep-Portable.zip` | Không muốn cài. Giải nén rồi chạy `Bloxkeep.exe`; dữ liệu nằm trong thư mục `data` bên cạnh. |

File chưa có chữ ký số, nên lần đầu chạy Windows SmartScreen sẽ cảnh báo: bấm *More info › Run anyway*. Có thể đối chiếu mã SHA-256 với `SHA256SUMS.txt` trong cùng bản phát hành. Các bản cập nhật app tự tải sẽ không bị cảnh báo lại.

**Cập nhật**: app tự kiểm tra trên GitHub vài lần mỗi ngày và tải bản mới ở nền (thường chỉ vài trăm KB). Khi xong, thanh bên hiện nút **Restart to update**; không bấm thì bản mới được cài lúc tắt app. Sau khi cập nhật, app hiện bảng *What's new*. Tắt tự cập nhật trong *Settings › Updates*.

**Gỡ**: *Windows Settings › Apps › Bloxkeep › Uninstall* (app đang mở sẽ tự đóng). Tài khoản đã lưu **được giữ lại** trong `%LOCALAPPDATA%\Bloxkeep` để cài lại là dùng tiếp; muốn xóa hẳn thì xóa thư mục đó.

## Bắt đầu

1. **Đặt mật khẩu chủ** ở lần mở đầu tiên. Quên mật khẩu này thì không khôi phục được dữ liệu.
2. **Thêm tài khoản**, chọn một cách:
   - **Dùng điện thoại**: mở app Roblox trên điện thoại, vào *Settings › Quick Log In*, nhập mã hoặc quét QR app hiện ra rồi xác nhận. Thêm xong acc này thì có mã mới cho acc tiếp theo.
   - **Dùng trình duyệt**: cửa sổ đăng nhập roblox.com chính thức mở ra; đăng nhập xong app lấy phiên đăng nhập rồi xóa sạch phiên trình duyệt tạm.
   - **Tên & mật khẩu**: mật khẩu chỉ gửi tới máy chủ đăng nhập của Roblox, không được lưu. Hỗ trợ xác minh 2 bước (app xác thực hoặc email). Nếu Roblox đòi captcha, câu đố hiện ngay trong app để bạn tự giải; cách này không cần vào được roblox.com nên dùng được ở Việt Nam.
   - Cách khác: dán cookie `.ROBLOSECURITY`, hoặc chuyển sang từ Roblox Account Manager (xem bên dưới).
3. **Vào game**: tìm game theo tên hoặc dán link game / Place ID / link server riêng vào thanh dưới, bấm chọn các acc (Shift để chọn liền nhau), bấm **Play**.

Phím tắt: `Ctrl+F` tìm, `Ctrl+A` chọn tất cả, `Enter` vào game, `F2` sửa, `Delete` xóa, `Esc` bỏ chọn. Chuột phải vào acc để xem thêm thao tác.

## Tính năng

**Danh sách acc**
- Xem dạng thẻ hoặc dạng danh sách; đặt tên gợi nhớ, nhóm và ghi chú cho từng acc; lọc theo trạng thái hoặc nhóm.
- Trạng thái tự cập nhật mỗi 30 giây: online, offline, đang chơi game gì (trên máy này hay thiết bị khác), bị kick vì lý do gì (AFK, mất mạng, đăng nhập ở nơi khác…).
- Acc hết phiên đăng nhập được đánh dấu *Signed out*, bấm *Sign in* để đăng nhập lại. *Check sign-in* kiểm tra hàng loạt.
- Sao chép mật khẩu/cookie: không vào lịch sử clipboard và tự xóa sau 30 giây.

**Chọn game và server**
- Ô tìm game có các mục: kết quả tìm theo tên, *Recent*, *Pinned*, game yêu thích của các acc trên Roblox, và game phổ biến.
- Nút *Servers* liệt kê server còn chỗ (sắp theo đông/vắng, kèm ping). Chọn một server, hoặc dùng *Join this server* trên một acc đang chơi để cho các acc khác vào cùng.
- Ô *Job ID* để vào đúng một server cụ thể.

**Ai đang chơi? (đổi acc)**
Màn hình chọn acc kiểu Steam, mở từ *Switch account* ở thanh bên (có thể bật hiện mỗi lần mở khóa app). Bấm một acc: app đóng Roblox đang chạy (hỏi trước nếu đang trong game) rồi mở Roblox bằng acc đó.

**Tự vào lại game**
Bật cho từng acc trong menu ⋯. Khi Roblox của acc đó bị văng, bị kick hoặc mất kết nối, app vào lại đúng game đó sau 10 giây. Quá 5 lần trong 15 phút thì tự tắt để tránh lặp. Bị đăng nhập ở thiết bị khác thì không vào lại. Tự tay đóng cửa sổ Roblox cũng tính là thoát game, nên hãy dùng nút *Close* trong app.

**Cấu hình Roblox theo acc**
*Settings › Roblox settings* đặt chất lượng đồ họa, giới hạn FPS và âm lượng cho mọi acc hoặc cho các acc được chọn. App ghi các giá trị này vào file cài đặt của Roblox ngay trước khi mở acc, rồi trả lại cài đặt của bạn khi Roblox đã chạy.

**RAM và cửa sổ**
- Thanh bên hiện số cửa sổ Roblox, RAM chúng dùng và RAM còn trống của máy.
- Cảnh báo trước khi mở thêm acc vượt quá RAM trống, và khi máy dùng trên 92% RAM.
- *Save RAM on background windows* (mặc định tắt): khi máy sắp hết RAM, các cửa sổ chạy nền nhường RAM cho cửa sổ bạn đang chơi.
- Nút ⊞ xếp tất cả cửa sổ Roblox thành lưới trên một màn hình. Tên acc được ghi lên tiêu đề cửa sổ để dễ phân biệt trên taskbar.

**Chống AFK** (mặc định tắt)
Roblox kick acc sau 20 phút không thao tác. Khi bật, lúc bạn không dùng máy (ít nhất 1 phút), cứ khoảng 15 phút app đưa từng cửa sổ Roblox lên trước một chút, nhích chuột vài pixel rồi trả cửa sổ và con trỏ về như cũ. Không bấm phím, không click. Dừng ngay khi bạn chạm vào máy. Không chạy khi máy đang khóa.

**Nhập từ Roblox Account Manager (RAM)**
*Add account ▾ › Move over from Roblox Account Manager*. RAM mã hóa file `AccountData.json`, nên hãy giải mã trước bằng công cụ chính thức [RAMDecrypt](https://github.com/ic3w0lf22/RAMDecrypt) rồi chọn file đã giải mã. Acc trùng được cập nhật cookie; cookie được kiểm tra ngay sau khi nhập.

## Multi-Roblox

**Mặc định tắt.** Roblox không chính thức hỗ trợ chạy nhiều client cùng lúc và có thể coi việc này là đáng ngờ. Bật trong *Settings* nếu bạn chấp nhận rủi ro, nhất là với acc quan trọng.

Cách hoạt động: app giữ trước hai khóa "chỉ một cửa sổ" của Roblox (`ROBLOX_singletonMutex`, `ROBLOX_singletonEvent`), nên các cửa sổ mở sau không đóng nhau. App không can thiệp vào tiến trình Roblox.

- Bloxkeep phải mở **trước** Roblox và để mở trong lúc chơi. Nếu đèn Multi-Roblox màu vàng: đóng hết Roblox (kể cả Roblox ở khay hệ thống), app sẽ tự bật lại.
- Nếu Roblox được đặt tự chạy cùng Windows, app sẽ đề nghị tắt (giống tắt trong *Task Manager › Startup apps*).
- Nếu Bloxstrap/Fishstrap chặn mở nhiều cửa sổ: chọn *RobloxPlayerBeta directly* trong *Settings › Open Roblox with*.
- Multi-Roblox áp dụng cho việc **vào game**. Màn hình chính của Roblox (mở mà không vào game) chỉ chạy được một cửa sổ, nên để dùng nhiều acc ở màn hình chính hãy dùng *Ai đang chơi?* để đổi qua lại.

## Bản Roblox được hỗ trợ

*Settings › Open Roblox with* chọn bản dùng để mở acc; app tự phát hiện bản nào đã cài.

| Lựa chọn | Ghi chú |
|---|---|
| Mặc định | Dùng trình đang mở link Roblox trên máy (Bloxstrap, Fishstrap hoặc Roblox) |
| Bloxstrap / Fishstrap | Giữ nguyên mod và FFlag của bạn |
| Roblox (chính thức) | Bản tải từ roblox.com |
| RobloxPlayerBeta directly | Bỏ qua mọi trình mở; thử khi trình mở chặn chạy nhiều cửa sổ |
| Roblox Microsoft Store | **Không hỗ trợ**: bản này tự đăng nhập bên trong, không mở được acc đã chọn |

**Roblox VNG**: thêm được acc VNG bằng cách đăng nhập qua điện thoại (tab *Roblox VNG*), nhưng chưa mở được game cho acc VNG.

## Dữ liệu và bảo mật

- Kho tài khoản: `%LOCALAPPDATA%\Bloxkeep\vault.rmv`, mã hóa AES-256-GCM với khóa sinh từ mật khẩu chủ (PBKDF2-SHA256, 600.000 vòng). Sửa file trái phép sẽ bị phát hiện. Chương trình được cài ở thư mục riêng (`%LOCALAPPDATA%\BloxkeepApp`), nên gỡ app không đụng tới kho.
- Bản portable lưu dữ liệu trong thư mục `data` cạnh `Bloxkeep.exe`.
- App chỉ kết nối tới:
  - máy chủ của Roblox: đăng nhập, vào game, trạng thái, ảnh;
  - GitHub: kiểm tra và tải bản cập nhật, không gửi dữ liệu tài khoản (tắt được);
  - Microsoft: chỉ khi phải cài WebView2.
- Giao diện bị khóa bằng Content Security Policy, không tự gửi request ra ngoài.
- Quên mật khẩu chủ: xóa `vault.rmv` (mất toàn bộ acc đã lưu) rồi tạo kho mới.
- Lỗi bất ngờ được ghi vào `error.log` trong thư mục dữ liệu, không chứa thông tin tài khoản.

## Tình trạng

Bloxkeep đang ở bản đầu tiên. Những phần sau **chưa được thử với tài khoản thật**; nếu gặp lỗi, hãy mở issue kèm nội dung lỗi:

- Vào game và Multi-Roblox với nhiều acc thật (Roblox đổi cách cấp vé đăng nhập và cách chặn nhiều cửa sổ từ tháng 9/2026; app đã làm theo cách mới).
- Tự vào lại game và chống AFK trong thời gian dài.
- Bước đăng nhập cuối của cách *Dùng điện thoại*, đăng nhập thành công bằng mật khẩu, xác minh 2 bước và giải captcha trong app.
- Chạy trên Windows 10.

## Phát triển

Cần .NET 10 SDK. App là C# (.NET 10, WinForms) làm khung, giao diện HTML/CSS/JS chạy trong WebView2. Công cụ đóng gói [Velopack](https://velopack.io) (`vpk`) được khai báo trong `dotnet-tools.json` và tự tải.

```bat
build.bat 1.0.1
```

Build vào `dist\releases`: bộ cài, zip portable, gói cập nhật và `SHA256SUMS.txt`. Tham số thứ hai là link repo GitHub dùng cho tự cập nhật (mặc định repo này; truyền repo khác khi build bản fork). Tự cập nhật chỉ hoạt động khi repo để public.

```bat
release.bat 1.0.1 https://github.com/minhhoang0122/bloxkeep
```

Đăng lên GitHub Releases, cần biến môi trường `GITHUB_TOKEN` có quyền ghi release. Mỗi lần phát hành phải tăng số phiên bản và thêm ghi chú cho phiên bản đó vào `RobloxManager\whatsnew.json` (tiếng Anh và tiếng Việt); ghi chú này hiện trong app sau khi cập nhật và cũng là nội dung trang phát hành.

**Xem thử giao diện không cần build**: mở `dev/preview.html` qua một web server tĩnh ở thư mục gốc repo (ví dụ `python -m http.server 5178`, rồi vào `http://127.0.0.1:5178/dev/preview.html`). Trang này dùng dữ liệu giả trong `dev/mock.js`.

### Cấu trúc

```
RobloxManager/             mã nguồn
  wwwroot/                 giao diện (nhúng vào exe); chuỗi EN/VI trong i18n.js
  AppController.cs         nhận lệnh từ giao diện, mở game, theo dõi tiến trình, tự vào lại game
  RobloxApi.cs             gọi API Roblox (đăng nhập, trạng thái, game, server)
  RobloxLauncher.cs        mở Roblox qua launcher đã chọn
  RobloxLogWatcher.cs      đọc log của Roblox để biết vào game / bị kick
  MultiInstance.cs         Multi-Roblox
  Vault.cs                 kho mã hóa
  AppUpdater.cs            tự cập nhật (Velopack)
  whatsnew.json            ghi chú từng phiên bản
Installer/                 ảnh màn hình chờ của bộ cài
dev/                       xem thử giao diện với dữ liệu giả (không đóng gói)
docs/screenshots/          ảnh chụp màn hình
```

## Giấy phép

[MIT](LICENSE). Bloxkeep không liên kết với Roblox Corporation.
