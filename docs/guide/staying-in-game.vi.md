[English](staying-in-game.md)

# Giữ tài khoản trong game

Cách Bloxkeep giữ các tài khoản luôn ở trong game: Tự vào lại game, Chống AFK, kiểm tra đăng nhập mỗi ngày, các tùy chọn về RAM và chạy ẩn dưới khay hệ thống. Tất cả nằm ở **Cài đặt › Treo game**.

<img src="../screenshots/vi/settings-staying.png" width="760" alt="Cài đặt, trang Treo game: Tự vào lại game, Chống AFK, Kiểm tra đăng nhập mỗi ngày, Báo khi sắp hết RAM, Tiết kiệm RAM cho cửa sổ chạy nền và Chạy ẩn dưới khay hệ thống khi đóng">

## Làm nhanh

1. Mở **Cài đặt › Treo game**.
2. Ở dòng **Tự vào lại game**, chọn **Bật cho mọi tài khoản**. Muốn bật riêng cho một tài khoản thì dùng **⋯ › Tự vào lại game** của tài khoản đó.
3. Bật **Chống AFK**.
4. Để nguyên **Chạy ẩn dưới khay hệ thống khi đóng** ở trạng thái bật, để bấm ✕ chỉ thu Bloxkeep xuống khay chứ không thoát.

## Tự vào lại game

Khi Roblox của một tài khoản bị văng, bị kick hoặc mất kết nối, Bloxkeep mở lại đúng game đó (hoặc đúng server riêng), nhưng không chắc vào lại đúng server cũ.

| Bật cho | Ở đâu |
|---|---|
| Một tài khoản | **⋯ › Tự vào lại game** của tài khoản đó. |
| Mọi tài khoản | **Cài đặt › Treo game › Tự vào lại game › Bật cho mọi tài khoản** (khi chỗ bật chỗ tắt sẽ ghi "Bật cho 3/10 tài khoản"). |
| Cửa sổ của một preset | Tùy chọn **Tự vào lại game** của preset đó (xem [Preset](presets.vi.md)). |

<img src="../screenshots/vi/card-rejoin.png" width="760" alt="Các thẻ tài khoản: một thẻ đang tự vào lại game sau 5 phút kèm nút Hủy, một thẻ báo Mất kết nối vì bị kick do treo máy">

### Chờ bao lâu

| Tình huống | Bloxkeep làm gì |
|---|---|
| Roblox bị văng | Mở lại game sau 10 giây. |
| Cứ bị văng liên tục | Từ lần thứ ba, chờ 2, 5, 10 rồi 15 phút giữa các lần thử. Thẻ tài khoản hiện thời gian chờ và nút **Hủy**. |
| Ở trong game 10 phút | Đếm lại từ đầu. |
| Mất mạng | Thử lại mỗi 30 giây, tối đa 20 lần. |
| Máy vừa thức dậy | Chờ có mạng rồi mới vào lại. |

### Khi nào không vào lại

- Bạn tự thoát game, hoặc tự tay đóng cửa sổ Roblox.
- Tài khoản đăng nhập ở thiết bị khác.
- Tài khoản bị ban.
- Chính Roblox đóng cửa sổ vì có cửa sổ khác mở ra.
- Bloxkeep đang khóa. Mở khóa xong thì tính năng chạy lại.

<details>
<summary>Chi tiết thêm</summary>

- Nếu Roblox kết thúc phiên đăng nhập của tài khoản, tự vào lại game tự tắt cho tài khoản đó, và tự bật lại khi bạn đăng nhập lại.
- Khi Multi-Roblox chưa sẵn sàng, Bloxkeep không vào lại (vì cửa sổ mới sẽ đóng mất cửa sổ đang mở) và báo cho bạn. Xem [Multi-Roblox](multi-roblox.vi.md).

</details>

## Chống AFK

