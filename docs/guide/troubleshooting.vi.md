[English](troubleshooting.md)

# Gỡ rối và câu hỏi thường gặp

Trả lời ngắn cho những vấn đề hay gặp nhất, rồi đến toàn bộ phím tắt.

## Roblox và đăng nhập

### **H:** Roblox đóng mất các cửa sổ khác

**Đ:** Kiểm tra lần lượt:

- [Multi-Roblox](multi-roblox.vi.md) đã bật chưa? Thẻ ở thanh bên ghi **Đang tắt** thì bật lên.
- Đèn vàng **Chưa hoạt động, bấm để sửa**: Roblox đã mở trước Bloxkeep. Bấm vào và làm theo các bước (đóng hết Roblox, kể cả ở khay hệ thống; tắt tự khởi động nếu có). Lần sau nhớ mở Bloxkeep trước Roblox.
- Đèn vàng **Roblox đã đóng cửa sổ** hoặc **Bản Roblox mới có thể chặn**: một bản Roblox mới có thể đã đổi cách chặn nhiều cửa sổ. Mở **Cài đặt › Phiên bản & đồ họa Roblox**, bấm **Kiểm tra bản cập nhật Bloxkeep**, và chơi từng tài khoản một cho tới khi có bản sửa.
- Đang dùng launcher? Nếu launcher chặn mở nhiều cửa sổ, chọn **Chạy thẳng RobloxPlayerBeta (bỏ qua launcher)** ở **Cài đặt › Khởi chạy › Mở Roblox bằng**.

### **H:** Cửa sổ Roblox bị văng ngay khi mở ("Memory dump captured", hoặc Bloxstrap báo "Roblox failed to launch")

**Đ:** Bộ nhớ ảo của Windows đã đầy, dù RAM vẫn còn trống: mỗi cửa sổ Roblox giữ trước khoảng 2,5 GB bộ nhớ ảo. Bloxkeep sẽ cảnh báo trước khi bạn bấm **Vào game** nếu sắp xảy ra chuyện này.

- Tắt bớt ứng dụng không cần, hoặc
- Tăng bộ nhớ ảo. Đặt ở ổ nào cũng được nên không tốn chỗ ổ C: **Win+R** › `sysdm.cpl` › **Advanced › Performance › Settings › Advanced › Virtual memory › Change**. Bỏ tích **Automatically manage**, chọn ổ còn trống nhiều, đặt **Custom size** 16384 / 32768 MB, bấm **Set**, **OK**, rồi khởi động lại máy.

### **H:** Tài khoản báo "Cần đăng nhập lại"

**Đ:** Roblox đã kết thúc phiên đăng nhập đã lưu (đổi mật khẩu, đăng xuất mọi phiên, hoặc một đợt kiểm tra bảo mật). Bấm **Đăng nhập** trên thẻ và xác nhận bằng điện thoại, hoặc thêm lại tài khoản bằng cách bất kỳ: Bloxkeep chỉ cập nhật chứ không tạo bản trùng. Bộ lọc **Cần đăng nhập lại** gom hết các tài khoản này, và **Kiểm tra đăng nhập mỗi ngày** sẽ phát hiện trước khi bạn bấm Vào game.

### **H:** Tài khoản bị ban

**Đ:** Nhãn **Bị ban** lấy từ thông tin công khai của Roblox, đọc trong lúc kiểm tra đăng nhập. **Vào game** và preset bỏ qua tài khoản này, tự vào lại game cũng không mở lại nó. Khi hết ban, bấm **⋯ › Kiểm tra đăng nhập** để bỏ nhãn.

### **H:** Một tài khoản bị game ban. Các tài khoản khác còn vào game đó được không?

**Đ:** Cẩn thận: game ban người chơi chứ không chỉ một tài khoản. Công cụ ban của Roblox ban luôn các tài khoản mà nó cho là của cùng người đó, với lý do "dùng tài khoản khác để né ban". Vì vậy khi bạn bấm **Vào game** cho tài khoản khác vào đúng game mà một tài khoản của bạn đang bị ban, Bloxkeep hỏi trước và nói rõ tài khoản nào. Preset thì vẫn mở nhưng có ghi trong kết quả. Nếu bị game ban nhầm, hãy kháng cáo với chính game đó (đa số qua server Discord của game): chỉ game mới gỡ được ban của mình.

### **H:** Máy chưa cài Roblox, hoặc chỉ có bản Microsoft Store

**Đ:**

- Thanh bên báo "Hình như máy này chưa cài Roblox.": cài Roblox từ roblox.com/download.
- Chỉ có Roblox bản Microsoft Store: chọn **Roblox (Microsoft Store), đang thử nghiệm** ở **Cài đặt › Khởi chạy › Mở Roblox bằng**, hoặc cài Roblox từ roblox.com.
- Launcher nào chưa cài thì ghi "(chưa cài)" trong danh sách đó. Hãy chọn cách mở khác.

