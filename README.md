<p align="center">
  <img src="RobloxManager/wwwroot/logo.svg" width="88" alt="Logo Bloxkeep">
</p>
<h1 align="center">Bloxkeep</h1>
<p align="center">Quản lý và mở nhiều tài khoản Roblox trên Windows. Tài khoản được mã hóa và chỉ lưu trên máy của bạn.</p>
<p align="center">Tiếng Việt | <a href="README.en.md">English</a></p>
<p align="center"><img src="https://img.shields.io/badge/Windows-10%20%7C%2011%20(64--bit)-0078D4" alt="Windows 10 hoặc 11, 64-bit"> <img src="https://img.shields.io/badge/.NET-10-512BD4" alt=".NET 10"> <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="Giấy phép MIT"></a></p>
<p align="center"><a href="#tải-về-và-cài-đặt">Tải về</a> · <a href="#bắt-đầu">Bắt đầu</a> · <a href="#dữ-liệu-và-bảo-mật">Bảo mật</a> · <a href="#câu-hỏi-thường-gặp">Câu hỏi thường gặp</a></p>

<p align="center"><img src="docs/screenshots/hero.png" width="860" alt="Cửa sổ chính của Bloxkeep: danh sách tài khoản dạng thẻ, thanh chọn game ở dưới và Multi-Roblox đang bật"></p>

Bloxkeep dành cho người chơi nhiều tài khoản Roblox trên một máy Windows: giữ các tài khoản ở một chỗ, chọn game, chọn tài khoản rồi bấm **Vào game**. Dữ liệu chỉ nằm trên máy của bạn, được mã hóa bằng mật khẩu chủ; Bloxkeep không có máy chủ riêng.

*Bloxkeep là dự án độc lập, không liên kết, không được Roblox Corporation bảo trợ hay tài trợ. Roblox là thương hiệu của Roblox Corporation.*

## Tính năng

- Thêm tài khoản theo năm cách: tên và mật khẩu (có xác minh 2 bước, captcha giải trong một cửa sổ nhỏ), mã hoặc QR quét bằng app Roblox trên điện thoại, trang roblox.com trong cửa sổ trình duyệt, cookie, hoặc chuyển từ Roblox Account Manager.
- Xem trạng thái từng tài khoản: online, đang chơi trên máy này hay thiết bị khác, bị văng vì lý do gì, RAM và thời gian đã chơi, hay đã hết phiên đăng nhập. Tìm, lọc, sắp xếp, xem dạng thẻ hoặc dạng danh sách.
- Chọn game theo tên, link hoặc Place ID, hoặc từ các mục Gần đây, Đã ghim, Yêu thích trên Roblox và Phổ biến. Chọn server (đông nhất, vắng nhất, ping thấp nhất, đủ chỗ cho N tài khoản, ngẫu nhiên) hoặc vào cùng bạn bè, để nhiều tài khoản vào chung một server.
- Chạy nhiều cửa sổ Roblox cùng lúc với Multi-Roblox. Tính năng này tắt sẵn, bạn tự bật khi cần.
- Lưu bộ khởi chạy gồm các tài khoản, game và cách chọn server, rồi chạy lại chỉ với một lần bấm.
- Tự vào lại game cho từng tài khoản khi Roblox bị văng, bị kick hoặc mất kết nối.
- Đặt đồ họa, giới hạn FPS và âm lượng Roblox riêng cho từng tài khoản.
- Theo dõi RAM: cảnh báo khi máy sắp hết RAM, cho cửa sổ chạy nền nhường RAM, xếp các cửa sổ Roblox thành lưới, ghi tên tài khoản lên tiêu đề cửa sổ.
- Sao lưu vào thư mục OneDrive hoặc Google Drive. Ngoài ra còn có 32 chủ đề, giao diện tiếng Anh và tiếng Việt, chạy ngầm ở khay hệ thống và thông báo Windows.

<img src="docs/screenshots/themes.png" width="800" alt="Sáu chủ đề của Bloxkeep: Aurora, Sunset, Sakura, Paper, Terminal và Dark">

## Tải về và cài đặt

Cần Windows 10 hoặc 11, bản 64-bit. Không cần cài .NET. Bộ cài tự cài Microsoft Edge WebView2 nếu máy chưa có.

Tải ở trang [Releases](../../releases/latest):

| File | Dùng khi |
|---|---|
| `Bloxkeep-Setup.exe` | Cài bình thường. Cài cho tài khoản Windows hiện tại, không cần quyền admin, tạo shortcut ở Start Menu và Desktop. |
| `Bloxkeep-Portable.zip` | Không muốn cài. Giải nén rồi chạy `Bloxkeep.exe`; dữ liệu nằm trong thư mục `data` bên cạnh. Bản portable cũng tự cập nhật. |

> [!CAUTION]
> Chỉ tải Bloxkeep ở trang Releases của repo này. Bản tải ở nơi khác có thể đã bị sửa để lấy cắp tài khoản.

File chưa có chữ ký số, nên lần đầu chạy Windows SmartScreen sẽ cảnh báo. Bấm **More info** rồi **Run anyway** (trên Windows tiếng Việt, hai nút này mang tên tiếng Việt tương ứng). Bạn có thể đối chiếu mã SHA-256 của file với `SHA256SUMS.txt` trong cùng bản phát hành.