**Cài đặt › Treo game › Chống AFK** (preset có thể ghi đè). Roblox kick tài khoản sau 20 phút không có thao tác. Bloxkeep đưa từng cửa sổ Roblox đang ở trong game lên trước trong chốc lát, nhích chuột vài pixel rồi trả về, và đưa bạn về lại cửa sổ đang dùng. Không bấm phím, không click gì.

- Bắt đầu từ phút thứ 12 kể từ thao tác cuối của cửa sổ, vào phút đầu tiên không ai dùng máy.
- Cửa sổ còn 2 phút nữa là bị kick thì được xử lý ngay cả khi bạn đang dùng máy, vào lúc bạn ngừng tay 2 giây, nhưng không bao giờ chen ngang ứng dụng toàn màn hình.

> [!WARNING]
> Chống AFK không chạy được khi máy đang khóa.

Nếu tài khoản vẫn bị kick, Bloxkeep nói rõ lý do: bạn dùng máy liên tục, máy đang khóa, có ứng dụng toàn màn hình phía trước, hoặc Windows không cho đưa cửa sổ lên trước.

## Kiểm tra đăng nhập mỗi ngày

**Cài đặt › Treo game › Kiểm tra đăng nhập mỗi ngày** (bật sẵn). Roblox có thể kết thúc phiên đăng nhập mà không báo ai.

- Mỗi ngày một lần, khoảng 45 giây sau khi bạn mở khóa, Bloxkeep kiểm tra đăng nhập của từng tài khoản, lần lượt, cách nhau 2 giây. Bloxkeep cũng xem Roblox có báo bị ban không.
- Một thông báo duy nhất nêu tên các tài khoản cần đăng nhập lại. Dòng cài đặt có ghi "Lần kiểm tra gần nhất: …".
- Muốn kiểm tra ngay: **⋯ › Kiểm tra đăng nhập** trên một tài khoản, hoặc **⋯ Tùy chọn khác › Kiểm tra đăng nhập mọi tài khoản**.

## Báo khi sắp hết RAM

**Cài đặt › Treo game › Báo khi sắp hết RAM** (bật sẵn).

- Trước khi mở, hỏi lại nếu RAM trống không đủ cho các cửa sổ mới (hộp thoại **Lưu ý trước khi mở**, xem [Vào game](playing.vi.md)).
- Trong lúc chơi, báo khi máy dùng từ 92 % RAM trở lên, tối đa 5 phút một lần.
- Thanh RAM trong thẻ **Roblox trên máy này** ở thanh bên chuyển đỏ từ 90 % (xem [Cửa sổ và thống kê](windows-and-stats.vi.md)).

## Tiết kiệm RAM cho cửa sổ chạy nền

**Cài đặt › Treo game › Tiết kiệm RAM cho cửa sổ chạy nền** (tắt sẵn). Các cửa sổ Roblox bạn không chơi được đặt mức ưu tiên bộ nhớ thấp, nên Windows lấy RAM của chúng trước. Khi máy thật sự thiếu RAM, chúng được yêu cầu nhả bớt.

Cửa sổ ở phía trước được trả lại mức ưu tiên bình thường. Roblox có thể không chịu nhả; khi đó Bloxkeep bỏ qua bước này.

## Chạy ẩn dưới khay hệ thống

**Cài đặt › Treo game › Chạy ẩn dưới khay hệ thống khi đóng** (bật sẵn). Bấm ✕ thì Bloxkeep thu xuống khay hệ thống. Tự vào lại game, Multi-Roblox và preset hẹn giờ vẫn chạy.

| Bấm chuột phải vào biểu tượng ở khay | Tác dụng |
|---|---|
| **Cửa sổ nhỏ** | Mở cửa sổ nhỏ luôn nằm trên cùng. |
| **Mở Bloxkeep** | Hiện lại cửa sổ chính. |
| **Thoát Bloxkeep** | Thoát Bloxkeep. |

Tắt tùy chọn này thì bấm ✕ là thoát hẳn.

← [Hướng dẫn](../guide.vi.md) · Tiếp: [Cửa sổ và thống kê](windows-and-stats.vi.md) →
