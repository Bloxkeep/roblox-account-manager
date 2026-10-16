[English](accounts.md)

# Tài khoản

Cách thêm tài khoản, đọc thẻ tài khoản, tìm và chọn tài khoản, và dùng menu của tài khoản.

<img src="../screenshots/vi/accounts-list.png" width="760" alt="Tab Tài khoản ở dạng danh sách: các nút lọc kèm số đếm, ô tìm kiếm, Nhóm và Sắp xếp, mỗi tài khoản một dòng kèm trạng thái">

## Làm nhanh

1. Bấm **Thêm tài khoản**.
2. Trên điện thoại, mở app Roblox đã đăng nhập tài khoản đó, vào **Cài đặt → Đăng nhập nhanh**.
3. Nhập mã Bloxkeep đang hiện, hoặc quét mã QR, rồi xác nhận trên điện thoại.
4. Thêm xong một tài khoản thì có mã mới cho tài khoản tiếp theo. Xong hết thì bấm **Xong**.

## Các cách thêm tài khoản

**Thêm tài khoản** mở hộp **Đăng nhập Roblox**. Mũi tên bên cạnh (**Cách thêm khác**) có các cách còn lại.

<img src="../screenshots/vi/add-account.png" width="760" alt="Hộp Đăng nhập Roblox: Đăng nhập bằng điện thoại (nên dùng), tên đăng nhập và mật khẩu, và các cách khác: trình duyệt, cookie, file Bloxkeep">

