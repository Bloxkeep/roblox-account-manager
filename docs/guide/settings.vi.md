[English](settings.md)

# Cài đặt

Mọi trang của **Cài đặt**, mỗi trang một bảng gọn: từng dòng, tác dụng và giá trị mặc định.

<img src="../screenshots/vi/settings.png" width="760" alt="Cài đặt › Khởi chạy: Multi-Roblox, tên tài khoản trên cửa sổ, kích thước và kiểu xếp cửa sổ, launcher và thời gian chờ giữa các tài khoản">

**Cài đặt** ở thanh bên liệt kê các trang ở bên trái. Ô **Tìm cài đặt** ở trên cùng (Ctrl+F khi đang ở Cài đặt) tìm các dòng trên mọi trang; nhấn Esc để xóa. Chấm đỏ cạnh tên trang nghĩa là có chỗ cần xem lại: Roblox đã đóng cửa sổ hoặc có cảnh báo Multi-Roblox (trang **Phiên bản & đồ họa Roblox**), hoặc Discord đã ngừng gửi (trang **Discord**). Dấu gạch (—) ở cột **Mặc định** nghĩa là dòng đó là một nút bấm hoặc chỉ hiện thông tin.

## Khởi chạy

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Multi-Roblox** | Chơi nhiều tài khoản cùng lúc. **Chi tiết** mở các bước. Xem [Multi-Roblox](multi-roblox.vi.md). | Tắt |
| **Hiện tên tài khoản trên cửa sổ** | Ghi tên tài khoản và mã tiến trình (PID) lên tiêu đề cửa sổ Roblox ("Roblox - Main (PID 33588)"), dễ phân biệt dưới taskbar và khi bấm Alt+Tab, và dễ tìm trong Task Manager. | Bật |
| **Ảnh tài khoản làm biểu tượng cửa sổ** | Mỗi cửa sổ Roblox hiện ảnh của tài khoản thay cho biểu tượng Roblox. | Bật |
| **Nút trên taskbar** | Gộp chung mọi cửa sổ Roblox (như Windows vẫn làm), mỗi tài khoản một nút (kèm ảnh), hoặc mỗi preset một nhóm. | Gộp chung |
| **Khi cửa sổ Roblox mở ra** | **Để Roblox tự chọn**, **Phóng to** hoặc **Xếp cạnh nhau** (cần bật Multi-Roblox). | Để Roblox tự chọn |
| **Kiểu xếp cửa sổ** | Cách xếp khi bấm **Xếp cửa sổ** (và khi tự xếp sau khi mở): **Lấp đầy màn hình**, **Ô nhỏ**, **Ô vừa**, **Ô lớn**. | Lấp đầy màn hình |
| **Mở Roblox bằng** | Chương trình dùng để mở Roblox (bảng dưới). **Chọn…** để chọn chương trình của bạn. | Mặc định |
| **Chờ giữa các tài khoản** | 0–60 giây. Mở quá nhiều tài khoản cùng lúc dễ bị lỗi. | 8 giây |

**Mở Roblox bằng**:

| Lựa chọn | Ghi chú |
|---|---|
| **Mặc định** (*tên*) | Chương trình đang mở link Roblox trên máy. |
| **Bloxstrap**, **Fishstrap** | Giữ nguyên mod của bạn. Launcher kiểu Bloxstrap khác tự hiện nếu tìm thấy. |
| **Roblox (chính thức)** | Bản cài từ roblox.com. |
| **Chạy thẳng RobloxPlayerBeta (bỏ qua launcher)** | Không qua launcher nào. Nên thử khi launcher chặn mở nhiều cửa sổ. |
| **Roblox (Microsoft Store), đang thử nghiệm** | Chỉ hiện khi máy có Roblox bản Microsoft Store. Mỗi tài khoản mở trong một cửa sổ riêng. |
| **Launcher của bạn:** *tên* | Chương trình bạn chọn bằng nút **Chọn…**. Nếu file đó bị chuyển đi hoặc xóa, lúc mở game app sẽ báo. |

Lựa chọn chưa cài sẽ ghi "*tên* (chưa cài)" và bị mờ.

<details>
<summary>Chi tiết thêm</summary>

Với lựa chọn Microsoft Store, Bloxkeep mở Roblox của Store ngay bên trong gói cài của nó, với phiên đăng nhập riêng của từng tài khoản, bỏ qua màn hình khởi động của Microsoft (vốn chỉ cho một cửa sổ). Microsoft Store vẫn tự cài các bản cập nhật như thường. Nếu không chạy được, hãy báo ở **Cập nhật Bloxkeep › Gửi góp ý**. Nếu máy chỉ có Roblox bản Microsoft Store, ở đây có dòng ghi chú nói rõ.

</details>

