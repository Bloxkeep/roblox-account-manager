[English](playing.md)

# Vào game

Cách chọn game và server, mở Roblox cho các tài khoản, và dùng cửa sổ trình duyệt của Bloxkeep để vào roblox.com.

<img src="../screenshots/vi/game-picker.png" width="760" alt="Danh sách game mở phía trên thanh chọn game, với các tab Gần đây, Đã ghim, Yêu thích trên Roblox và Phổ biến">

## Làm nhanh

1. Ở tab **Tài khoản**, chọn những tài khoản muốn mở.
2. Bấm vào ô game ở cuối trang, gõ tên game, Place ID hoặc link. Chọn game trong danh sách hiện ra.
3. Bấm **Vào game**. Nút cho biết sẽ mở bao nhiêu tài khoản.
4. Nếu Bloxkeep hỏi **Lưu ý trước khi mở**, đọc rồi bấm **Vẫn mở**.
5. Muốn dừng giữa chừng thì bấm **Dừng**. Muốn đóng Roblox thì bấm **Đóng**.

## Thanh chọn game

Thanh ở cuối tab **Tài khoản** có, từ trái sang phải: ảnh game (**Chọn game**), ô game, ô **Job ID (tùy chọn)**, nút **Vào cùng người chơi…**, rồi các nút **Xếp cửa sổ**, **Mở Roblox mà không vào game**, **Đóng**, **Preset** và **Vào game**.

Ô game ("Tìm game, hoặc dán link / Place ID…", Ctrl+L hoặc `/`) nhận:

| Bạn gõ hoặc dán | Ghi chú |
|---|---|
| Tên game | Chọn game trong danh sách. Gõ tên mà không chọn thì không tính. |
| Place ID | |
| Link `roblox.com/games/…` | Link server riêng cũng được. |
| Link chia sẻ server riêng | `roblox.com/share?…type=Server` |
| Link `roblox://` | |

Dưới ô game là tên game, người làm game, số người đang chơi và nút **Server**. Server riêng có nhãn **Server riêng**, game có Job ID ghi **Server cụ thể**, còn link chia sẻ ghi "Link server riêng: Bloxkeep sẽ mở link này khi vào game." Tối đa ba game gần đây hiện thành lối tắt; dấu ✕ để bỏ một game.

**Chọn game từ danh sách**: bấm vào ảnh game, hoặc bấm vào ô game khi đang trống. Các tab gồm **Gần đây**, **Đã ghim**, **Yêu thích trên Roblox** (game mà các tài khoản của bạn đã thích trên Roblox) và **Phổ biến** (khi lấy được danh sách). Bấm ☆ để ghim game.