| Cách | Dùng khi | Ghi chú |
|---|---|---|
| **Đăng nhập bằng điện thoại** (nên dùng) | Hầu như lúc nào cũng nên dùng. | Mã hoặc QR, không cần mật khẩu, không phải giải đố. Mã tự làm mới vài lần, sau đó bấm **Lấy mã mới**. |
| **Dùng tên đăng nhập & mật khẩu** | Không có điện thoại bên cạnh. | Mật khẩu chỉ gửi tới Roblox; Bloxkeep không lưu lại. Roblox có thể bắt giải đố hoặc nhập mã 2 bước. |
| **Đăng nhập bằng trình duyệt** | Các cách khác không được, hoặc Roblox đòi xác minh thêm. | Trang đăng nhập chính thức của roblox.com. Cần vào được www.roblox.com và Microsoft Edge WebView2. Không dùng cho Roblox VNG. |
| **Dán cookie** | Bạn đã có sẵn cookie `.ROBLOSECURITY` của tài khoản. | Chỉ **Cookie .ROBLOSECURITY** là bắt buộc. |
| **Mở roblox.com** rồi **Lưu vào Bloxkeep** | Chủ yếu cần dùng trang web, nhiều tài khoản cùng lúc. | Một tab trống trong cửa sổ trình duyệt của Bloxkeep ([Mở trang web](playing.vi.md#mở-trang-web)). Đăng nhập, rồi bấm **Lưu vào Bloxkeep**. |
| **Nhập từ: File Bloxkeep** | Chuyển tài khoản từ máy khác hoặc từ bản sao lưu. | Cần mật khẩu chủ của file đó ([Nhập từ file Bloxkeep khác](#nhập-từ-file-bloxkeep-khác)). |

Thêm một tài khoản đã có trong danh sách thì không bị trùng: Bloxkeep chỉ cập nhật phiên đăng nhập của nó. Bloxkeep không đọc được file của các trình quản lý tài khoản khác; nếu công cụ kia cho xem cookie của tài khoản thì dùng **Dán cookie**.

> [!WARNING]
> Cookie `.ROBLOSECURITY` là chìa khóa của tài khoản: ai có nó là dùng được tài khoản đó. Đừng bao giờ đưa cho ai.

<details>
<summary>Chi tiết thêm</summary>

- **Điện thoại**: khi điện thoại đã nhập mã, hãy xác nhận trên đó. "Mã đã hết hạn." và "Đã từ chối trên thiết bị." đều cần bấm **Lấy mã mới**. Hai tab **Roblox** / **Roblox VNG** để chọn loại tài khoản.
- **Xác minh 2 bước**: nhập mã 6 số ở tab mà tài khoản có (**App xác thực**, **Email**, **Tin nhắn SMS**). Nếu khóa app xác thực đã lưu trong Bloxkeep, mã được điền sẵn: chỉ cần bấm **Xác minh**.
- Đôi khi Roblox không cho đăng nhập bằng mật khẩu (tài khoản bị khóa, dùng passkey hoặc khóa bảo mật). Khi đó thông báo sẽ bảo bạn dùng điện thoại hoặc trình duyệt.

</details>

## Thẻ tài khoản

Nhãn ở góc thẻ là **Trên máy này** (Roblox đang mở cho tài khoản ở máy này; rê chuột lên để xem PID), **Đang mở**, **Bị ban** hoặc **Cần đăng nhập lại**. Chấm tròn ở avatar cho biết trạng thái, và dòng chữ dưới tên nói rõ hơn:

| Chấm | Chữ dưới tên | Nghĩa là |
|---|---|---|
| Xanh lá | **Đang chơi**, **Đang vào game**, **Đang chơi · thiết bị khác** | Đang ở trong game, trên máy này hoặc thiết bị khác. |
| Xanh dương | **Online**, **Roblox đang mở** · Chưa vào game | Đang trên web hoặc app Roblox, hoặc Roblox đang ở màn hình chính trên máy này. |
| Cam | **Roblox Studio** | Đang làm game. |
| Vàng | **Bị văng khỏi server** | Cửa sổ vẫn mở nhưng đã ra khỏi server, kèm lý do như **Bị kick vì AFK**. |
| Xám | **Offline**, **Đã đăng xuất**, **Chưa rõ trạng thái** | Không hoạt động; Roblox đang mở ở máy này nhưng không còn đăng nhập tài khoản này; hoặc chưa tải xong. |

Vòng đỏ quanh avatar nghĩa là **Cần đăng nhập lại** hoặc **Bị ban**. Khi Bloxkeep đang xử lý, bạn còn thấy **Đang mở Roblox…**, **Chờ vào lại · máy này** (bấm **Hủy** để thôi) và **Đang mở lại · máy này** (preset sắp mở lại cửa sổ theo lịch).

<img src="../screenshots/vi/card-rejoin.png" width="760" alt="Các thẻ tài khoản: một thẻ chờ vào lại sau 5 phút kèm nút Hủy, một thẻ bị văng vì AFK, một thẻ cần đăng nhập lại, một thẻ bị ban">

Tài khoản đang mở trên máy này còn hiện **RAM đang dùng**, **Thời gian đã chơi** và **PID** (bấm để sao chép). Các nút là **Vào game** (đổi thành **Đóng** khi đang mở, **Hủy** khi đang chờ vào lại, **Đăng nhập** khi cần đăng nhập lại), **Sửa (F2)** và **⋯ Tùy chọn khác**. Trạng thái tự làm mới mỗi 30 giây; bấm **Làm mới trạng thái online** để làm mới ngay.

## Chọn, tìm và sắp xếp

- **Bấm** vào thẻ để chọn, bấm lần nữa để bỏ chọn. **Shift + bấm** chọn liền một dãy. **Bấm đúp** để vào game riêng tài khoản đó.
- **Ctrl+A** hoặc **Chọn tất cả** chọn mọi tài khoản đang hiện. **Esc** hoặc **Bỏ chọn** bỏ chọn hết.
- **Tìm** (Ctrl+F) theo tên, tên đăng nhập (gõ `@tên` cũng được), tên gợi nhớ, nhóm, ghi chú, game đang chơi và ID người dùng. Không cần gõ dấu.
- **Bộ lọc** có kèm số đếm: **Tất cả**, **Cần đăng nhập lại** và **Bị ban** (chỉ hiện khi có tài khoản như vậy), **Đang chơi**, **Online**, **Offline**, **Trên máy này**.
- **Nhóm** hiện ra khi có tài khoản được xếp nhóm (đặt ở **Sửa › Nhóm**). **Sắp xếp**: **Tên**, **Trạng thái**, **Chơi gần đây**, **Thứ tự mở**.
- **Dạng thẻ** / **Dạng danh sách** đổi kiểu xem. Kiểu xem và cách sắp xếp được nhớ lại.

Nếu ô tìm kiếm hay bộ lọc đang che bớt tài khoản đã chọn, đầu trang ghi rõ số bị ẩn. **Vào game** và **Xóa** vẫn tính cả chúng.

## Menu của tài khoản

Mở bằng **⋯ Tùy chọn khác** trên thẻ, hoặc bấm chuột phải vào tài khoản. Nếu tài khoản chưa được chọn, nó thành tài khoản duy nhất được chọn; nếu nó nằm trong nhóm đang chọn, menu áp dụng cho mọi tài khoản đã chọn.

<img src="../screenshots/vi/account-menu.png" width="760" alt="Menu của tài khoản mở trên một thẻ: Vào game, Mở Roblox, Đóng Roblox, Buộc tắt, Sửa, Kiểm tra đăng nhập, Vào cùng server, Tự vào lại game, Trang cá nhân Roblox, Mở trang web, các mục Sao chép và Xóa">

| Mục | Làm gì | Không dùng được khi |
|---|---|---|
| **Vào game** / **Vào game (N tài khoản)** (Enter) | Mở game đang có trong ô game, ô trống thì mở màn hình chính Roblox. | — |
| **Mở Roblox (không vào game)** | Mở Roblox ở màn hình chính, mỗi tài khoản một cửa sổ. | — |
| **Đóng Roblox** | Đóng cửa sổ Roblox như bình thường. | Không tài khoản nào đã chọn đang mở trên máy này. |
| **Buộc tắt** | Tắt Roblox ngay, dùng khi cửa sổ bị treo. Những gì chưa lưu trong game sẽ mất. | Giống **Đóng Roblox**. |
| **Sửa…** (F2) | Tên gợi nhớ, nhóm, mật khẩu, khóa app xác thực, ghi chú. | Đang chọn nhiều tài khoản. |
| **Kiểm tra đăng nhập** | Kiểm tra phiên đăng nhập đã lưu và xem Roblox có báo bị ban không. | — |
| **Vào cùng server** | Đưa game và server (Job ID) của tài khoản này vào thanh chọn game; chọn các tài khoản khác rồi bấm **Vào game** để vào chung. | Đang chọn nhiều tài khoản, hoặc không biết server. |
| **Tự vào lại game** | Bật hoặc tắt tự vào lại game cho các tài khoản này. | — |
| **Dùng cấu hình Roblox** | Cho các tài khoản này dùng **Cấu hình Roblox** (đồ họa, FPS, âm lượng). | Chỉ hiện khi **Cài đặt › Phiên bản & đồ họa Roblox › Áp dụng cho** là **Tài khoản đã chọn**. |
| **Trang cá nhân Roblox** | Mở trang cá nhân trên roblox.com bằng trình duyệt của bạn. | Đang chọn nhiều tài khoản, hoặc không có ID người dùng. |
| **Mở trang web** | Mở roblox.com với tài khoản này đã đăng nhập sẵn, trong cửa sổ trình duyệt của Bloxkeep. | Đang chọn nhiều tài khoản, hoặc là tài khoản Roblox VNG. |
| **Sao chép tên đăng nhập** | Sao chép tên đăng nhập. | Đang chọn nhiều tài khoản. |
| **Sao chép mật khẩu** | Sao chép mật khẩu đã lưu. | Đang chọn nhiều tài khoản. Chưa lưu mật khẩu thì app giải thích lý do. |
| **Sao chép mã 2FA** | Sao chép mã 6 số hiện tại. | Đang chọn nhiều tài khoản. Chưa lưu khóa thì app giải thích lý do. |
| **Xóa** / **Xóa N tài khoản** (Del) | Xóa tài khoản khỏi Bloxkeep sau khi hỏi lại. Không thể hoàn tác. | — |

Menu **⋯ Tùy chọn khác** ở đầu trang có **Kiểm tra đăng nhập mọi tài khoản**, **Chọn tất cả**, **Xếp cửa sổ Roblox**, **Thứ tự mở tài khoản…** (cần từ 2 tài khoản; xem [Vào game](playing.vi.md#thứ-tự-mở-tài-khoản)), **Đóng tất cả Roblox**, **Buộc tắt tất cả Roblox** và **Hướng dẫn sử dụng**.

## Sao chép mật khẩu và mã 2FA

Bloxkeep không giữ lại mật khẩu bạn dùng để đăng nhập. Nếu bạn chưa tự lưu, **Sao chép mật khẩu** sẽ báo "Tài khoản này chưa lưu mật khẩu". **Sao chép mã 2FA** cũng cần có khóa app xác thực. Cách lưu:

1. Chọn tài khoản rồi bấm **F2** (hoặc **Sửa…** trong menu).
2. Gõ mật khẩu vào ô **Mật khẩu Roblox**. Với 2FA, dán khóa (chữ A–Z và số 2–7) hoặc link `otpauth://` vào ô **Khóa app xác thực**, rồi xem "Mã hiện tại: …" có trùng với app xác thực của bạn không.
3. Bấm **Lưu**. Mục **Sửa** từ đó hiện mã 2FA đang chạy, kèm nút **Sao chép** và **Gỡ**.

- **Sao chép mật khẩu** tự xóa khỏi clipboard sau 30 giây. Mật khẩu nằm trong kho đã mã hóa và chỉ để bạn dùng: Bloxkeep không dùng nó để đăng nhập.
- **Sao chép mã 2FA** sao chép mã hiện tại ("Mã đổi sau *N* giây."). Khi đăng nhập bằng mật khẩu, mã cũng được điền sẵn.

> [!WARNING]
> Lưu khóa trong Bloxkeep thì ai có mật khẩu chủ của bạn cũng tạo được mã. Chỉ lưu khi bạn chấp nhận điều đó.

## Cần đăng nhập lại và Bị ban

| Nhãn | Vì sao | Cách xử lý |
|---|---|---|
| **Cần đăng nhập lại** | Roblox đã kết thúc phiên đăng nhập đã lưu: đổi mật khẩu, đăng xuất mọi phiên, hoặc một đợt kiểm tra bảo mật. | Bấm **Đăng nhập** trên thẻ và xác nhận bằng điện thoại, hoặc thêm lại tài khoản bằng cách bất kỳ (chỉ cập nhật, không tạo bản trùng). |
| **Bị ban** | Thông tin công khai của Roblox cho biết như vậy; app đọc nó lúc kiểm tra đăng nhập. | **Vào game** và preset bỏ qua tài khoản này, tự vào lại game cũng không mở lại. Hết ban thì **⋯ › Kiểm tra đăng nhập** sẽ xóa nhãn. |

Bộ lọc **Cần đăng nhập lại** gom hết các tài khoản này. **Kiểm tra đăng nhập mỗi ngày** phát hiện trước khi bạn bấm **Vào game** ([Giữ tài khoản trong game](staying-in-game.vi.md)). Muốn kiểm tra ngay, dùng **⋯ › Kiểm tra đăng nhập** trên một tài khoản, hoặc **⋯ Tùy chọn khác › Kiểm tra đăng nhập mọi tài khoản**.

## Nhập từ file Bloxkeep khác

Dùng cho tài khoản trong `vault.rmv` của máy khác, hoặc file `Bloxkeep-backup-…`. Mở ở **Cách thêm khác › Nhập từ file dữ liệu Bloxkeep khác…**, hoặc **Cài đặt › Sao lưu & dữ liệu › Nhập tài khoản từ file dữ liệu Bloxkeep › Nhập…**.

1. Bấm **Chọn file…** và chọn file `.rmv`.
2. Nhập **Mật khẩu chủ của file này** rồi bấm **Mở**.
3. Tích những tài khoản muốn lấy. Mỗi dòng ghi **Mới** hay **Đã có**. **Chọn hết** / **Bỏ chọn** để tích hoặc bỏ tất cả.
4. Muốn lấy cả preset thì tích **Nhập cả preset (N)**, rồi bấm **Nhập**.

- File gốc vẫn giữ nguyên. Tài khoản lấy sang được khóa bằng mật khẩu chủ của Bloxkeep trên máy này.
- Với tài khoản đã có, Bloxkeep chỉ thay phiên đăng nhập nếu phiên cũ không còn dùng được. Tên, nhóm và ghi chú bạn đặt vẫn giữ nguyên.
- Preset nhập vào có hẹn giờ bị tắt. Muốn thay toàn bộ dữ liệu (máy mới) thì dùng **Khôi phục** ([Sao lưu và bảo mật](backup-and-security.vi.md)).

## Roblox VNG

Thêm các tài khoản này ở tab **Roblox VNG** của hộp **Đăng nhập Roblox** hoặc của **Đăng nhập bằng điện thoại** (xác nhận trong app Roblox VNG). Không có cách đăng nhập bằng trình duyệt cho loại này. Chúng có nhãn **VNG**.

- Bloxkeep chưa mở được game cho chúng: nút **Vào game** bị mờ ("Bloxkeep chưa mở được Roblox VNG.").
- Khi mở nhiều tài khoản một lúc, tài khoản VNG bị bỏ qua và kết quả có ghi rõ. **Mở trang web** cũng bị mờ.

← [Hướng dẫn](../guide.vi.md) · Tiếp: [Vào game](playing.vi.md) →