<img src="docs/screenshots/1-install.png" width="480" alt="Bộ cài Bloxkeep đang cài đặt">

### Cập nhật

Bloxkeep tự kiểm tra bản mới trên GitHub và tải về ở nền. Tải xong, thanh bên hiện nút **Khởi động lại để cập nhật**; Roblox vẫn chạy bình thường trong lúc đó. Nếu bạn không bấm, bản mới được cài khi tắt app. Sau khi cập nhật, app hiện bảng **Có gì mới trong Bloxkeep**.

Tự cập nhật chỉ hoạt động khi repo trên GitHub ở chế độ công khai.

Muốn tắt tự cập nhật: bỏ chọn **Cài đặt › Cập nhật › Tự động cập nhật**. Nút **Kiểm tra ngay** vẫn dùng được.

Chạy `Bloxkeep-Setup.exe` bản mới hơn đè lên bản đã cài cũng được: bộ cài hỏi cập nhật (hoặc sửa lại nếu cùng phiên bản) và giữ nguyên dữ liệu.

### Gỡ cài đặt

Nếu Bloxkeep đang chạy, hãy thoát trước (chuột phải biểu tượng ở khay › **Thoát Bloxkeep**): trình gỡ sẽ tắt app ngay mà không hỏi, và biểu tượng của app vừa bị tắt có thể còn nằm ở khay cho đến khi bạn rê chuột qua. Sau đó vào **Settings › Apps › Installed apps** của Windows, tìm **Bloxkeep** (publisher `minhhoang0122`) và chọn **Uninstall**. Hộp thoại của bộ cài và trình gỡ chỉ có tiếng Anh.

| Bị xóa | Được giữ |
|---|---|
| Chương trình trong `%LOCALAPPDATA%\BloxkeepApp`, shortcut ở Start menu và Desktop (kể cả shortcut bạn đã ghim vào Start hay thanh tác vụ), bộ nhớ đệm của giao diện, và mục Bloxkeep trong Installed apps. Bloxkeep không thêm gì khác vào Windows: không có mục khởi động cùng Windows, không có service, không chiếm giao thức `roblox-player:`. | Tài khoản và cài đặt trong `%LOCALAPPDATA%\Bloxkeep` (`vault.rmv`, `prefs.json`, `error.log`), nên cài lại là dùng tiếp; và các file sao lưu trong thư mục sao lưu. Muốn xóa cả những thứ này thì tự xóa thư mục đó và các file sao lưu. |

<details>
<summary>Ảnh: cập nhật và gỡ cài đặt</summary>

<img src="docs/screenshots/2-restart-to-update.png" width="800" alt="Nút Khởi động lại để cập nhật ở thanh bên"> <img src="docs/screenshots/6-whats-new.png" width="800" alt="Bảng Có gì mới sau khi cập nhật"> <img src="docs/screenshots/3-settings-updates.png" width="800" alt="Mục Cập nhật trong Cài đặt">
<img src="docs/screenshots/4-uninstall-progress.png" width="480" alt="Đang gỡ Bloxkeep"> <img src="docs/screenshots/5-uninstall-done.png" width="480" alt="Gỡ xong, dữ liệu tài khoản được giữ lại">
</details>

## Bắt đầu