### **H:** "Sao chép mật khẩu" bị mờ hoặc báo chưa lưu

**Đ:** Bị mờ nghĩa là bạn đang chọn nhiều tài khoản: các mục sao chép chỉ dùng cho một tài khoản mỗi lần. "Tài khoản này chưa lưu mật khẩu" nghĩa là Bloxkeep không giữ lại mật khẩu lúc đăng nhập: hãy gõ mật khẩu ở **Sửa (F2) › Mật khẩu Roblox** rồi lưu. **Sao chép mã 2FA** cũng vậy, với **Sửa › Khóa app xác thực**. Xem [Tài khoản](accounts.vi.md).

### **H:** Vì sao mục khác bị mờ?

**Đ:** Tìm mục đó ở đây:

| Bị mờ | Vì sao |
|---|---|
| Nút **Vào game** ở thanh dưới ("Chọn tài khoản trước") | Chưa chọn tài khoản nào. |
| **Vào game** trên thẻ | Tài khoản đang được mở. |
| **Mở Roblox** | Chưa chọn tài khoản nào. |
| **Chọn server** | Ô game chưa có game, hoặc đang là server riêng. |
| **Đóng** ở thanh dưới, **Đóng Roblox**, **Buộc tắt** | Không tài khoản nào đã chọn đang mở trên máy này. |
| **Sửa…**, **Vào cùng server**, **Trang cá nhân Roblox**, **Mở trang web**, các mục **Sao chép** | Đang chọn nhiều tài khoản. **Vào cùng server** còn cần biết server; **Mở trang web** không dùng được cho Roblox VNG. |
| **Chạy** của preset | Preset không còn tài khoản, hoặc đang có lượt mở khác. |
| **Đồ họa**, **Giới hạn FPS**, **Âm lượng** | **Áp dụng cho** đang là **Không áp dụng**. |
| Các dòng trong **Loại sự kiện** | **Thông báo Windows** đang tắt. |
| **Sao lưu ngay** | Chưa chọn thư mục sao lưu. |
| **Hỏi lại** | Bạn chưa bỏ qua câu hỏi nào. |
| **Tự động cập nhật**, **Nhận bản beta** | Bản này không tự cập nhật được. |

## Dữ liệu và mật khẩu

### **H:** Dữ liệu của tôi nằm ở đâu?

**Đ:** **Cài đặt › Sao lưu & dữ liệu › Nơi lưu dữ liệu › Mở thư mục** sẽ mở đúng chỗ. Bản cài đặt dùng `%LOCALAPPDATA%\Bloxkeep`; bản portable dùng thư mục `data` nằm cạnh `Bloxkeep.exe`. Xem [Sao lưu và bảo mật](backup-and-security.vi.md).

### **H:** Tôi quên mật khẩu chủ

**Đ:** Không lấy lại được, và cũng không cần gỡ cài đặt. Ngay dưới nút **Mở khóa** có hai lựa chọn:

- **Khôi phục từ bản sao lưu**: mở một bản sao lưu mà bạn còn nhớ mật khẩu. File đang không mở được vẫn được giữ lại.
- **Tạo vault mới**: một vault riêng với mật khẩu chủ mới. Tài khoản Roblox không bị ảnh hưởng gì, chỉ cần đăng nhập lại để thêm vào. Vault cũ vẫn nằm trong danh sách, nhớ ra mật khẩu thì chọn lại nó; không cần nữa thì gỡ nó bằng nút **×** ở màn hình chọn vault (file vẫn được giữ trong thư mục dữ liệu).

## Hẹn giờ và trạng thái

### **H:** Preset hẹn giờ không chạy

**Đ:** Đến giờ chạy, Bloxkeep phải đang chạy và đã mở khóa. Những lần bị lỡ khi Bloxkeep tắt, đang khóa hoặc máy ngủ sẽ không được chạy bù. Lần chạy cũng bị bỏ qua nếu cần bạn xác nhận mới mở được (Multi-Roblox tắt hoặc chưa sẵn sàng, thiếu RAM, có tài khoản đang ở game khác); thông báo sẽ nói rõ lý do. Xem [Preset](presets.vi.md).

### **H:** Đã thoát game mà vẫn báo đang chơi

**Đ:** Trạng thái của Roblox bị chậm, có khi tới vài phút. Với các cửa sổ trên máy này, Bloxkeep đọc theo log của chính Roblox nên không bị ảnh hưởng.

## Góp ý

### **H:** Gửi góp ý kèm nhật ký lỗi thế nào?