## Phiên bản & đồ họa Roblox

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Phiên bản Roblox** | "Đã là bản mới nhất", hoặc đã có bản mới và cách lấy về. Roblox tự cập nhật lần mở tới; nếu dùng **Chạy thẳng RobloxPlayerBeta** thì mở Roblox bình thường một lần; Roblox bản Microsoft Store cập nhật qua Store. | — |
| **Bản đang cài**, **Bản mới nhất của Roblox** | Bản đang cài trên máy kèm launcher nó đến từ; bản mới nhất, chỉ hiện khi khác bản đang cài. | — |
| **Kiểm tra Multi-Roblox**, **Bản Roblox sắp ra** | Kiểm tra từng bản Roblox mới xem có thay đổi nào có thể chặn Multi-Roblox không. **Cách hoạt động** giải thích; khi có cảnh báo sẽ có nút **Kiểm tra bản cập nhật Bloxkeep**. | — |
| **Lịch sử cập nhật** | Mọi bản Roblox đã thấy trên máy, mới nhất ở trên, mỗi bản có link **Ghi chú** (ghi chú phát hành viết cho người làm game). | — |
| **Áp dụng cho** | Ai nhận ba giá trị bên dưới: **Không áp dụng**, **Mọi tài khoản**, **Tài khoản đã chọn** (**Chọn tài khoản…**, hoặc **⋯ › Dùng cấu hình Roblox** trên từng tài khoản). | Không áp dụng |
| **Đồ họa** | **Giữ nguyên**, 1–10. | Giữ nguyên |
| **Giới hạn FPS** | **Giữ nguyên**, 30 / 60 / 144 / 240 FPS. | Giữ nguyên |
| **Âm lượng** | **Giữ nguyên**, **Tắt tiếng**, 25 / 50 / 75 / 100 %. | Giữ nguyên |

Ba dòng giá trị bị mờ khi **Áp dụng cho** là **Không áp dụng**. Bloxkeep ghi các giá trị này vào cài đặt của Roblox ngay trước khi mở tài khoản, rồi trả lại cài đặt của bạn khi Roblox đã chạy.

## Treo game

Chi tiết từng dòng nằm ở [Giữ tài khoản trong game](staying-in-game.vi.md).

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Tự vào lại game** | **Bật cho mọi tài khoản** / **Tắt cho mọi tài khoản** (khi chỗ bật chỗ tắt ghi "Bật cho *n*/*m* tài khoản"). Menu ⋯ của từng tài khoản bật/tắt riêng cho tài khoản đó. | Tắt |
| **Chống AFK** | Treo máy không lo bị kick, kể cả khi bạn đi vắng hay đang dùng cửa sổ khác. Không hoạt động khi máy đang khóa. | Tắt |
| **Kiểm tra đăng nhập mỗi ngày** | Tìm các tài khoản Roblox đã đăng xuất, để bạn biết trước khi bấm Vào game. Ghi "Lần kiểm tra gần nhất: …". | Bật |
| **Báo khi sắp hết RAM** | Hỏi trước khi mở nếu thiếu RAM hoặc bộ nhớ ảo của Windows, và cảnh báo khi đang chơi. | Bật |
| **Tiết kiệm RAM cho cửa sổ chạy nền** | Cửa sổ Roblox bạn không chơi bị hạ mức ưu tiên bộ nhớ, nên Windows lấy RAM của chúng trước. | Tắt |
| **Nhường nhân mạnh cho cửa sổ đang chơi** | Cửa sổ Roblox chạy nền chỉ dùng nhân tiết kiệm của CPU, ở chế độ tiết kiệm điện; cửa sổ bạn đang chơi giữ các nhân mạnh. | Tắt |
| **Chạy ẩn dưới khay hệ thống khi đóng** | Nút ✕ thu Bloxkeep xuống khay hệ thống thay vì thoát. | Bật |
| **Tăng tốc phần cứng** | Vẽ cửa sổ Bloxkeep bằng card đồ họa. Tắt đi thì cửa sổ đỡ tốn RAM (khoảng 90 MB trên máy dùng card NVIDIA). Tự tắt nếu driver card đồ họa bị lỗi. Bật hay tắt thì cửa sổ được tải lại. | Bật |

## Thông báo

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Thông báo Windows** | Chỉ hiện khi Bloxkeep chạy nền (đang ẩn, thu nhỏ, hoặc bị cửa sổ khác che). Mỗi tài khoản, mỗi loại tối đa một thông báo trong 5 phút. Thông báo bên trong Bloxkeep thì vẫn hiện như thường. | Bật |
| **Loại sự kiện** | **Tài khoản bị văng hoặc tự vào lại**, **Preset hẹn giờ**, **Tài khoản cần đăng nhập lại** (gồm cả bị ban), **Sắp hết RAM**, **Lỗi khi mở game**. Bị mờ khi **Thông báo Windows** đang tắt. | Bật hết |
| **Câu hỏi xác nhận › Câu hỏi đã bỏ qua** | Số câu hỏi bạn đã tích **Không hỏi lại**. Xem [Vào game](playing.vi.md). | — |
| **Câu hỏi xác nhận › Hỏi lại** | Đưa tất cả trở lại. Bị mờ khi chưa bỏ qua câu nào. | — |

## Discord

Từng dòng được giải thích ở [Gửi lên Discord](discord.vi.md): **Gửi lên Discord**, **Link webhook**, **Gửi thử**, **Tổng kết thời gian chơi**, **Cảnh báo**, **Luôn cập nhật tin trạng thái**, **Giờ yên lặng** và **Tùy chỉnh tin nhắn**. **Gửi lên Discord** tắt cho tới khi bạn bật.