1. **Đặt mật khẩu chủ** ở lần mở đầu tiên (ít nhất 6 ký tự). Mật khẩu này dùng để mở khóa Bloxkeep và **không khôi phục được** nếu quên, xem [Dữ liệu và bảo mật](#dữ-liệu-và-bảo-mật). Giao diện mặc định là tiếng Anh: bấm nút **VI** ở góc trên màn hình này để chuyển sang tiếng Việt. Nếu đang chuyển sang máy mới, bấm **Khôi phục từ bản sao lưu** thay vì tạo mới, xem [Sao lưu và khôi phục](#sao-lưu-và-khôi-phục).

   <img src="docs/screenshots/lock.png" width="640" alt="Màn hình mở khóa với nút đổi ngôn ngữ EN/VI ở góc trên">

2. **Thêm tài khoản**: bấm nút **Thêm tài khoản** (hoặc chọn một cách ngay trên danh sách trống). Chọn một cách:
   - **Dùng tài khoản & mật khẩu** (nhập ngay trong hộp thoại **Đăng nhập Roblox**): mật khẩu chỉ gửi tới máy chủ đăng nhập của Roblox. Đăng nhập bằng mật khẩu không lưu mật khẩu đó. Có **Xác minh 2 bước** (**App xác thực** hoặc **Email**). Nếu Roblox đòi captcha, bấm **Giải câu đố** và làm trong cửa sổ nhỏ hiện ra.
   - **Đăng nhập bằng điện thoại**: Bloxkeep hiện mã và QR. Trên điện thoại, mở **app Roblox**, vào **Cài đặt › Đăng nhập nhanh**, nhập mã hoặc quét QR rồi xác nhận. Thêm xong một tài khoản thì có mã mới cho tài khoản tiếp theo.
   - **Đăng nhập bằng trình duyệt**: trang đăng nhập roblox.com chính thức mở trong một cửa sổ tạm. Đăng nhập xong, Bloxkeep lấy phiên đăng nhập rồi xóa cửa sổ tạm đó. Bloxkeep không đọc mật khẩu bạn gõ ở đây.
   - **Dùng cookie** hoặc **Chuyển từ Roblox Account Manager**: xem [Chuyển từ Roblox Account Manager](#chuyển-từ-roblox-account-manager).

   <img src="docs/screenshots/add-account.png" width="700" alt="Hộp thoại Đăng nhập Roblox với tab Roblox và Roblox VNG, cùng các cách đăng nhập khác">

3. **Chọn game, chọn tài khoản, bấm Vào game.** Gõ tên game hoặc dán link / Place ID vào ô ở thanh dưới, bấm vào các thẻ tài khoản để chọn (giữ `Shift` khi bấm để chọn liền một dãy), rồi bấm **Vào game**. Để trống ô game thì nút đổi thành **Mở Roblox**: mỗi tài khoản đã chọn mở Roblox ở trang chủ.

   <img src="docs/screenshots/demo.gif" width="800" alt="Chọn game, chọn hai tài khoản rồi bấm Vào game">

4. Phím tắt: `Ctrl+F` tìm, `Ctrl+A` chọn tất cả, `Enter` vào game, `F2` sửa, `Delete` xóa, `Esc` bỏ chọn. Chuột phải vào một tài khoản để xem thêm thao tác.

## Sử dụng

### Danh sách tài khoản

- Lọc theo **Tất cả**, **Đang chơi**, **Online**, **Offline**, **Trên máy này**, **Cần đăng nhập lại**; sắp xếp theo **Tên**, **Trạng thái** hoặc **Chơi gần đây**; tìm theo tên, nhóm, ghi chú. Chuyển giữa **Dạng thẻ** và **Dạng danh sách**.
- Trạng thái tự làm mới mỗi 30 giây. Khi bị văng, thẻ ghi rõ lý do đọc từ log của Roblox: **Bị kick vì AFK**, **Bị game kick**, **Acc đăng nhập ở thiết bị khác**, **Rớt mạng**, **Không vào được server**.
- Tài khoản hết phiên được đánh dấu **Hết phiên đăng nhập**; bấm **Đăng nhập** để đăng nhập lại. **Kiểm tra đăng nhập** có trong menu ⋯ của từng tài khoản, và **Kiểm tra đăng nhập của tất cả tài khoản** ở menu ⋯ phía trên danh sách.
- **Sửa…** (`F2`) để đặt **Tên gợi nhớ**, **Nhóm**, **Ghi chú**. Tại đây bạn có thể tự lưu **Mật khẩu Roblox** nếu muốn (không bắt buộc); mật khẩu nằm trong kho đã mã hóa.
- **Sao chép mật khẩu** và **Sao chép cookie** không vào lịch sử clipboard của Windows và tự xóa sau 30 giây.

<img src="docs/screenshots/accounts-list.png" width="800" alt="Danh sách tài khoản dạng danh sách với trạng thái từng tài khoản">

### Chọn game và server

- Ô game nhận tên game, Place ID, link `roblox.com/games/…` (kể cả link server riêng), link chia sẻ server và link `roblox://`.
- Bảng chọn game có các tab **Gần đây**, **Đã ghim**, **Yêu thích trên Roblox**, **Phổ biến**. Bấm ☆ để ghim game.
- **Job ID (tuỳ chọn)**: vào đúng một server, để nhiều tài khoản vào chung.
- Nút **Server** liệt kê server công khai còn chỗ: sắp theo **Đông nhất**, **Vắng nhất**, **Ping thấp nhất**, **FPS cao nhất**; lọc **Đủ chỗ cho N acc**; hoặc bấm **Server ngẫu nhiên**. Mục **Bạn bè trong game này** cho bạn **Vào cùng** bạn bè của các tài khoản (nếu cài đặt riêng tư của họ cho phép). Ping hiển thị là ping trung bình của người chơi trong server, không phải ping từ máy bạn.
- Trên một tài khoản đang chơi, chọn **Vào cùng server** để các tài khoản khác vào đúng server đó.

<img src="docs/screenshots/game-picker.png" width="800" alt="Bảng chọn game với các tab Gần đây, Đã ghim, Yêu thích trên Roblox, Phổ biến">

<img src="docs/screenshots/servers.png" width="700" alt="Danh sách server của một game, có mục bạn bè đang chơi và bộ lọc đủ chỗ cho 2 tài khoản">

### Bộ khởi chạy

Một bộ khởi chạy lưu tên, danh sách tài khoản (theo thứ tự), game, cách chọn server và giãn cách. Ô **Game** chọn như ở thanh dưới: chọn từ các tab **Gần đây**, **Đã ghim**, **Yêu thích trên Roblox**, **Phổ biến**, gõ tên để tìm, hoặc dán link / Place ID. Để trống thì chỉ mở **Màn hình chính Roblox**. Server có thể là **Bất kỳ (Roblox tự chọn)**, **Đông nhất**, **Vắng nhất**, **Ping thấp nhất**, **Ngẫu nhiên** hoặc **Server cố định (Job ID)**.

Mở nút **Bộ khởi chạy** ở thanh dưới để **Lưu thành bộ khởi chạy…**, **Sửa bộ khởi chạy…** hoặc **Chạy**. Lưu được tối đa 50 bộ.

Phần **Tùy chọn** của mỗi bộ áp dụng cho các cửa sổ Roblox mà bộ đó mở (kể cả khi chúng tự vào lại game). Bấm **Vào game** bình thường cho cùng tài khoản thì vẫn theo cài đặt chung.

- **Chống AFK**: **Theo Cài đặt**, **Bật** hoặc **Tắt**.
- **Tự vào lại game**: **Theo từng tài khoản**, **Bật** hoặc **Tắt**.
- **Hiệu năng**: **Treo máy nhẹ** chọn đồ họa thấp nhất, 30 FPS, tắt tiếng, và thu nhỏ cửa sổ khi đã vào game. Cửa sổ mở không có tùy chọn này vẫn giữ cài đặt Roblox của bạn.
- **Mở lại Roblox**: mỗi 2, 4, 6 hoặc 12 giờ, từng cửa sổ được đóng rồi mở lại vào đúng game, lần lượt từng cái. Cửa sổ bạn đang dùng sẽ được để sau.

<img src="docs/screenshots/presets.png" width="700" alt="Hộp thoại Bộ khởi chạy mới: tên, game, server, giãn cách và các tài khoản">

### Multi-Roblox

Multi-Roblox **tắt sẵn**. Roblox không chính thức hỗ trợ chạy nhiều client cùng lúc; bạn tự chịu rủi ro khi bật. Bật bằng công tắc Multi-Roblox ở thanh bên hoặc trong **Cài đặt › Mở Roblox**.

- Bloxkeep phải mở **trước** Roblox và để mở trong lúc chơi.
- Đèn trạng thái: **Đang tắt**, **Đang bật — mở được nhiều cửa sổ**, hoặc **Chưa hoạt động — bấm để xem cách sửa** (đèn vàng). Cách sửa: đóng hết Roblox, kể cả Roblox ở khay hệ thống; nếu Roblox tự chạy cùng Windows, bấm **Tắt tự khởi động** (giống tắt trong Task Manager › Startup apps, bật lại được ở đó). Sau đó đèn chuyển xanh.
- Nếu Bloxstrap hoặc Fishstrap chặn mở nhiều cửa sổ, chọn **Chạy thẳng RobloxPlayerBeta (bỏ qua trình mở)** trong **Cài đặt › Mở Roblox › Mở Roblox bằng**.
- Mỗi tài khoản một trang chủ riêng: chọn các tài khoản, để trống ô game, bấm **Mở Roblox**.

Cách hoạt động: Bloxkeep giữ trước hai đối tượng "chỉ một cửa sổ" của Roblox (`ROBLOX_singletonMutex`, `ROBLOX_singletonEvent`), nên cửa sổ mở sau không đóng cửa sổ trước. Bloxkeep không chèn gì vào Roblox.

### Tự vào lại game

Bật cho từng tài khoản trong menu ⋯ › **Tự vào lại game**. Khi Roblox của tài khoản đó bị văng, bị kick hoặc mất kết nối, Bloxkeep vào lại cùng game (hoặc cùng server riêng) sau 10 giây, nhưng không chắc vào đúng server cũ. Nếu cứ bị văng liên tục, từ lần thứ ba Bloxkeep chờ 2, 5, 10 rồi 15 phút giữa các lần thử (thẻ tài khoản hiện thời gian chờ và nút **Hủy**); ở lại trong game 10 phút thì đếm lại từ đầu. Nếu mất mạng, Bloxkeep thử lại mỗi 30 giây, tối đa 20 lần.

Bloxkeep không vào lại khi bạn tự thoát game hoặc khi tài khoản đăng nhập ở thiết bị khác. Tự tay đóng cửa sổ Roblox cũng tính là thoát game, nên hãy dùng nút **Đóng** trong app.

### Cấu hình Roblox theo tài khoản

Trong **Cài đặt › Cấu hình Roblox**, mục **Dùng cho** chọn **Không dùng**, **Mọi acc** hoặc **Acc được chọn** (chọn từng tài khoản bằng **Dùng cấu hình Roblox** trong menu ⋯). Đặt **Đồ họa** từ 1 đến 10, **Giới hạn FPS** 30/60/144/240, **Âm lượng** từ **Tắt tiếng** đến 100 %, hoặc **Giữ nguyên**.

Bloxkeep ghi các giá trị này vào file cài đặt của Roblox ngay trước khi mở tài khoản, rồi trả lại cài đặt của bạn khi Roblox đã chạy.

### Mở Roblox bằng

Chọn trong **Cài đặt › Mở Roblox › Mở Roblox bằng**. Bloxkeep tự tìm các bản đã cài.

| Lựa chọn | Ghi chú |
|---|---|
| **Mặc định** | Trình đang mở link Roblox trên máy (Roblox, Bloxstrap, Fishstrap…) |
| **Bloxstrap**, **Fishstrap** | Giữ nguyên mod và FFlag của bạn |
| Trình mở kiểu Bloxstrap khác | Tự hiện nếu đã cài |
| **Roblox (chính thức)** | Bản cài từ roblox.com |
| **Chạy thẳng RobloxPlayerBeta (bỏ qua trình mở)** | Không qua trình mở nào; thử khi trình mở chặn nhiều cửa sổ |
| **Chọn trình mở khác…** | File exe bạn tự chọn |
| **Roblox (Microsoft Store)** | Không hỗ trợ: không mở được tài khoản đã chọn. Hãy cài Roblox từ roblox.com |

### RAM và cửa sổ

- Thẻ **Roblox trên máy này** ở thanh bên hiện số cửa sổ Roblox, RAM chúng dùng, RAM máy còn trống, cùng nút **Đóng tất cả**. Dòng **Phiên bản Roblox** cho biết bản Roblox đã cài có phải mới nhất không.
- **Báo khi sắp hết RAM** (bật sẵn): cảnh báo trước khi mở nhiều tài khoản hơn RAM trống chịu được, và khi máy dùng quá 92 % RAM.
- **Tiết kiệm RAM cho cửa sổ chạy nền** (tắt sẵn): khi máy sắp hết RAM, cửa sổ chạy nền nhường RAM cho cửa sổ bạn đang chơi. Roblox có thể không nhả RAM; khi đó Bloxkeep bỏ qua.
- **Xếp lưới cửa sổ Roblox**: xếp mọi cửa sổ Roblox thành lưới trên màn hình bạn chọn.
- **Hiện tên acc trên cửa sổ** (bật sẵn): ghi tên tài khoản lên tiêu đề cửa sổ Roblox để dễ phân biệt trên taskbar.
- **Giãn cách giữa các acc**: từ 0 đến 60 giây, mặc định 8 giây. Mở dồn quá nhiều tài khoản một lúc dễ lỗi.

### Chạy ngầm, thông báo, giao diện

- **Chạy ngầm khi đóng** (bật sẵn): bấm X thì Bloxkeep thu xuống khay hệ thống, tự vào lại game và Multi-Roblox vẫn chạy. Muốn thoát hẳn, chuột phải biểu tượng ở khay và chọn **Thoát Bloxkeep**.
- **Thông báo Windows** (bật sẵn): chỉ khi Bloxkeep chạy nền, báo khi tài khoản bị văng, khi một tài khoản cứ bị văng liên tục, hoặc khi sắp hết RAM.
- **Discord** (mặc định tắt): dán link webhook của một kênh vào **Cài đặt › Discord** để nhận tổng kết thời gian chơi theo ngày hoặc tuần, một tin trạng thái tự cập nhật và các cảnh báo như trên, mỗi loại có công tắc và tùy chọn nhắc tên riêng.
- **Cài đặt › Giao diện**: 32 **Chủ đề** cộng **Theo Windows** (mặc định **Tối**), và **Ngôn ngữ** English / Tiếng Việt.
- **Tăng tốc phần cứng**: vẽ giao diện bằng card đồ họa thay vì CPU; tự tắt nếu driver card đồ họa gặp lỗi liên tục.

### Chuyển từ Roblox Account Manager

Roblox Account Manager mã hóa file `AccountData.json`, nên trước tiên hãy giải mã nó bằng công cụ chính thức [RAMDecrypt](https://github.com/ic3w0lf22/RAMDecrypt). Sau đó bấm **Chuyển từ Roblox Account Manager** và chọn file đã giải mã. Tên gợi nhớ, nhóm, ghi chú và mật khẩu (nếu RAM có lưu) được chép sang. Tài khoản đã có thì được cập nhật cookie, và cookie được kiểm tra ngay sau khi nhập.

Muốn thêm bằng cookie: chọn **Dùng cookie** rồi dán cookie `.ROBLOSECURITY`.

### Roblox VNG

Thêm được tài khoản Roblox VNG bằng tab **Roblox VNG** (đăng nhập bằng điện thoại hoặc bằng mật khẩu). Tài khoản VNG có nhãn "VNG". Hiện Bloxkeep **chưa mở được game** cho tài khoản VNG.

<details>
<summary>Thêm ảnh</summary>

<img src="docs/screenshots/settings.png" width="800" alt="Trang Cài đặt">
</details>

## Dữ liệu và bảo mật

**Lưu những gì.** Tài khoản (cookie, tên đăng nhập, ID, tên gợi nhớ, nhóm, ghi chú, mật khẩu nếu bạn tự nhập), cài đặt, game gần đây và đã ghim, bộ khởi chạy.

**Lưu ở đâu.** Mọi thứ nằm trên máy của bạn. **Cài đặt › Dữ liệu › Nơi lưu dữ liệu** hiện thư mục này, có nút **Mở thư mục**.

| Đường dẫn | Nội dung |
|---|---|
| `%LOCALAPPDATA%\Bloxkeep\` | Thư mục dữ liệu (bản cài). Bản portable dùng thư mục `data\` cạnh app. |
| `vault.rmv` | Kho đã mã hóa: tài khoản, cài đặt, game gần đây và đã ghim, bộ khởi chạy |
| `vault.rmv.bak` | Lần lưu trước đó, tự dùng nếu `vault.rmv` bị hỏng |
| `vault.rmv.damaged-…`, `vault.rmv.spare-…`, `vault-before-restore-….rmv` | Bản giữ lại sau khi tự sửa kho hoặc sau khi khôi phục; app không bao giờ xóa |
| `prefs.json` | Cài đặt không bí mật: ngôn ngữ, chủ đề, cập nhật, thông báo, thư mục sao lưu |
| `roblox-settings.json` | Cài đặt đồ họa, FPS, âm lượng của bạn, để trả lại sau khi mở tài khoản |
| `error.log` | Lỗi và ghi chú ngắn khi đăng nhập, không chứa thông tin tài khoản |
| `webview\`, `webview-safe\` | Bộ nhớ đệm của giao diện, bị xóa khi gỡ app |
| `%LOCALAPPDATA%\BloxkeepApp\` | Chương trình đã cài (bị xóa khi gỡ) |

**Bảo vệ thế nào.** Kho được mã hóa bằng AES-256-GCM. Khóa sinh từ mật khẩu chủ qua PBKDF2-HMAC-SHA256 với 600.000 vòng và salt ngẫu nhiên. Phần đầu file cũng được xác thực, nên sai mật khẩu hoặc file bị sửa đều bị phát hiện. Mật khẩu chủ không bao giờ được lưu. Mỗi lần lưu được ghi trọn vẹn rồi mới thay file cũ, và lần lưu trước được giữ trong `vault.rmv.bak`.

> [!WARNING]
> Quên mật khẩu chủ thì không khôi phục được tài khoản đã lưu. Bản sao lưu cũng chỉ mở được bằng mật khẩu lúc nó được lưu.

**Những gì rời khỏi máy.** Bloxkeep không có telemetry hay analytics. Dữ liệu chỉ đi tới:

| Nơi nhận | Khi nào, gửi gì |
|---|---|
| Máy chủ API của Roblox (`auth`, `users`, `presence`, `friends`, `games`… `.roblox.com`) | Đăng nhập (mật khẩu chỉ gửi tới `auth.roblox.com`), kiểm tra phiên, trạng thái mỗi 30 giây, tìm game, server, bạn bè, vé vào game, phiên bản Roblox mới nhất |
| `www.roblox.com` | Cửa sổ đăng nhập bằng trình duyệt, trang cá nhân, link vào game giao cho Roblox |
| `*.rbxcdn.com` | Ảnh đại diện và ảnh game |
| `*.robloxapp.vnggames.com` | Như trên, chỉ cho tài khoản Roblox VNG |
| Nhà cung cấp captcha của Roblox (Arkose Labs, HUMAN Security) | Chỉ trong cửa sổ câu đố, khi Roblox đòi captcha lúc đăng nhập bằng mật khẩu |
| GitHub | Kiểm tra và tải bản cập nhật, không gửi dữ liệu tài khoản; tắt được |
| Microsoft (`go.microsoft.com`) | Chỉ khi phải cài WebView2 |
| Thư mục sao lưu bạn chọn | Bản sao của kho đã mã hóa; app OneDrive / Google Drive của bạn có thể đồng bộ nó |
| Webhook Discord của bạn (`discord.com`) | Chỉ khi bạn bật **Cài đặt › Discord** (mặc định tắt): tên acc, tên game, thời gian chơi, số lượt, PID, RAM. Không bao giờ gửi cookie, mật khẩu hay chính link webhook; link được lưu trong kho đã mã hóa |

**Trong app.** Giao diện bị khóa bằng Content Security Policy nên không tự gửi request ra mạng; mọi kết nối đi qua phần lõi của app. Mật khẩu hay cookie chỉ tới giao diện khi bạn bấm hiện hoặc sao chép. Bản sao chép không vào lịch sử clipboard và tự xóa sau 30 giây. `error.log` không chứa thông tin tài khoản.

**Giữ cookie an toàn.** Cookie là chìa khóa đăng nhập đầy đủ của tài khoản. Đừng gửi cookie, file `vault.rmv` hay file sao lưu cho ai. Nếu lỡ lộ, hãy đăng xuất khỏi mọi phiên khác trong phần cài đặt của Roblox (trên roblox.com hoặc trong app Roblox).

**Tự kiểm tra.** Đối chiếu SHA-256 với `SHA256SUMS.txt`, đọc mã nguồn, hoặc [tự build](#tự-build-từ-mã-nguồn).

## Sao lưu và khôi phục

Bật **Cài đặt › Sao lưu › Sao lưu vào thư mục**, rồi bấm **Chọn…** hoặc dùng gợi ý thư mục OneDrive / Google Drive mà Bloxkeep tìm thấy trên máy (ví dụ **Dùng Google Drive**). Bloxkeep chỉ chép file vào thư mục đó; app OneDrive hoặc Google Drive của bạn tự tải lên.

- Bản sao lưu là bản sao của kho đã mã hóa, tên bắt đầu bằng `Bloxkeep-backup-` và kèm ngày giờ sao lưu.
- Sao lưu sau mỗi thay đổi (tối đa 10 phút một lần), khi khóa hoặc tắt app nếu có thay đổi, và ngay sau khi đổi mật khẩu chủ hay chọn thư mục mới. Bấm **Sao lưu ngay** để sao lưu tức thì.
- Giữ 10 bản mới nhất, cộng bản mới nhất của mỗi ngày trong 30 ngày gần đây. Bloxkeep chỉ dọn bản sao lưu của chính kho này, nên hai máy dùng chung một thư mục không xóa bản của nhau.
- Nếu mật khẩu chủ dễ đoán và thư mục là thư mục đám mây, app sẽ nhắc bạn đặt mật khẩu dài hơn.

**Khôi phục**: bấm **Khôi phục…** trong **Cài đặt › Sao lưu**, hoặc **Khôi phục từ bản sao lưu** ở màn hình đầu tiên trên máy mới. Dữ liệu hiện tại được giữ lại thành `vault-before-restore-….rmv`, sau đó app khóa lại; mở khóa bằng mật khẩu lúc bản sao lưu được lưu.

**Chuyển sang máy mới**: cài Bloxkeep, bấm **Khôi phục từ bản sao lưu**, chọn file sao lưu mới nhất, nhập mật khẩu lúc nó được lưu.

<img src="docs/screenshots/settings-backup.png" width="700" alt="Mục Sao lưu trong Cài đặt, sao lưu vào thư mục OneDrive">

## Giới hạn và tình trạng thử nghiệm

- Trạng thái online của Roblox bị trễ: thoát game rồi mà Roblox vẫn có thể báo "đang chơi" thêm vài phút.
- Không dùng được Roblox bản Microsoft Store.
- Roblox VNG: thêm được tài khoản, chưa mở được game.
- Multi-Roblox: Bloxkeep phải mở trước Roblox và để mở trong lúc chơi. Nếu Roblox (kể cả bản ở khay hệ thống) mở trước, đèn sẽ vàng đến khi đóng hết Roblox. Roblox không chính thức hỗ trợ nhiều client.
- VPN chỉ áp cho một số app: nếu Roblox đi qua VPN mà Bloxkeep thì không, mở game có thể báo lỗi 403. Xem [câu hỏi thường gặp](#câu-hỏi-thường-gặp).
- Ping của server là ping trung bình của người chơi, không phải của bạn. Server của bạn bè chỉ hiện nếu cài đặt riêng tư của họ cho phép.
- Tiết kiệm RAM: Roblox có thể không nhả RAM.
- File chưa có chữ ký số nên SmartScreen cảnh báo lần đầu. Tự cập nhật chỉ hoạt động khi repo GitHub để công khai.
- Quên mật khẩu chủ thì không khôi phục được dữ liệu.

Đã thử với 2 tài khoản thật trên Windows 11 (Roblox 0.742, tháng 10/2026): đăng nhập bằng mật khẩu qua bước proof-of-work và captcha nhấn-giữ của HUMAN, vào game, 2 tài khoản vào cùng một server bằng Multi-Roblox, mở 2 trang chủ riêng.

**Chưa thử với tài khoản thật:**

- Tự vào lại game trong thời gian dài.
- Bước cuối của **Đăng nhập bằng điện thoại**, xác minh 2 bước, và captcha xoay hình (Arkose).
- Chạy trên Windows 10.

Nếu gặp lỗi, hãy [mở issue](../../issues) kèm nội dung lỗi.

## Câu hỏi thường gặp

<details>
<summary>Tài khoản của tôi có bị ban không?</summary>

Không ai đảm bảo được. Roblox không chính thức hỗ trợ chạy nhiều client cùng lúc, nên dùng Multi-Roblox là bạn tự chịu rủi ro. Bloxkeep đăng nhập và mở game theo cùng cách trang roblox.com làm, và không đụng vào bộ nhớ của game. Một số game cấm dùng tài khoản phụ; hãy đọc luật của game bạn chơi.
</details>

<details>
<summary>Đây có phải virus không? Sao Windows hay antivirus cảnh báo?</summary>

Không. File chưa có chữ ký số nên SmartScreen và một số antivirus cảnh báo với file mới. Bạn có thể đối chiếu SHA-256 với `SHA256SUMS.txt`, đọc mã nguồn trong repo này, hoặc [tự build](#tự-build-từ-mã-nguồn).
</details>

<details>
<summary>Tôi quên mật khẩu chủ.</summary>

Không khôi phục được. Mật khẩu chủ không được lưu ở đâu cả. Bạn có thể xóa `vault.rmv` và `vault.rmv.bak` trong `%LOCALAPPDATA%\Bloxkeep` (mất toàn bộ tài khoản đã lưu) rồi thiết lập lại, hoặc khôi phục một bản sao lưu mà bạn còn nhớ mật khẩu.
</details>

<details>
<summary>Làm sao chuyển sang máy mới?</summary>

Bật sao lưu vào thư mục OneDrive hoặc Google Drive trên máy cũ. Trên máy mới, cài Bloxkeep, bấm **Khôi phục từ bản sao lưu** và nhập mật khẩu lúc sao lưu. Xem [Sao lưu và khôi phục](#sao-lưu-và-khôi-phục).
</details>

<details>
<summary>Đèn Multi-Roblox màu vàng.</summary>

Có Roblox đã mở trước Bloxkeep. Đóng hết Roblox, kể cả Roblox ở khay hệ thống. Nếu Roblox tự chạy cùng Windows, bấm **Tắt tự khởi động** trong bảng hướng dẫn. Đèn sẽ chuyển xanh. Lần sau hãy mở Bloxkeep trước Roblox.
</details>

<details>
<summary>Dùng Bloxstrap / Fishstrap thì mod còn không?</summary>

Còn, nếu chọn **Bloxstrap** hoặc **Fishstrap** trong **Mở Roblox bằng**. Nếu trình mở chặn nhiều cửa sổ, chọn **Chạy thẳng RobloxPlayerBeta (bỏ qua trình mở)**; khi đó Roblox mở không qua trình mở.
</details>

<details>
<summary>Thoát game rồi mà vẫn hiện "đang chơi".</summary>

Trạng thái của Roblox bị trễ, có khi vài phút. Với cửa sổ chạy trên máy này, Bloxkeep tự bỏ qua báo cáo cũ; trạng thái từ thiết bị khác vẫn có thể trễ.
</details>

<details>
<summary>Mở game báo lỗi 403 "An error was encountered during authentication".</summary>

Thường do VPN chỉ áp cho một số app: Roblox đi qua VPN còn Bloxkeep thì không. Vé vào game gắn với địa chỉ IP đã xin nó, nên hãy cho Bloxkeep đi cùng đường với Roblox.
</details>

<details>
<summary>Ở Việt Nam không vào được roblox.com thì có dùng được không?</summary>

Thêm tài khoản thì được. Đăng nhập bằng tên và mật khẩu không cần vào www.roblox.com, vì chỉ gọi tới máy chủ API và captcha của Roblox. Cách **Đăng nhập bằng trình duyệt** thì cần roblox.com. Bloxkeep không có VPN hay cách vượt chặn theo vùng nào. Tài khoản Roblox VNG thêm được nhưng chưa mở được game.
</details>

<details>
<summary>Bloxkeep có gửi mật khẩu hay cookie đi đâu không?</summary>

Chỉ tới Roblox. Mật khẩu chỉ gửi tới máy chủ đăng nhập của Roblox, cookie chỉ dùng để gọi máy chủ Roblox cho chính tài khoản đó. Danh sách đầy đủ ở bảng [Những gì rời khỏi máy](#dữ-liệu-và-bảo-mật).
</details>

<details>
<summary>Sao không hỗ trợ Roblox bản Microsoft Store?</summary>

Bản Microsoft Store không mở được tài khoản bạn chọn, nên Bloxkeep không dùng được nó. Hãy cài Roblox từ roblox.com (có thể dùng kèm Bloxstrap hoặc Fishstrap).
</details>

<details>
<summary>Làm sao xóa sạch Bloxkeep?</summary>

Gỡ Bloxkeep trong **Settings › Apps** của Windows, rồi xóa thư mục `%LOCALAPPDATA%\Bloxkeep`. Với bản portable, xóa thư mục chứa app (gồm cả `data`).
</details>

## Tự build từ mã nguồn

Cần Windows và **.NET 10 SDK**. App gồm phần lõi C# (.NET 10, WinForms) và giao diện HTML/CSS/JS chạy trong WebView2. Công cụ đóng gói [Velopack](https://velopack.io) (`vpk`) được khai báo trong `dotnet-tools.json` và script tự tải.

```bat
build.bat 1.0.1
```

Kết quả nằm trong `dist\releases\`: `Bloxkeep-Setup.exe`, `Bloxkeep-Portable.zip`, các gói cập nhật và `SHA256SUMS.txt`. Tham số thứ hai là link repo GitHub mà bản build sẽ tìm cập nhật (mặc định là repo này); truyền repo khác khi build bản fork.

```bat
release.bat 1.0.1 https://github.com/minhhoang0122/bloxkeep
```

`release.bat` đăng lên GitHub Releases, cần biến môi trường `GITHUB_TOKEN` có quyền ghi release. Mỗi phiên bản cần ghi chú (tiếng Anh và tiếng Việt) trong `RobloxManager\whatsnew.json`; ghi chú này hiện trong app sau khi cập nhật và trên trang phát hành. Để thử cập nhật trước khi đăng, đặt `BLOXKEEP_UPDATE_SOURCE` là một thư mục chứa các file phát hành.

**Xem thử giao diện không cần build**: chạy một web server tĩnh ở thư mục gốc repo (ví dụ `python -m http.server 5178`), rồi mở `http://127.0.0.1:5178/dev/preview.html`. Trang này dùng dữ liệu giả trong `dev/mock.js`.

```
RobloxManager/             mã nguồn C#
  wwwroot/                 giao diện (nhúng vào exe); chuỗi EN/VI trong i18n.js
  AppController.cs         nhận lệnh từ giao diện, mở game, theo dõi tiến trình, tự vào lại game
  RobloxApi.cs             gọi API Roblox (đăng nhập, trạng thái, game, server)
  RobloxLauncher.cs        mở Roblox qua trình mở đã chọn
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

[MIT](LICENSE) © 2026 minhhoang0122

*Bloxkeep là dự án độc lập, không liên kết, không được Roblox Corporation bảo trợ hay tài trợ. Roblox là thương hiệu của Roblox Corporation.*