**Job ID (tùy chọn)** là mã của đúng một server (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`), để nhiều tài khoản vào chung. Đổi sang game khác thì Job ID của game cũ tự bị xóa. Nhấn Enter trong ô này là **Vào game**.

> [!TIP]
> Muốn đưa nhiều tài khoản vào chung một server, chọn server ở **Server** (**Dùng**), hoặc dùng **Vào cùng server** trên một tài khoản đang ở đó. Sau đó chọn các tài khoản còn lại rồi bấm **Vào game**.

## Server

Nút **Server** (không có với server riêng) mở danh sách server của game.

<img src="../screenshots/vi/servers.png" width="760" alt="Danh sách server của một game: bạn bè trong game này kèm nút Dùng server này, rồi các server công khai với nút sắp xếp, Đủ chỗ cho 2 tài khoản và Server ngẫu nhiên">

- **Bạn bè trong game này**: bạn bè của các tài khoản của bạn đang ở trong game này, mỗi người ghi "bạn của *tài khoản*", kèm nút **Dùng server này**. Quyền riêng tư của họ có thể che đi.
- **Server công khai**: sắp xếp theo **Đông nhất**, **Vắng nhất**, **Ping thấp nhất** hoặc **FPS cao nhất**. **Đủ chỗ cho N tài khoản** (khi chọn từ 2 tài khoản) chỉ hiện server còn đủ chỗ cho tất cả. **Server ngẫu nhiên** chọn bừa một server.
- **Dùng** điền Job ID của server đó vào thanh chọn game ("Đã chọn server. Giờ chọn tài khoản rồi bấm Vào game."). Server đã đầy thì nút bị mờ.
- Ping hiển thị là ping trung bình của người chơi trong server, không phải ping từ máy bạn. Khi Roblox đang bận, danh sách tự thử lại, sau đó hiện nút **Thử lại**.
- Server không còn ai thì không hiện trong danh sách: người cuối cùng đã thoát nên server đang đóng, vào sẽ gặp lỗi "This experience has ended".
- Khi gửi nhiều tài khoản vào một server: tài khoản mở đầu tiên sẽ cho biết server có nhận hay không (bình thường chỉ khoảng 1 giây là được xếp vào server). Nếu sau 12 giây vẫn chưa vào được (server đã đầy hoặc đã đóng), tài khoản đó và các tài khoản còn lại sẽ chuyển sang một server khác còn đủ chỗ cho tất cả.

## Vào cùng người chơi

Nút hình người có mũi tên cạnh ô Job ID (cũng có trong Ctrl+K). Nút bị mờ cho tới khi bạn chọn tài khoản. Gõ **Tên đăng nhập Roblox hoặc ID người dùng** (dán link trang cá nhân cũng được) rồi bấm **Tìm và vào**: các tài khoản đã chọn sẽ vào đúng server người đó đang ở. Nếu không được, hộp thoại nói rõ lý do:

- không có tài khoản Roblox nào tên như vậy (cần tên đăng nhập, không phải tên hiển thị);
- người đó đang trong game nhưng quyền riêng tư không cho các tài khoản của bạn biết họ ở server nào;
- người đó đang offline, hoặc ẩn trạng thái online;
- người đó đang trên web hoặc app Roblox, hoặc ở Roblox Studio, chứ không ở trong game.

Bloxkeep chỉ dùng các tài khoản đã chọn để hỏi Roblox người đó đang ở đâu. Bloxkeep không bao giờ theo dõi hay kết bạn với ai để lách quyền riêng tư của họ.

## Vào game, Mở Roblox, Đóng và Dừng

| Nút | Làm gì |
|---|---|
| **Vào game** | Ghi **Chọn tài khoản** (bị mờ) khi chưa chọn ai. Có game trong ô thì ghi **Vào game** kèm số tài khoản. Ô game để trống thì ghi **Mở Roblox**: mỗi tài khoản mở Roblox ở màn hình chính. |
| **Mở Roblox mà không vào game** (biểu tượng màn hình) | Cũng mở Roblox ở màn hình chính, bất kể ô game có gì. Bị mờ khi chưa chọn tài khoản. |
| **Đóng Roblox của các tài khoản đã chọn** (biểu tượng nút nguồn) | Bị mờ nếu không tài khoản nào đã chọn đang mở trên máy này. |
| **Dừng** | Thay cho **Vào game** trong lúc đang mở: "Đã mở 2 tài khoản (dừng giữa chừng)." |

Một lượt mở diễn ra thế này:

- Các tài khoản mở lần lượt, cách nhau một khoảng **Chờ giữa các tài khoản** (**Cài đặt › Khởi chạy**, mặc định 8 giây; preset có thể đặt riêng).
- Tài khoản đang ở đúng game này thì được giữ nguyên. Tài khoản đang ở game khác thì Bloxkeep hỏi trước, rồi đóng và mở lại vào game mới.
- Tài khoản bị ban bị bỏ qua, có ghi trong kết quả. Nếu có tài khoản không mở được, hộp **Một số tài khoản chưa mở được** liệt kê lý do.
- Mở xong, các cửa sổ có thể tự xếp: xem [Cửa sổ và thống kê](windows-and-stats.vi.md).

## Thứ tự mở tài khoản

**⋯ Tùy chọn khác › Thứ tự mở tài khoản…** liệt kê mọi tài khoản: kéo tay nắm hoặc bấm mũi tên để đổi chỗ, rồi bấm **Xong**. **Vào game** mở các tài khoản đã chọn theo thứ tự này, tài khoản thêm sau xếp cuối. **Sắp xếp › Thứ tự mở** cho danh sách hiện theo đúng thứ tự đó. Preset có thứ tự riêng của nó.

## Lưu ý trước khi mở

Trước khi mở, Bloxkeep có thể dừng lại hỏi trong hộp thoại **Lưu ý trước khi mở**. Câu hỏi nào cũng có nút **Hủy** và **Vẫn mở**.

| Câu hỏi | Khi nào hiện | Không hỏi lại? |
|---|---|---|
| "*Tài khoản* sẽ thoát game đang chơi để vào game này." | Tài khoản đã chọn đang ở game hay server khác. | Không, luôn hỏi. |
| "Multi-Roblox đang tắt nên mỗi lần mở Roblox mới, cửa sổ cũ sẽ bị đóng." | Mở nhiều tài khoản, hoặc mở một tài khoản khi đang có cửa sổ Roblox khác, trong lúc Multi-Roblox tắt. Có thêm nút **Bật Multi-Roblox rồi chơi**. | Có |
| "Multi-Roblox chưa hoạt động: Roblox đã mở từ trước khi bật Multi-Roblox." | Multi-Roblox đã bật nhưng chưa sẵn sàng. Có thêm nút **Sửa Multi-Roblox**. | Không |
| Cảnh báo của Kiểm tra Multi-Roblox | Mở nhiều cửa sổ trên một bản Roblox mà chưa có hai cửa sổ nào cùng chơi được. | Có |
| "RAM chỉ còn trống *X*, mà mở thêm *N* cửa sổ Roblox cần khoảng *Y*." | Đang bật **Báo khi sắp hết RAM** và RAM sắp không đủ. | Có |
| "RAM vẫn còn trống, nhưng bộ nhớ ảo của Windows sắp đầy nên mở thêm *N* cửa sổ Roblox có thể bị văng." | Đang bật **Báo khi sắp hết RAM**, RAM vẫn đủ nhưng bộ nhớ ảo của Windows sắp không đủ. Xem [Gỡ rối](troubleshooting.vi.md#h-cửa-sổ-roblox-bị-văng-ngay-khi-mở-memory-dump-captured-hoặc-bloxstrap-báo-roblox-failed-to-launch). | Có |

<img src="../screenshots/vi/before-you-open.png" width="760" alt="Hộp Lưu ý trước khi mở: Multi-Roblox đang tắt, có ô Không hỏi lại và các nút Hủy, Vẫn mở, Bật Multi-Roblox rồi chơi">

- Ô **Không hỏi lại** chỉ hiện khi mọi cảnh báo trong hộp thoại đều bỏ qua được. Tích vào rồi bấm **Vẫn mở** thì từ đó những loại cảnh báo này không hỏi nữa, mỗi loại tính riêng. Cảnh báo đã bỏ qua cũng không chặn preset hẹn giờ.
- Muốn được hỏi lại: **Cài đặt › Thông báo › Câu hỏi xác nhận › Hỏi lại**. Dòng **Câu hỏi đã bỏ qua** cho biết đang có bao nhiêu câu. Về chính Multi-Roblox, xem [Multi-Roblox](multi-roblox.vi.md).

<details>
<summary>Chi tiết thêm</summary>

Các câu hỏi khác cũng có **Không hỏi lại**: xóa preset, **Đóng tất cả Roblox**, **Buộc tắt tất cả Roblox**, **Bật Multi-Roblox?**, **Bỏ các thay đổi?** và **Khóa Bloxkeep?**. Nút **Hỏi lại** cũng đưa các câu này trở lại. Riêng xóa tài khoản, khôi phục bản sao lưu và gỡ link Discord thì lúc nào cũng hỏi.

</details>

## Mở trang web

**Mở trang web** trong menu của tài khoản mở roblox.com với tài khoản đó đã đăng nhập sẵn, trong cửa sổ trình duyệt của Bloxkeep. Cửa sổ này có mỗi tài khoản một tab. Không dùng được với tài khoản Roblox VNG.

- Mỗi tab có phiên đăng nhập riêng, chỉ nằm trong bộ nhớ. Không có gì lọt vào trình duyệt của bạn.
- Mở một tài khoản đã có tab thì Bloxkeep chuyển sang tab đó. Mỗi tab có nút Quay lại, Tiến tới và Tải lại riêng.
- Dấu **+** ở cuối dải tab mở một tab trống. Đăng nhập trên roblox.com rồi bấm **Lưu vào Bloxkeep** để thêm tài khoản (hoặc làm mới phiên đăng nhập). Không lưu thì đóng tab là mất đăng nhập ở đó.
- Đóng cửa sổ là đóng hết các tab. Khóa Bloxkeep cũng đóng cửa sổ này.

| Phím | Làm gì |
|---|---|
| Ctrl+T | Mở tab trống. |
| Ctrl+W, ✕ hoặc bấm chuột giữa | Đóng tab. |
| Ctrl+Tab | Chuyển tab. |

<details>
<summary>Chi tiết thêm</summary>

Đăng xuất trong tab của một tài khoản đã lưu cũng sẽ đăng xuất tài khoản đó khỏi Bloxkeep, nên Bloxkeep chặn lại và giải thích. Chọn **Đóng tab** hoặc **Xem tiếp**.

</details>

← [Hướng dẫn](../guide.vi.md) · Tiếp: [Multi-Roblox](multi-roblox.vi.md) →