## Giao diện

<img src="../screenshots/vi/themes.png" width="760" alt="Cùng một tab Tài khoản trong sáu chủ đề: Dark, City, Aurora, Sakura, Paper và Terminal">

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Chủ đề** | **Theo Windows** (sáng hay tối theo Windows), hoặc một trong 74 chủ đề thuộc các nhóm **Cơ bản**, **Phong cảnh**, **Màu sắc**, **Phong cách** và **Theo mùa**. Dùng phím mũi tên để xem thử; chủ đề được lưu khi bạn dừng tay. | Tối |
| **Ngôn ngữ** | EN / VI. | EN |
| **Ẩn tên tài khoản** | Thay tên thật bằng "Tài khoản 1", "Tài khoản 2"… ở mọi chỗ. Xem [Sao lưu và bảo mật](backup-and-security.vi.md). | Tắt |

## Bảo mật

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Mật khẩu chủ** | **Mật khẩu hiện tại**, **Mật khẩu mới (ít nhất 6 ký tự)**, **Nhập lại mật khẩu mới**, rồi bấm **Đổi mật khẩu**. | Đặt lúc mở app lần đầu |

Độ mạnh hiện ra khi bạn gõ. App không chấp nhận mật khẩu hiện tại sai, mật khẩu mới ngắn hơn 6 ký tự hoặc chỉ toàn dấu cách, hay trùng mật khẩu cũ. Các bản sao lưu làm trước khi đổi vẫn chỉ mở được bằng mật khẩu cũ.

## Sao lưu & dữ liệu

Chi tiết nằm ở [Sao lưu và bảo mật](backup-and-security.vi.md).

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Sao lưu vào thư mục** | Chép kho dữ liệu đã mã hóa vào một thư mục, để không mất tài khoản khi máy hỏng. | Chưa chọn thư mục |
| **Thư mục** | **Chọn…**, hoặc thư mục đồng bộ mà Bloxkeep tìm thấy (**Dùng Google Drive**, **Dùng OneDrive**). **Mở thư mục** để mở ra xem. | — |
| **Lần sao lưu gần nhất** | "Sao lưu gần nhất: …" hoặc "Chưa có bản sao lưu", kèm nút **Sao lưu ngay** (bị mờ khi chưa chọn thư mục). | — |
| **Khôi phục** › **Khôi phục…** | Thay dữ liệu hiện tại bằng một bản sao lưu. Luôn hỏi trước; dữ liệu hiện tại được giữ lại một bản. | — |
| **Nhập tài khoản từ file dữ liệu Bloxkeep** › **Nhập…** | Thêm tài khoản từ một file dữ liệu hay bản sao lưu khác; tài khoản đang có vẫn giữ nguyên. | — |
| **Nơi lưu dữ liệu** › **Mở thư mục** | Mở thư mục Bloxkeep cất dữ liệu của bạn. | — |

## Cập nhật Bloxkeep

<img src="../screenshots/vi/3-settings-updates.png" width="620" alt="Cài đặt › Cập nhật Bloxkeep: phiên bản kèm nút Có gì mới và Khởi động lại để cập nhật, công tắc Tự động cập nhật, và các dòng Hướng dẫn sử dụng, Gửi góp ý">

| Dòng | Tác dụng | Mặc định |
|---|---|---|
| **Bloxkeep** *phiên bản* | Cho biết trạng thái cập nhật, kèm **Có gì mới** và **Kiểm tra ngay** (bị mờ khi đang kiểm tra hoặc đang tải) hay **Khởi động lại để cập nhật**. | — |
| **Tự động cập nhật** | Tự tải bản mới và tự cài vào lúc bạn không dùng. Tắt đi thì không có gì tự tải về, nhưng **Kiểm tra ngay** vẫn dùng được. Bị mờ với bản không tự cập nhật được. | Bật |
| **Nhận bản beta** | Cập nhật cả lên bản beta: có tính năng mới sớm hơn mọi người, nhưng có thể còn lỗi. Đang dùng bản beta mà tắt đi thì bạn vẫn giữ bản đó tới khi có bản chính thức tiếp theo. Bị mờ với bản không tự cập nhật được. | Tắt |
| **Hướng dẫn sử dụng › Mở** | Mở chính hướng dẫn này. | — |
| **Gửi góp ý › Viết góp ý** | Một ý tưởng, hoặc lỗi gặp phải. Lời nhắn đi thẳng tới người làm Bloxkeep. Xem [Gỡ rối](troubleshooting.vi.md). | — |

Khi bản cập nhật đã sẵn sàng, nút **Khởi động lại để cập nhật** hiện ở thanh bên. Roblox vẫn chạy bình thường trong lúc Bloxkeep khởi động lại, và mục **Có gì mới** cho biết những gì đã thay đổi.

← [Hướng dẫn](../guide.vi.md) · Tiếp: [Sao lưu và bảo mật](backup-and-security.vi.md) →