**Đ:** Vào **Cài đặt › Cập nhật Bloxkeep › Gửi góp ý › Viết góp ý** (hoặc Ctrl+K rồi gõ "góp ý"):

1. **Bạn muốn gửi gì?**: **Báo lỗi** hoặc **Góp ý, ý tưởng**.
2. **Nội dung**, và **Cách liên hệ với bạn** (không bắt buộc) nếu bạn muốn được trả lời.
3. Ô **Gửi kèm nhật ký lỗi của Bloxkeep** được tích sẵn khi báo lỗi. Tên tài khoản, cookie và mật khẩu được xóa trước khi gửi. Bloxkeep phải đang mở khóa mới gửi kèm được.
4. **Xem đúng những gì sẽ gửi đi** cho xem toàn bộ nội dung trước khi gửi.
5. Bấm **Gửi**. Mỗi phút chỉ gửi được một lần.

Lời nhắn đi thẳng tới người làm Bloxkeep qua Discord, không kèm gì khác từ máy bạn ([chi tiết](../feedback.md), tiếng Anh).

## Phím tắt

| Ở đâu | Phím | Tác dụng |
|---|---|---|
| Mọi nơi | Ctrl+K | Tìm nhanh: tài khoản, preset, game, trang cài đặt và thao tác. Nhấn lần nữa để đóng. Không dùng được ở màn hình khóa hay khi đang có hộp thoại khác. |
| Mọi nơi | Ctrl+F | Ô tìm kiếm của tab đang mở (Tài khoản, Preset khi đã có preset, Cài đặt). Ở các tab khác thì nhảy tới ô tìm tài khoản. |
| Mọi nơi | Ctrl+L hoặc `/` | Nhảy tới ô game. |
| Mọi nơi | Esc | Rời ô đang gõ; nếu không thì bỏ chọn tài khoản. Đóng menu, danh sách hoặc hộp thoại. |
| Tab Tài khoản | Bấm / Shift + bấm | Chọn hoặc bỏ chọn một thẻ / chọn liền một dãy. |
| Tab Tài khoản | Bấm đúp | Vào game riêng tài khoản đó. |
| Tab Tài khoản | Chuột phải | Menu của tài khoản. |
| Tab Tài khoản | Phím mũi tên, Home, End | Đi giữa các thẻ. |
| Tab Tài khoản | Space | Chọn thẻ đang trỏ tới. |
| Tab Tài khoản | Ctrl+A | Chọn mọi tài khoản đang hiện. |
| Tab Tài khoản | Enter | Vào game với các tài khoản đã chọn. |
| Tab Tài khoản | F2 | Sửa tài khoản đang chọn (đúng một tài khoản). |
| Tab Tài khoản | Delete | Xóa các tài khoản đã chọn (có hỏi trước). |
| Ô game, ô Job ID | ↓ / ↑ | Đi trong danh sách game. |
| Ô game, ô Job ID | Enter | Chọn game đang sáng; nếu ô đang có link hoặc Place ID thì vào game. Trong ô Job ID: vào game. |
| Ô game, ô Job ID | Esc | Đóng danh sách game. |
| Tìm nhanh | ↓ / ↑, Page Down / Page Up | Đi trong danh sách kết quả. |
| Tìm nhanh | Enter | Mở mục đang sáng (tài khoản thì được chọn và hiện ra; preset thì chạy; game thì đưa vào ô game). |
| Tìm nhanh | Ctrl+Enter / Shift+Enter | Với tài khoản: **Vào game** / **Sửa**. |
| Menu và hộp thoại | ↓ / ↑ | Đi trong menu. Tab hoặc Esc để đóng menu. |
| Menu và hộp thoại | Enter | Trong form: bấm nút chính. |
| Menu và hộp thoại | Esc | Đóng hộp thoại (hỏi trước nếu bạn đã gõ gì đó). |
| Menu và hộp thoại | Tab | Di chuyển giữa các ô trong hộp thoại. |
| Danh sách trang Cài đặt | ↓ / ↑, Home, End | Chuyển trang (cửa sổ hẹp thì dùng thêm ← / →). |
| Bảng chọn chủ đề | Phím mũi tên, Home, End | Xem thử các chủ đề. |
| Ô Tìm cài đặt | Esc | Xóa nội dung tìm. |
| Cửa sổ nhỏ | Esc | Ẩn cửa sổ nhỏ. |
| Cửa sổ trình duyệt của Bloxkeep | Ctrl+T / Ctrl+W / Ctrl+Tab | Mở tab trống / đóng tab / chuyển tab. |

Các phím ở tab Tài khoản dùng được khi bạn không đang gõ trong ô nào.

← [Hướng dẫn](../guide.vi.md)
