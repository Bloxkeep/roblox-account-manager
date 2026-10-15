# Hướng dẫn sử dụng Bloxkeep

[English](guide.md) | Tiếng Việt

Tài liệu này đi qua mọi phần của Bloxkeep: tính năng nằm ở đâu, dùng để làm gì, và khi nào nó bị mờ hoặc không áp dụng. Đường dẫn trong menu được viết dạng **Cài đặt › Khởi chạy › Mở Roblox bằng**. "⋯" là nút ba chấm **Tùy chọn khác**.

## Mục lục

1. [Bắt đầu](#bắt-đầu)
2. [Thêm tài khoản](#thêm-tài-khoản)
3. [Danh sách tài khoản](#danh-sách-tài-khoản)
4. [Menu của tài khoản](#menu-của-tài-khoản)
5. [Vào game](#vào-game)
6. [Multi-Roblox](#multi-roblox)
7. [Preset và hẹn giờ](#preset-và-hẹn-giờ)
8. [Giữ tài khoản trong game](#giữ-tài-khoản-trong-game)
9. [Theo dõi game, cửa sổ và thống kê](#theo-dõi-game-cửa-sổ-và-thống-kê)
10. [Gửi lên Discord](#gửi-lên-discord)
11. [Cài đặt, từng trang một](#cài-đặt-từng-trang-một)
    - [Khởi chạy](#khởi-chạy) · [Phiên bản & đồ họa Roblox](#phiên-bản--đồ-họa-roblox) · [Treo game](#treo-game) · [Thông báo](#thông-báo) · [Discord](#discord) · [Giao diện](#giao-diện) · [Bảo mật](#bảo-mật) · [Sao lưu & dữ liệu](#sao-lưu--dữ-liệu) · [Cập nhật Bloxkeep](#cập-nhật-bloxkeep)
12. [Phím tắt](#phím-tắt)
13. [Gỡ rối và câu hỏi thường gặp](#gỡ-rối-và-câu-hỏi-thường-gặp)
14. [Quyền riêng tư](#quyền-riêng-tư)

---

## Bắt đầu

### Tải và cài Bloxkeep

Bloxkeep chạy trên Windows 10 hoặc 11, bản 64-bit. Chỉ tải ở [trang Releases](https://github.com/minhhoang0122/bloxkeep/releases/latest): bản lấy từ nơi khác có thể đã bị sửa để đánh cắp tài khoản. Có bộ cài thường, bộ cài offline và bản portable dạng zip; [README](../README.vi.md#tải-về-và-cài-đặt) nói nên chọn bản nào. Các file chưa có chữ ký số nên lần đầu Windows SmartScreen sẽ cảnh báo: bấm **More info** rồi **Run anyway**.

### Mật khẩu chủ

Lần đầu mở, Bloxkeep yêu cầu **Đặt mật khẩu chủ**. Mọi thứ bạn lưu trong Bloxkeep đều được mã hóa bằng mật khẩu này.

- Ít nhất 6 ký tự và không được chỉ toàn dấu cách. Thanh **Yếu / Tạm được / Mạnh** chỉ để tham khảo.
- Mật khẩu chủ **không lấy lại được**. Bloxkeep không lưu nó ở đâu cả, nên quên là mất hết tài khoản đã lưu. Cách duy nhất để lấy lại là một bản sao lưu mà bạn còn nhớ mật khẩu.
- Nên bật sao lưu sớm: **Cài đặt › Sao lưu & dữ liệu › Sao lưu vào thư mục** (xem [Sao lưu & dữ liệu](#sao-lưu--dữ-liệu)). Bản sao lưu mở bằng mật khẩu dùng lúc sao lưu.
- Chuyển sang máy mới? Ở màn hình đầu tiên, bấm **Khôi phục từ bản sao lưu** thay vì đặt mật khẩu mới.
- Muốn đổi mật khẩu sau này: **Cài đặt › Bảo mật**.

Nút EN / VI ở góc trên màn hình này dùng để đổi ngôn ngữ (cũng có ở **Cài đặt › Giao diện › Ngôn ngữ**).

### Mở khóa và khóa app

Mỗi lần khởi động, Bloxkeep hiện **Chào mừng trở lại**: nhập mật khẩu chủ rồi bấm **Mở khóa**. Nếu đang bật Caps Lock, app sẽ nhắc.

Nút **Khóa app** nằm cuối thanh bên. Khi khóa:

- các cửa sổ Roblox vẫn mở nguyên;
- tự vào lại game, preset hẹn giờ và tin nhắn Discord tạm dừng cho tới khi bạn mở khóa;
- lượt mở tài khoản đang chạy dở bị dừng, cửa sổ trình duyệt của Bloxkeep bị đóng (xem [Mở trang web](#mở-trang-web)), và mật khẩu hay mã 2FA bạn vừa sao chép bị xóa khỏi clipboard;
- mọi dữ liệu đã giải mã bị xóa khỏi bộ nhớ.

Nếu có thứ sẽ bị tạm dừng (cửa sổ đang bật tự vào lại game, preset hẹn giờ, Discord), Bloxkeep hỏi **Khóa Bloxkeep?** và liệt kê ra, ví dụ "Tự vào lại game của 3 tài khoản" hay "2 preset hẹn giờ". Tích **Không hỏi lại** nếu không muốn bị hỏi nữa.

Trong lúc khóa:

- Preset đến giờ chạy sẽ không chạy. Mở khóa xong, Bloxkeep báo một lần: "*Preset* không chạy lúc *giờ*: Bloxkeep đang khóa." Lần đó không được chạy bù.
- Preset đến giờ dừng vẫn đóng cửa sổ đúng giờ.
- Màn hình khóa nhắc rằng các cửa sổ Roblox vẫn đang mở.

---

## Thêm tài khoản

Bấm **Thêm tài khoản** (góc trên bên phải tab **Tài khoản**) để mở hộp **Đăng nhập Roblox**. Mũi tên bên cạnh (**Cách thêm khác**) có: **Đăng nhập bằng điện thoại**, **Đăng nhập bằng trình duyệt**, **Dán cookie**, **Nhập từ file dữ liệu Bloxkeep khác…** và **Mở roblox.com**. Khi chưa có tài khoản nào, danh sách trống cũng hiện sẵn các cách này.

| Cách | Dùng khi | Cần gì |
|---|---|---|
| **Đăng nhập bằng điện thoại** (Nên dùng) | Hầu như lúc nào cũng nên dùng. Không cần mật khẩu, không phải giải đố. | Tài khoản đã đăng nhập sẵn trong app Roblox trên điện thoại. |
| **Dùng tên đăng nhập & mật khẩu** | Không có điện thoại bên cạnh. | Roblox có thể bắt giải đố hoặc nhập mã 2 bước. |
| **Đăng nhập bằng trình duyệt** | Các cách khác không được, hoặc Roblox đòi xác minh thêm. | Vào được www.roblox.com. Không dùng cho Roblox VNG. |
| **Dán cookie** | Bạn đã có sẵn cookie của tài khoản. | Cookie `.ROBLOSECURITY`. |
| **Mở roblox.com** rồi **Lưu vào Bloxkeep** | Chủ yếu cần dùng trang web, nhiều tài khoản cùng lúc. | Không cần gì thêm. |
| **Nhập từ: File Bloxkeep** | Chuyển tài khoản từ máy khác hoặc từ bản sao lưu. | Mật khẩu chủ của file đó. |

Thêm một tài khoản đã có trong danh sách thì không bị trùng: Bloxkeep chỉ cập nhật phiên đăng nhập của nó ("*tên* đã có sẵn nên chỉ cập nhật phiên đăng nhập.").

### Đăng nhập bằng điện thoại

1. Mở app Roblox trên điện thoại, đăng nhập tài khoản cần thêm.
2. Vào **Cài đặt → Đăng nhập nhanh**.
3. Nhập mã hoặc quét QR, rồi xác nhận.

Hộp thoại hiện mã, mã QR và dòng "Đang chờ xác nhận · hết hạn sau *p:gg*".

- Khi điện thoại đã nhập mã: "*tên* đã nhập mã. Hãy xác nhận trên thiết bị đó." Xác nhận trên điện thoại là xong.
- Thêm xong một tài khoản thì có ngay mã mới cho tài khoản tiếp theo ("Đã thêm: … Dùng mã mới cho tài khoản tiếp theo."). Xong hết thì bấm **Xong**.
- Mã tự làm mới vài lần. Sau đó app báo "Mã đã hết hạn.": bấm **Lấy mã mới**. Gặp "Đã từ chối trên thiết bị." cũng bấm **Lấy mã mới**.
- Hai tab **Roblox** / **Roblox VNG** để chọn loại tài khoản. Với Roblox VNG, xác nhận trong app Roblox VNG.
- Nút **Đăng nhập bằng trình duyệt** ở dưới chuyển sang cách dùng trình duyệt (không có ở tab Roblox VNG).

Nút **Đăng nhập** trên thẻ của tài khoản bị đăng xuất cũng mở hộp thoại này, với dòng "Đang đăng nhập lại *tên*." ở trên cùng.

### Tên đăng nhập và mật khẩu

Trong hộp **Đăng nhập Roblox**, điền **Tên đăng nhập**, **Mật khẩu** rồi bấm **Đăng nhập**.

- Mật khẩu chỉ được gửi tới Roblox. **Bloxkeep không lưu lại.** Vì vậy sau khi đăng nhập, **Sao chép mật khẩu** không có gì để sao chép (xem [Sao chép mật khẩu](#sao-chép-mật-khẩu-và-cách-lưu-mật-khẩu)).
- **Xác minh 2 bước**: nhập mã 6 số. Các tab (**App xác thực**, **Email**, **Tin nhắn SMS**) là những cách mà tài khoản có. Với email và SMS, Roblox tự gửi mã; **Gửi mã** / **Gửi lại mã** để gửi lần nữa (Gửi lại phải chờ 30 giây). Nếu khóa app xác thực của tài khoản đã lưu trong Bloxkeep, mã được điền sẵn ("Đã tự điền bằng khóa xác thực đã lưu."), bạn chỉ cần bấm **Xác minh**. Nhập sai nhiều lần thì nút **Xác minh** đếm ngược rồi mới cho thử tiếp.
- **Xác minh nhanh** (giải đố): bấm **Giải câu đố** và làm trong cửa sổ nhỏ hiện ra. Roblox có thể bắt giải thêm câu nữa.
- **Sắp xong**: Roblox đã cho vào nhưng Bloxkeep chưa tải được tài khoản. Bấm **Thử lại**, mật khẩu không bị gửi lần nữa.
- **Quay lại** để về màn hình đầu.
- Có thể thêm nhiều tài khoản liên tiếp: thêm xong một cái, ô nhập được xóa trống cho tài khoản sau.

Đôi khi Roblox không cho đăng nhập kiểu này (tài khoản bị khóa để bảo vệ, tài khoản dùng passkey hoặc khóa bảo mật, hoặc Roblox chặn lần đăng nhập). Khi đó thông báo sẽ bảo bạn đăng nhập bằng điện thoại hoặc trình duyệt.

### Đăng nhập bằng trình duyệt

Một cửa sổ **Đăng nhập Roblox** mở trang đăng nhập chính thức của roblox.com. Đăng nhập xong, Bloxkeep lưu tài khoản và mở trang mới cho tài khoản tiếp theo. Xong hết thì bấm **Xong**. Bloxkeep không đọc mật khẩu bạn gõ ở đây. Cách này cần vào được www.roblox.com và cần Microsoft Edge WebView2 (bộ cài đã tự cài). Không có cho Roblox VNG.

### Dán cookie

Cookie là một mẩu dữ liệu nhỏ mà roblox.com lưu trong trình duyệt để nhớ bạn đã đăng nhập. Cookie `.ROBLOSECURITY` chính là chìa khóa của tài khoản: ai có nó là dùng được tài khoản đó. Đừng bao giờ đưa cho ai.

**Thêm tài khoản › Cách thêm khác › Dán cookie** mở hộp **Thêm tài khoản bằng cookie**:

- **Cookie .ROBLOSECURITY** (bắt buộc);
- **Tên gợi nhớ**, **Nhóm**, **Mật khẩu Roblox** (không bắt buộc, chỉ để bạn xem lại), **Ghi chú**.

Bấm **Thêm tài khoản**. Khóa app xác thực thì thêm sau, ở mục **Sửa**.

### Mở roblox.com rồi Lưu vào Bloxkeep

**Thêm tài khoản › Cách thêm khác › Mở roblox.com** (hoặc **Chỉ cần dùng trang web? Mở roblox.com** ở danh sách trống) mở một tab trống trong cửa sổ trình duyệt của Bloxkeep, ở trang đăng nhập roblox.com.

- Mỗi tab là một tài khoản riêng, với phiên đăng nhập riêng chỉ nằm trong bộ nhớ. Không có gì lọt vào Chrome hay Edge của bạn.
- Đăng nhập trên trang web, rồi bấm **Lưu vào Bloxkeep**: tài khoản vào danh sách (hoặc được làm mới phiên đăng nhập), và lần sau mở là vào thẳng.
- Không lưu thì đóng tab là mất đăng nhập ở đó.
- Dấu **+** ở cuối dải tab mở thêm một tab trống.

### Nhập từ file Bloxkeep khác

Dùng để lấy tài khoản từ file `vault.rmv` của máy khác, hoặc từ file `Bloxkeep-backup-…`. Có ba chỗ mở: **Thêm tài khoản › Cách thêm khác › Nhập từ file dữ liệu Bloxkeep khác…**, **Nhập từ: File Bloxkeep** trong hộp **Đăng nhập Roblox**, và **Cài đặt › Sao lưu & dữ liệu › Nhập tài khoản từ file dữ liệu Bloxkeep › Nhập…**.

1. Bấm **Chọn file…** và chọn file `.rmv`.
2. Nhập **Mật khẩu chủ của file này** rồi bấm **Mở**.
3. Tích những tài khoản muốn lấy. Mỗi dòng ghi **Mới** hay **Đã có**, và tài khoản còn đăng nhập, cần đăng nhập lại, chưa kiểm tra hay đã lưu mật khẩu. **Chọn hết** / **Bỏ chọn** để tích hoặc bỏ tất cả.
4. Muốn lấy cả preset thì tích **Nhập cả preset (N)**.
5. Bấm **Nhập**.

Cần biết:

- File gốc vẫn giữ nguyên. Tài khoản lấy sang được khóa bằng mật khẩu chủ của Bloxkeep trên máy này.
- Với tài khoản đã có, Bloxkeep chỉ thay phiên đăng nhập nếu phiên cũ không còn dùng được. Tên, nhóm và ghi chú bạn đặt vẫn giữ nguyên.
- Preset nhập vào có hẹn giờ bị tắt, và tài khoản chưa có ở đây bị bỏ ra khỏi preset. Lưu được tối đa 50 preset.
- Nhập sai mật khẩu ba lần thì nút **Mở** đếm ngược rồi mới cho thử tiếp. Không nhập được chính file dữ liệu bạn đang dùng.
- Muốn thay toàn bộ dữ liệu (máy mới) thì dùng **Khôi phục** (xem [Sao lưu & dữ liệu](#sao-lưu--dữ-liệu)).

### Tài khoản từ trình quản lý khác

Bloxkeep không đọc được file của các trình quản lý tài khoản khác. Hãy thêm những tài khoản đó bằng một trong các cách ở trên. Nếu công cụ kia cho xem cookie của tài khoản thì dùng **Dán cookie** là được.

### Tài khoản Roblox VNG

Thêm ở tab **Roblox VNG** của hộp **Đăng nhập Roblox** (tên đăng nhập và mật khẩu) hoặc của **Đăng nhập bằng điện thoại** (xác nhận trong app Roblox VNG). Không có cách đăng nhập bằng trình duyệt cho loại này. Tài khoản VNG có nhãn **VNG** ("Tài khoản Roblox VNG (Việt Nam)").

Giới hạn hiện tại:

- Bloxkeep chưa mở được game cho tài khoản VNG. Nút **Vào game** của chúng bị mờ ("Bloxkeep chưa mở được Roblox VNG.").
- Khi mở nhiều tài khoản một lúc, tài khoản VNG bị bỏ qua và kết quả có ghi rõ.
- **Mở trang web** cũng bị mờ với tài khoản VNG.

---

## Danh sách tài khoản

### Phần đầu trang

Dưới tiêu đề **Tài khoản** có dòng "*N* tài khoản · *P* đang chơi · *O* online" và link **Chọn tất cả**. Khi đã chọn tài khoản, dòng này thành "Đã chọn *N*. **Bỏ chọn**". Nếu ô tìm kiếm hay bộ lọc đang che bớt tài khoản đã chọn, app ghi rõ số tài khoản bị ẩn: **Vào game** và **Xóa** vẫn tính cả chúng.

Bên phải là ô tìm kiếm, nút **Làm mới trạng thái online** (mũi tên vòng), nút **⋯ Tùy chọn khác** và nút **Thêm tài khoản** kèm mũi tên.

Menu **⋯ Tùy chọn khác**:

| Mục | Bị mờ khi |
|---|---|
| **Kiểm tra đăng nhập mọi tài khoản** | Chưa có tài khoản nào, hoặc đang có lượt kiểm tra chạy. |
| **Chọn tất cả** (Ctrl+A) | Chưa có tài khoản nào. |
| **Xếp cửa sổ Roblox** | Không bao giờ. Không có cửa sổ nào thì app báo "Không có cửa sổ Roblox nào để xếp." |
| **Thứ tự mở tài khoản…** | Có ít hơn 2 tài khoản. |
| **Đóng tất cả Roblox** | Không bao giờ. Có hỏi trước. |
| **Buộc tắt tất cả Roblox** | Không bao giờ. Có hỏi trước. |
| **Hướng dẫn sử dụng** | Không bao giờ. Mở trang này. |

### Thẻ tài khoản cho biết gì

Nhãn ở góc thẻ:

| Nhãn | Nghĩa là |
|---|---|
| **Trên máy này** | Tài khoản đang mở Roblox trên máy này. Rê chuột lên để xem PID. |
| **Đang mở** | Bloxkeep đang mở Roblox cho tài khoản này. |
| **Bị ban** | Roblox báo tài khoản bị ban. **Vào game** và preset sẽ bỏ qua nó. |
| **Cần đăng nhập lại** | Roblox đã kết thúc phiên đăng nhập đã lưu. Bấm **Đăng nhập**. |

Ô trạng thái ở giữa thẻ:

| Trạng thái | Nghĩa là |
|---|---|
| **Đang mở Roblox…** | Đang chờ cửa sổ game, sau đó hiện tên game. |
| **Chờ vào lại · máy này** | Tự vào lại game đang chờ tới lượt: "Sau 2 phút", "Sắp vào lại". Bấm **Hủy** để thôi. |
| **Đang mở lại · máy này** | Preset sắp mở lại cửa sổ này theo lịch. |
| **Đang chơi** / **Đang vào game** | Đang ở trong game (hoặc đang vào) trên máy này, kèm tên game. |
| **Bị văng khỏi server** | Cửa sổ vẫn mở nhưng đã ra khỏi server. Lý do: **Bị kick vì AFK**, **Bị game kick**, **Đã đăng nhập ở thiết bị khác**, **Rớt mạng**, **Không vào được server**, hoặc **Mã** *N*. Roblox không nói lý do thì ghi "Roblox vẫn đang mở". |
| **Roblox đang mở** · Chưa vào game | Roblox đang ở màn hình chính. |
| **Đã đăng xuất** | Roblox đang mở nhưng không còn đăng nhập tài khoản này (đã đăng xuất ngay trong cửa sổ Roblox). |
| **Đang chơi · thiết bị khác** | Đang chơi trên thiết bị khác. Ghi "Không xem được game (do cài đặt quyền riêng tư)" nếu quyền riêng tư che mất tên game. |
| **Online** | Đang trên web hoặc app Roblox, không ở trong game. |
| **Roblox Studio** | Đang làm game. |
| **Offline** | "Online lần cuối …", hoặc "Không hoạt động". |
| **Cần đăng nhập lại** | "Đăng nhập lại để dùng tiếp". |
| **Chưa rõ trạng thái** | Chưa tải xong ("Đang cập nhật…"). |

Khi tài khoản đang mở trên máy này, thẻ còn hiện **RAM đang dùng**, **Thời gian đã chơi** và **PID** (bấm vào PID để sao chép). Bên dưới là nhãn **VNG**, nhóm của tài khoản, biểu tượng vòng lặp khi **Đang bật tự vào lại game**, và lần cuối mở từ Bloxkeep ("Mở 5 phút trước" hoặc "Chưa mở lần nào"). Ghi chú (nếu có) nằm ở dòng cuối.

Các nút dưới cùng:

- **Vào game**; đổi thành **Đóng** khi tài khoản đang mở hoặc đang được mở, **Hủy** khi đang chờ vào lại, **Đăng nhập** khi cần đăng nhập lại. **Vào game** bị mờ với tài khoản Roblox VNG và khi tài khoản đang được mở.
- **Sửa (F2)**.
- **⋯ Tùy chọn khác** (xem [Menu của tài khoản](#menu-của-tài-khoản)).

Trạng thái tự làm mới mỗi 30 giây; **Làm mới trạng thái online** để làm mới ngay. Trạng thái của Roblox với thiết bị khác có thể chậm vài phút.

### Chọn tài khoản

- **Bấm** vào thẻ để chọn, bấm lần nữa để bỏ chọn. Mỗi lần bấm là thêm vào nhóm đang chọn.
- **Shift + bấm** để chọn liền một dãy, từ thẻ bấm trước đó tới thẻ này.
- **Ctrl+A** hoặc **Chọn tất cả**: chọn mọi tài khoản đang hiện (chỉ những tài khoản mà tìm kiếm và bộ lọc đang cho hiện).
- **Esc** hoặc **Bỏ chọn**: bỏ chọn hết.
- **Bấm đúp** vào thẻ để vào game riêng tài khoản đó.
- Dùng bàn phím: phím mũi tên, Home, End để đi giữa các thẻ; **Space** để chọn thẻ đang trỏ tới.

### Tìm, lọc, nhóm, sắp xếp và kiểu xem

- **Tìm** (Ctrl+F): theo tên, tên đăng nhập (gõ `@tên` cũng được), tên hiển thị, tên gợi nhớ, nhóm, ghi chú, game đang chơi và ID người dùng. Không cần gõ dấu: "hoang" vẫn tìm ra "Hoàng".
- **Bộ lọc**, mỗi nút có kèm số đếm: **Tất cả**, **Cần đăng nhập lại** và **Bị ban** (chỉ hiện khi có tài khoản như vậy), **Đang chơi**, **Online**, **Offline**, **Trên máy này** (chỉ hiện khi có tài khoản đang mở ở máy này).
- Menu **Nhóm** ("Nhóm: Tất cả") hiện ra khi có tài khoản được xếp nhóm. Đặt nhóm ở **Sửa › Nhóm**.
- **Sắp xếp** (khi có từ 2 tài khoản): **Tên**, **Trạng thái**, **Chơi gần đây**, **Thứ tự mở**.
- **Dạng thẻ** / **Dạng danh sách**: hai nút đổi kiểu xem. Dạng danh sách có đủ nút và menu như dạng thẻ.
- Kiểu xem và cách sắp xếp được nhớ lại.
- Không có kết quả: app hiện **Không có tài khoản phù hợp**, kèm **Xóa tìm kiếm** hoặc **Hiện mọi tài khoản**.

### Ẩn tên tài khoản

**Cài đặt › Giao diện › Ẩn tên tài khoản** thay tên thật bằng "Tài khoản 1", "Tài khoản 2"… (theo vị trí trong danh sách) ở mọi chỗ, kể cả tiêu đề cửa sổ Roblox, tiện khi chụp màn hình hay livestream. Tên đăng nhập @ cũng ẩn đi, mục **Sửa** không hiện ID người dùng, và ô **Tên gợi nhớ** không sửa được trong lúc này ("Đang ẩn vì bật Ẩn tên tài khoản"). Ô tìm kiếm vẫn tìm được theo tên thật. Tin nhắn Discord có công tắc riêng (xem [Tùy chỉnh tin nhắn](#tùy-chỉnh-tin-nhắn)).

---

## Menu của tài khoản

Mở bằng nút **⋯ Tùy chọn khác** trên thẻ, hoặc bấm chuột phải vào tài khoản. Nếu tài khoản đó chưa được chọn, nó trở thành tài khoản duy nhất được chọn. Nếu nó nằm trong nhóm đang chọn, menu áp dụng cho mọi tài khoản đã chọn.

| Mục | Làm gì | Bị mờ hoặc không có khi |
|---|---|---|
| **Vào game** / **Vào game (N tài khoản)** (Enter) | Mở game đang có trong ô game (ô trống thì mở màn hình chính Roblox), theo [thứ tự mở](#thứ-tự-mở-tài-khoản). | — |
| **Mở Roblox (không vào game)** | Mở Roblox ở màn hình chính, mỗi tài khoản một cửa sổ. | — |
| **Đóng Roblox** | Đóng cửa sổ Roblox như bình thường. | Không tài khoản nào đã chọn đang mở trên máy này. |
| **Buộc tắt** | Tắt Roblox ngay, dùng khi cửa sổ bị treo. Những gì chưa lưu trong game sẽ mất. | Giống **Đóng Roblox**. |
| **Sửa…** (F2) | Tên gợi nhớ, nhóm, mật khẩu, khóa app xác thực, ghi chú. | Đang chọn nhiều tài khoản. |
| **Kiểm tra đăng nhập** | Kiểm tra phiên đăng nhập đã lưu và xem Roblox có báo bị ban không. Kết quả cho biết bao nhiêu tài khoản cần đăng nhập lại, có nút **Xem** để chuyển sang bộ lọc **Cần đăng nhập lại**. | — |
| **Vào cùng server** | Đưa game và server (Job ID) của tài khoản này vào thanh chọn game. Sau đó chọn các tài khoản khác rồi bấm **Vào game**: tất cả vào chung một server. | Đang chọn nhiều tài khoản, hoặc không biết server (tài khoản không ở trong game, hoặc quyền riêng tư che mất). |
| **Tự vào lại game** | Bật hoặc tắt tự vào lại game cho các tài khoản này. Dấu tích cho biết đang bật hay tắt. | — (nếu preset đang quyết định cho cửa sổ đang mở, mục này ghi rõ) |
| **Dùng cấu hình Roblox** | Cho các tài khoản này dùng **Cấu hình Roblox** (đồ họa, FPS, âm lượng). | Chỉ có khi **Cài đặt › Phiên bản & đồ họa Roblox › Áp dụng cho** là **Tài khoản đã chọn**. |
| **Trang cá nhân Roblox** | Mở trang cá nhân của tài khoản trên roblox.com bằng trình duyệt của bạn. | Đang chọn nhiều tài khoản, hoặc không có ID người dùng. |
| **Mở trang web** | Mở roblox.com với tài khoản này đã đăng nhập sẵn, trong một tab của cửa sổ trình duyệt Bloxkeep. | Đang chọn nhiều tài khoản, hoặc là tài khoản Roblox VNG. |
| **Sao chép tên đăng nhập** | Sao chép tên đăng nhập. | Đang chọn nhiều tài khoản. |
| **Sao chép mật khẩu** | Sao chép mật khẩu đã lưu. | Đang chọn nhiều tài khoản. Chưa lưu mật khẩu thì app giải thích lý do (xem dưới). |
| **Sao chép mã 2FA** | Sao chép mã 6 số hiện tại. | Đang chọn nhiều tài khoản. Chưa lưu khóa thì app giải thích lý do (xem dưới). |
| **Xóa** / **Xóa N tài khoản** (Del) | Xóa tài khoản khỏi Bloxkeep sau khi hỏi lại. Không thể hoàn tác. | — |

### Sao chép mật khẩu và cách lưu mật khẩu

Dù bạn đăng nhập bằng cách nào, Bloxkeep cũng không giữ lại mật khẩu. Vì vậy, nếu bạn chưa tự lưu, **Sao chép mật khẩu** sẽ báo: "Tài khoản này chưa lưu mật khẩu: lúc đăng nhập, Bloxkeep không giữ lại mật khẩu. Muốn sao chép được, hãy nhập mật khẩu ở mục Sửa (F2)." Rê chuột lên mục này cũng thấy dòng giải thích đó.

Cách lưu mật khẩu:

1. Chọn tài khoản rồi bấm **F2** (hoặc nút bút chì, hoặc **Sửa…** trong menu).
2. Gõ mật khẩu vào ô **Mật khẩu Roblox** (không bắt buộc, chỉ để bạn xem lại).
3. Bấm **Lưu**.

Từ đó ô này hiện "•••••••• (đã lưu)". Nút con mắt để xem lại mật khẩu; gõ mật khẩu mới là thay luôn. Mật khẩu nằm trong kho đã mã hóa và chỉ để bạn dùng: Bloxkeep không dùng nó để đăng nhập.

Mật khẩu đã sao chép tự xóa khỏi clipboard sau 30 giây và không lưu vào lịch sử clipboard của Windows. Đừng gửi cho ai.

### Sao chép mã 2FA và khóa app xác thực

Mã 2FA là mã 6 số mà app xác thực tạo ra cho bước xác minh 2 bước, cứ 30 giây đổi một lần. Bloxkeep tạo được đúng mã đó nếu bạn lưu khóa app xác thực của tài khoản:

1. **Sửa (F2) › Khóa app xác thực** (không bắt buộc).
2. Dán khóa (chữ A–Z và số 2–7) hoặc link `otpauth://`.
3. Bloxkeep hiện "Mã hiện tại: … Kiểm tra xem app xác thực có hiện đúng mã này không." Nếu trùng thì bấm **Lưu**.

Từ đó, mục **Sửa** hiện mã đang chạy và số giây còn lại, kèm nút **Sao chép** và **Gỡ** (khóa chỉ bị gỡ khi bạn lưu; bấm **Hoàn tác** để giữ lại). **Sao chép mã 2FA** trong menu sẽ sao chép mã ("Đã sao chép mã 2FA. Mã đổi sau *N* giây."), và khi đăng nhập bằng mật khẩu, mã được điền sẵn.

Chưa lưu khóa thì **Sao chép mã 2FA** báo: "Tài khoản này chưa lưu khóa app xác thực. Thêm khóa ở mục Sửa (F2) để sao chép mã 2 bước ở đây."

Lưu khóa trong Bloxkeep thì ai có mật khẩu chủ của bạn cũng tạo được mã. Chỉ lưu khi bạn chấp nhận điều đó. Bloxkeep chỉ tạo mã theo thời gian, là loại Roblox dùng.

### Sửa (F2)

Hộp **Sửa tài khoản** hiện ảnh đại diện, tên, @tên đăng nhập và ID người dùng, rồi tới các ô: **Tên gợi nhớ**, **Nhóm** (có gợi ý các nhóm bạn đã đặt), **Mật khẩu Roblox**, **Khóa app xác thực** và **Ghi chú**. Bấm **Lưu**.

Ở đây không có ô cookie. Muốn làm mới phiên đăng nhập, bấm **Đăng nhập** trên thẻ, hoặc thêm lại chính tài khoản đó bằng bất kỳ cách nào: Bloxkeep chỉ cập nhật chứ không tạo bản trùng.

Nếu đã gõ gì đó mà bấm Esc hoặc ✕, Bloxkeep hỏi **Bỏ các thay đổi?** (có thể tích **Không hỏi lại**).

### Mở trang web

**Mở trang web** mở roblox.com với tài khoản đã đăng nhập sẵn, trong một tab của cửa sổ trình duyệt Bloxkeep.

- Mỗi tài khoản có một tab riêng, với phiên đăng nhập riêng chỉ nằm trong bộ nhớ. Không có gì lọt vào trình duyệt của bạn.
- Mở một tài khoản đã có tab thì Bloxkeep chuyển sang tab đó.
- Dấu **+** ở cuối dải tab mở một tab trống để đăng nhập trên roblox.com, rồi bấm **Lưu vào Bloxkeep**.
- Mỗi tab có nút Quay lại, Tiến tới và Tải lại riêng.
- Đóng tab bằng dấu ✕ của nó, bấm chuột giữa, hoặc Ctrl+W. Ctrl+T mở tab trống, Ctrl+Tab chuyển tab.
- Đăng xuất trong tab của một tài khoản đã lưu cũng sẽ đăng xuất tài khoản đó khỏi Bloxkeep, nên Bloxkeep chặn lại và giải thích: chọn **Đóng tab** hoặc **Xem tiếp**.
- Đóng cửa sổ là đóng hết các tab. Khóa Bloxkeep cũng đóng cửa sổ này.

### Xóa

**Xóa** luôn hỏi lại và liệt kê các tài khoản: "Không thể hoàn tác." Câu hỏi này không tắt được. Thống kê vẫn giữ thời gian chơi của tài khoản đã xóa, ghi là "Tài khoản đã xóa".

---

## Vào game

### Thanh chọn game

Thanh ở cuối tab **Tài khoản** có, từ trái sang phải: ảnh game (**Chọn game**), ô game, ô **Job ID (tùy chọn)**, nút **Vào cùng người chơi…**, rồi các nút **Xếp cửa sổ**, **Mở Roblox mà không vào game**, **Đóng**, **Preset** và **Vào game**.

**Ô game** ("Tìm game, hoặc dán link / Place ID…", Ctrl+L hoặc `/`) nhận:

- tên game: chọn game trong danh sách hiện ra;
- Place ID;
- link `roblox.com/games/…`, kể cả link server riêng;
- link chia sẻ server riêng (`roblox.com/share?…type=Server`);
- link `roblox://`.

Gõ tên mà không chọn trong danh sách thì không tính: "Hãy chọn game trong kết quả tìm kiếm, hoặc dán link game hay Place ID."

Dưới ô game là tên game, người làm game, số người đang chơi và nút **Server**. Server riêng có nhãn **Server riêng**; link chia sẻ ghi "Link server riêng: Bloxkeep sẽ mở link này khi vào game."; game có Job ID ghi **Server cụ thể**. Tối đa ba game gần đây hiện thành lối tắt; dấu ✕ để bỏ một game khỏi đó.

**Chọn game từ danh sách**: bấm vào ảnh game, hoặc bấm vào ô game khi đang trống. Các tab gồm **Gần đây**, **Đã ghim**, **Yêu thích trên Roblox** (game mà các tài khoản của bạn đã thích trên Roblox) và **Phổ biến** (khi lấy được danh sách). Bấm ☆ để ghim game.

**Job ID (tùy chọn)**: mã của đúng một server (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`), để nhiều tài khoản vào chung. Đổi sang game khác thì Job ID của game cũ tự bị xóa. Nhấn Enter trong ô này là **Vào game**.

### Server

Nút **Server** (không có với server riêng) mở danh sách server của game.

- **Bạn bè trong game này**: bạn bè của các tài khoản của bạn đang ở trong game này, mỗi người ghi "bạn của *tài khoản*", kèm nút **Dùng server này**. Quyền riêng tư của họ có thể che đi.
- **Server công khai**: sắp xếp theo **Đông nhất**, **Vắng nhất**, **Ping thấp nhất** hoặc **FPS cao nhất**. **Đủ chỗ cho N tài khoản** (khi chọn từ 2 tài khoản) chỉ hiện server còn đủ chỗ cho tất cả. **Server ngẫu nhiên** chọn bừa một server. **Xem thêm** để tải thêm.
- **Dùng** điền Job ID của server đó vào thanh chọn game: "Đã chọn server. Giờ chọn tài khoản rồi bấm Vào game." Server đã đầy thì nút bị mờ ("Server này đã đầy").
- Ping hiển thị là ping trung bình của người chơi trong server, không phải ping từ máy bạn.
- Khi Roblox đang bận, danh sách tự thử lại, sau đó hiện nút **Thử lại**.

### Vào cùng người chơi

Nút hình người có mũi tên cạnh ô Job ID (cũng có trong Ctrl+K). Nút bị mờ cho tới khi bạn chọn tài khoản.

Gõ **Tên đăng nhập Roblox hoặc ID người dùng** (dán link trang cá nhân cũng được) rồi bấm **Tìm và vào**. Các tên đã tìm gần đây được liệt kê; dấu ✕ để xóa một tên. Các tài khoản đã chọn sẽ vào đúng server người đó đang ở. Nếu không được, hộp thoại nói rõ lý do:

- không có tài khoản Roblox nào tên như vậy (cần tên đăng nhập, không phải tên hiển thị);
- người đó đang trong game nhưng quyền riêng tư không cho các tài khoản của bạn biết họ ở server nào;
- người đó đang offline, hoặc ẩn trạng thái online;
- người đó đang trên web hoặc app Roblox, hoặc ở Roblox Studio, chứ không ở trong game.

Bloxkeep chỉ dùng các tài khoản đã chọn để hỏi Roblox người đó đang ở đâu. Bloxkeep không bao giờ theo dõi hay kết bạn với ai để lách quyền riêng tư của họ.

### Vào game, Mở Roblox, Đóng và Dừng

- Nút **Vào game** ghi **Chọn tài khoản** (bị mờ) khi chưa chọn ai. Có game trong ô thì ghi **Vào game** kèm số tài khoản. Ô game để trống thì ghi **Mở Roblox**: mỗi tài khoản đã chọn mở Roblox ở màn hình chính.
- **Mở Roblox mà không vào game** (biểu tượng màn hình) làm đúng như vậy, bất kể ô game có gì. Bị mờ khi chưa chọn tài khoản.
- **Đóng Roblox của các tài khoản đã chọn** (biểu tượng nút nguồn) bị mờ nếu không tài khoản nào đã chọn đang mở trên máy này.
- Trong lúc đang mở, nút **Vào game** đổi thành **Dừng**: "Đã mở 2 tài khoản (dừng giữa chừng)."

Một lượt mở diễn ra thế này:

- Các tài khoản mở lần lượt, cách nhau một khoảng **Chờ giữa các tài khoản** (**Cài đặt › Khởi chạy**, mặc định 8 giây; preset có thể đặt riêng).
- Tài khoản đang ở đúng game này thì được giữ nguyên ("1 tài khoản đã mở sẵn nên được giữ nguyên.").
- Tài khoản đang ở game khác thì Bloxkeep hỏi trước, rồi đóng và mở lại vào game mới (Roblox sẽ kick cửa sổ thứ hai của cùng một tài khoản).
- Tài khoản bị ban và tài khoản Roblox VNG bị bỏ qua, có ghi trong kết quả.
- Nếu có tài khoản không mở được, hộp **Một số tài khoản chưa mở được** liệt kê lý do.
- Mở xong, các cửa sổ có thể tự xếp (xem [Xếp cửa sổ](#xếp-cửa-sổ)).

### Thứ tự mở tài khoản

**⋯ Tùy chọn khác › Thứ tự mở tài khoản…** liệt kê mọi tài khoản: kéo tay nắm hoặc bấm mũi tên để đổi chỗ, rồi bấm **Xong**. **Vào game** sẽ mở các tài khoản đã chọn theo thứ tự này; tài khoản thêm sau xếp cuối. **Sắp xếp › Thứ tự mở** cho danh sách hiện theo đúng thứ tự đó. Preset có thứ tự riêng của nó.

### Lưu ý trước khi mở

Trước khi mở, Bloxkeep có thể dừng lại hỏi trong hộp thoại **Lưu ý trước khi mở**:

| Cảnh báo | Khi nào | Nút | Không hỏi lại? |
|---|---|---|---|
| "*Tài khoản* sẽ thoát game đang chơi để vào game này." (hoặc "…sẽ thoát game và về màn hình chính Roblox.") | Tài khoản đã chọn đang ở game hay server khác. | Hủy, **Vẫn mở** | Không, luôn hỏi. |
| "Multi-Roblox đang tắt nên mỗi lần mở Roblox mới, cửa sổ cũ sẽ bị đóng." | Mở nhiều tài khoản, hoặc mở một tài khoản khi đang có cửa sổ Roblox khác, trong lúc Multi-Roblox tắt. | Hủy, **Vẫn mở**, **Bật Multi-Roblox rồi chơi** | Có |
| "Multi-Roblox chưa hoạt động: Roblox đã mở từ trước khi bật Multi-Roblox." | Multi-Roblox đã bật nhưng chưa sẵn sàng. | Hủy, **Sửa Multi-Roblox**, **Vẫn mở** | Không |
| Cảnh báo của Kiểm tra Multi-Roblox (Roblox đã đổi khóa chặn mở nhiều cửa sổ) | Mở nhiều cửa sổ trên một bản Roblox mà chưa có hai cửa sổ nào cùng chơi được. | Hủy, **Vẫn mở** | Có |
| "RAM chỉ còn trống *X*, mà mở thêm *N* cửa sổ Roblox cần khoảng *Y*." | Đang bật **Báo khi sắp hết RAM** và RAM sắp không đủ. | Hủy, **Vẫn mở** | Có |

- Ô **Không hỏi lại** chỉ hiện khi mọi cảnh báo trong hộp thoại đều bỏ qua được. Tích vào rồi bấm **Vẫn mở** thì từ đó những loại cảnh báo này không hỏi nữa, mỗi loại tính riêng.
- Cảnh báo đã bỏ qua cũng không chặn preset hẹn giờ.
- Muốn được hỏi lại: **Cài đặt › Thông báo › Câu hỏi xác nhận › Hỏi lại**. Dòng **Câu hỏi đã bỏ qua** cho biết đang có bao nhiêu câu.

Các câu hỏi khác cũng có **Không hỏi lại**: xóa preset, **Đóng tất cả Roblox**, **Buộc tắt tất cả Roblox**, **Bật Multi-Roblox?**, **Bỏ các thay đổi?** và **Khóa Bloxkeep?**. Nút **Hỏi lại** cũng đưa các câu này trở lại. Riêng xóa tài khoản, khôi phục bản sao lưu và gỡ link Discord thì lúc nào cũng hỏi.

---

## Multi-Roblox

Bình thường Roblox chỉ cho mở một cửa sổ: mở cái thứ hai là cái đầu bị đóng. **Multi-Roblox** cho nhiều tài khoản chơi cùng lúc. Tính năng này **tắt sẵn**. Roblox không chính thức hỗ trợ chạy nhiều client, nên khi bật, app sẽ hỏi **Bật Multi-Roblox?**: bạn tự chịu rủi ro khi dùng.

Cách hoạt động: Roblox dùng một cái khóa để chỉ cho chạy một cửa sổ. Bloxkeep giữ trước tên của khóa đó khi chưa có Roblox nào chạy, nhờ vậy các cửa sổ không đóng nhau. Bloxkeep không chèn gì vào Roblox. Từ đó có hai điều cần nhớ:

- Phải mở Bloxkeep **trước** Roblox.
- Đừng tắt Bloxkeep trong lúc chơi (chạy ẩn dưới khay hệ thống thì được).

### Thẻ Multi-Roblox ở thanh bên

| Đèn | Chữ | Nghĩa là | Bấm vào chữ |
|---|---|---|---|
| Xám | **Đang tắt** | Multi-Roblox đang tắt. | Mở phần chi tiết. |
| Xanh | **Đang bật, mở được nhiều cửa sổ** | Đang chạy tốt. | Mở phần chi tiết. |
| Vàng | **Chưa hoạt động, bấm để sửa** | Roblox đã mở từ trước khi bật Multi-Roblox. | Mở phần chi tiết kèm cách sửa. |
| Vàng | **Roblox đã đóng cửa sổ, bấm để xem** | Roblox đã đóng một cửa sổ khi cửa sổ khác mở ra. | Mở **Cài đặt › Phiên bản & đồ họa Roblox**. |
| Vàng | **Bản Roblox mới có thể chặn, bấm để xem** | [Kiểm tra Multi-Roblox](#kiểm-tra-multi-roblox) thấy bản Roblox có thay đổi. | Mở **Cài đặt › Phiên bản & đồ họa Roblox**. |

Công tắc trên thẻ để bật hoặc tắt. Công tắc này cũng có ở **Cài đặt › Khởi chạy › Multi-Roblox**, kèm nhãn trạng thái (**Tắt**, **Sẵn sàng**, **Chưa sẵn sàng**) và link **Chi tiết**.

### Sửa lỗi "Chưa hoạt động"

Phần chi tiết liệt kê các bước:

1. **Đóng hết Roblox**, kể cả Roblox ở khay hệ thống: bấm **Đóng tất cả Roblox**.
2. Chỉ khi Roblox tự chạy cùng Windows: **Không cho Roblox tự chạy cùng Windows** bằng nút **Tắt tự khởi động** (nút đổi thành "Đã tắt ✓").
3. Sau đó Multi-Roblox sẽ tự bật (đèn chuyển xanh).

Lần sau nhớ mở Bloxkeep trước Roblox.

### Chế độ cửa sổ và xếp cửa sổ

Khi Multi-Roblox đang bật, phần chi tiết có hai công tắc:

- **Luôn mở Roblox ở chế độ cửa sổ** (bật sẵn): mọi cửa sổ Roblox dùng chung một cài đặt toàn màn hình, nên khi mở nhiều cửa sổ sẽ có cái toàn màn hình, có cái không. Bloxkeep mở tất cả ở chế độ cửa sổ, mở xong thì trả lại cài đặt của bạn.
- **Xếp các cửa sổ cạnh nhau sau khi mở** (tắt sẵn): mở xong các tài khoản, Bloxkeep xếp các cửa sổ cạnh nhau trên màn hình chính. Đây cũng chính là lựa chọn **Cài đặt › Khởi chạy › Khi cửa sổ Roblox mở ra › Xếp cạnh nhau**.

Nút **Tắt Multi-Roblox** ở dưới để tắt tính năng.

### Kiểm tra Multi-Roblox

**Cài đặt › Phiên bản & đồ họa Roblox › Cập nhật Roblox › Kiểm tra Multi-Roblox**. Mỗi khi Roblox có bản mới, Bloxkeep xem bản đó có thay đổi gì làm Multi-Roblox ngừng chạy không, thường là trước khi bản đó về máy bạn. **Cách hoạt động** mở [trang giải thích](roblox-watch.md) (tiếng Anh).

| Kết quả | Nghĩa là |
|---|---|
| **Không thay đổi** | Khóa vẫn như bản trước. |
| **Đã chạy tốt trên máy này** | Đã có hai cửa sổ cùng chơi được trên bản này ở máy bạn. Đây là bằng chứng chắc chắn. |
| **Có thay đổi, Bloxkeep đã tự xử lý** | Roblox đổi tên khóa, Bloxkeep đã giữ thêm tên mới. Chưa chắc chắn hẳn. |
| **Có thay đổi, có thể bị chặn** | Đoạn mã chặn mở nhiều cửa sổ đã thay đổi. |
| **Nhiều khả năng bị chặn** | Khóa mà Multi-Roblox giữ không còn nữa. |
| **Chưa kiểm tra** / **Chưa kiểm tra được** | Chưa có kết quả. |

Dòng **Bản Roblox sắp ra** cho kết quả của bản Roblox chưa phát hành. **Lịch sử cập nhật** liệt kê mọi bản Roblox đã thấy trên máy.

Khi có cảnh báo thì làm gì:

- **Cứ chơi bình thường.** Nếu Roblox bắt đầu đóng cửa sổ cũ mỗi khi mở cửa sổ mới, Bloxkeep sẽ báo, và lúc đó cập nhật Bloxkeep là sửa được. Bấm **Kiểm tra bản cập nhật Bloxkeep**.
- Nếu Roblox đã đóng cửa sổ ("Roblox đã đóng *N* cửa sổ khi mở thêm cửa sổ khác"): cập nhật Bloxkeep, có thể bản mới đã sửa. Trong lúc chờ, chơi từng tài khoản một. **Ẩn** để tắt dòng báo này. Dòng này cũng tự mất khi có từ hai tài khoản cùng ở trong game, hoặc khi Roblox cập nhật.
- Cho tới khi hai cửa sổ cùng chơi được trên bản đó, mở nhiều tài khoản sẽ được hỏi trước (xem [Lưu ý trước khi mở](#lưu-ý-trước-khi-mở)).

---

## Preset và hẹn giờ

Một preset lưu sẵn các tài khoản (theo thứ tự), một game (hoặc không, để mở màn hình chính Roblox), cách chọn server, thời gian chờ giữa các tài khoản, tùy chọn cho cửa sổ, nơi gửi tin Discord và lịch hẹn giờ. Lưu được tối đa 50 preset.

### Tạo preset

- Tab **Preset** › **Thêm preset**;
- hoặc nút **Preset** trên thanh chọn game › **Lưu thành preset…**: preset được điền sẵn từ thanh chọn game (các tài khoản đang chọn, game, Job ID thành **Server cố định**, tên game thành tên preset);
- hoặc Ctrl+K › "Lưu thành preset…".

| Ô | Ý nghĩa |
|---|---|
| **Tên** | Bắt buộc. |
| **Game** | Giống ô game ở thanh dưới. Để trống thì chỉ mở Roblox. |
| **Server** | **Bất kỳ (Roblox tự chọn)**, **Đông nhất**, **Vắng nhất**, **Ping thấp nhất**, **Ngẫu nhiên**, hoặc **Server cố định (Job ID)**. Server được chọn lúc preset chạy, trong số server còn đủ chỗ cho các tài khoản của preset; không có server nào đủ chỗ thì để Roblox tự chọn. |
| **Job ID** | Chỉ dùng cho **Server cố định**. |
| **Chờ giữa các tài khoản (giây)** | Từ 0 đến 120. Để trống thì dùng giá trị trong Cài đặt. |
| **Tài khoản** | Tích chọn. Các tài khoản đã tích mở theo thứ tự này: kéo tay nắm hoặc bấm mũi tên để đổi. Nếu có tài khoản đã bị xóa, form sẽ báo; khi lưu chúng bị bỏ ra. |

Nếu không lưu được, lý do hiện ngay trên các nút và ô cần sửa được đưa con trỏ tới.

### Áp dụng cho cửa sổ Roblox

Các tùy chọn này áp dụng cho các cửa sổ mà preset mở, cho tới khi chúng đóng (kể cả khi chúng tự vào lại game). Bấm **Vào game** bình thường thì vẫn theo Cài đặt chung. Tài khoản nằm trong hai preset thì theo preset đã mở cửa sổ của nó.

| Tùy chọn | Lựa chọn |
|---|---|
| **Chống AFK** | **Theo Cài đặt (đang bật/đang tắt)**, **Bật**, **Tắt** |
| **Tự vào lại game** | **Theo từng tài khoản**, **Bật**, **Tắt** |
| **Hiệu năng** | **Bình thường**, hoặc **Treo máy nhẹ**: đồ họa thấp nhất, 30 FPS, tắt tiếng, thu nhỏ cửa sổ khi đã vào game. |
| **Mở lại Roblox** | **Không**, hoặc **Mỗi 2 / 4 / 6 / 12 giờ**: lần lượt đóng từng cửa sổ rồi mở lại vào đúng game cũ. Cần có game. |
| **Xếp cửa sổ** | **Theo Cài đặt (đang bật/đang tắt)**, **Bật**, **Tắt**: xếp các cửa sổ sau khi preset mở xong. |

### Discord cho từng preset

**Gửi tin của preset này vào**: **Kênh trong Cài đặt**, hoặc **Webhook riêng** (dán link; bấm **Gửi thử** để kiểm tra). Gồm tin hẹn giờ, bản tổng kết sau khi preset hẹn giờ dừng, và cảnh báo của các cửa sổ do preset mở. Các tin khác vẫn vào kênh trong Cài đặt. Nếu **Gửi lên Discord** đang tắt ở **Cài đặt › Discord** thì chưa có tin nào được gửi. Nếu Discord xóa webhook của preset, tin của preset lại vào kênh trong Cài đặt.

### Hẹn giờ

Bật **Hẹn giờ** ("Tự chạy preset này vào giờ đã đặt"):

- **Giờ bắt đầu**;
- **Lặp lại**: **Các ngày trong tuần** (bấm chọn các ngày) hoặc **Một lần, vào ngày**;
- **Dừng**: **Không dừng (chơi tiếp)**, **Sau vài giờ** (từ ½ đến 168, mỗi bước nửa giờ), hoặc **Vào giờ**.

Cần biết:

- Bloxkeep phải đang chạy (chạy ẩn dưới khay cũng được) và đã mở khóa.
- Đến giờ mà Bloxkeep đang tắt, đang khóa hoặc máy đang ngủ thì lần đó bị bỏ, không chạy bù. Nếu là do đang khóa hoặc máy ngủ, Bloxkeep báo một lần ("…không chạy lúc 20:00: Bloxkeep đang khóa." hoặc "…máy đang ngủ hoặc bận.").
- Nếu cần bạn xác nhận mới mở được (Multi-Roblox tắt hoặc chưa sẵn sàng, thiếu RAM, có tài khoản đang ở game khác), lần chạy đó bị bỏ qua và Bloxkeep báo cho bạn. Cảnh báo bạn đã chọn **Không hỏi lại** thì không chặn.
- Đến giờ dừng, chỉ những tài khoản do lần chạy này mở mới bị đóng. Giờ dừng vẫn có tác dụng khi Bloxkeep đang khóa; giờ dừng rơi vào lúc máy ngủ thì được thực hiện ngay khi máy thức dậy.
- Hẹn giờ một lần tự tắt sau khi đã chạy (hoặc khi giờ đó đã qua).

### Tab Preset

Mỗi dòng hiện biểu tượng game, tên preset, game ("Màn hình chính Roblox" nếu không có game), và một dòng tóm tắt: server, thời gian chờ, lịch hẹn, và những tùy chọn khác với Cài đặt. Bên dưới là "Lần tới: Hôm nay 20:00", "Chạy lần cuối 2 giờ trước", hoặc "Đang mở 3 cửa sổ". Bên phải là ảnh đại diện các tài khoản, công tắc **Hẹn giờ** (chỉ có khi lịch hẹn đã có giờ bắt đầu), rồi:

- **Chạy**: bị mờ khi preset không còn tài khoản nào, hoặc khi đang có lượt mở khác. Khi cửa sổ của preset đang mở, nút đổi thành **Đóng**, chỉ đóng đúng các cửa sổ do preset này mở.
- **Sửa**, **Nhân bản** (tạo bản sao; nếu preset gốc đang bật hẹn giờ thì hẹn giờ của bản sao bị tắt), **Xóa** (có hỏi lại, có thể chọn **Không hỏi lại**).
- Tay nắm và mũi tên để đổi thứ tự (ẩn đi khi đang tìm kiếm). Menu **Preset** ở thanh dưới dùng đúng thứ tự này.

Phần đầu trang đếm số preset và số preset hẹn giờ, và ghi "đã đạt mức tối đa" khi đủ 50. Ô **Tìm preset hoặc game** (Ctrl+F) để lọc danh sách. Preset mà mọi tài khoản đều đã bị xóa sẽ ghi **Không còn tài khoản nào**.

Nút **Preset** ở thanh chọn game liệt kê mọi preset (bấm vào để chạy; bị mờ khi preset không còn tài khoản), rồi **Lưu thành preset…** và **Sửa preset…** (mở tab Preset). Trong Ctrl+K, gõ tên preset để chạy nó.

---

## Giữ tài khoản trong game

### Tự vào lại game

Bật ở:

- từng tài khoản: **⋯ › Tự vào lại game**;
- mọi tài khoản: **Cài đặt › Treo game › Tự vào lại game › Bật cho mọi tài khoản** (khi chỗ bật chỗ tắt sẽ ghi "Bật cho 3/10 tài khoản");
- cửa sổ của một preset: tùy chọn **Tự vào lại game** của preset đó.

Khi Roblox của một tài khoản bị văng, bị kick hoặc mất kết nối, Bloxkeep mở lại đúng game đó (hoặc đúng server riêng) sau 10 giây, nhưng không chắc vào lại đúng server cũ. Nếu cứ bị văng liên tục, từ lần thứ ba Bloxkeep chờ 2, 5, 10 rồi 15 phút giữa các lần thử; thẻ tài khoản hiện thời gian chờ và nút **Hủy**. Ở yên trong game 10 phút thì đếm lại từ đầu. Mất mạng thì Bloxkeep thử lại mỗi 30 giây, tối đa 20 lần. Máy vừa thức dậy thì Bloxkeep chờ có mạng rồi mới vào lại.

Bloxkeep không vào lại khi:

- bạn tự thoát game, hoặc tự tay đóng cửa sổ Roblox;
- tài khoản đăng nhập ở thiết bị khác;
- tài khoản bị ban;
- chính Roblox đóng cửa sổ vì có cửa sổ khác mở ra;
- Bloxkeep đang khóa (mở khóa xong thì tính năng chạy lại).

Nếu Roblox kết thúc phiên đăng nhập của tài khoản, tự vào lại game tự tắt cho tài khoản đó, và tự bật lại khi bạn đăng nhập lại. Khi Multi-Roblox chưa sẵn sàng, Bloxkeep không vào lại (vì cửa sổ mới sẽ đóng mất cửa sổ đang mở) và báo cho bạn.

### Chống AFK

**Cài đặt › Treo game › Chống AFK** (preset có thể ghi đè). Roblox kick tài khoản sau 20 phút không có thao tác. Bloxkeep đưa từng cửa sổ Roblox đang ở trong game lên trước trong chốc lát, nhích chuột vài pixel rồi trả về, và đưa bạn về lại cửa sổ đang dùng. Không bấm phím, không click gì.

- Bắt đầu từ phút thứ 12 kể từ thao tác cuối của cửa sổ, vào phút đầu tiên không ai dùng máy.
- Cửa sổ còn 2 phút nữa là bị kick thì được xử lý ngay cả khi bạn đang dùng máy, vào lúc bạn ngừng tay 2 giây, nhưng không bao giờ chen ngang ứng dụng toàn màn hình.
- Không chạy được khi máy đang khóa.
- Nếu tài khoản vẫn bị kick, Bloxkeep nói rõ lý do (bạn dùng máy liên tục, máy đang khóa, có ứng dụng toàn màn hình phía trước, hoặc Windows không cho đưa cửa sổ lên trước).

### Kiểm tra đăng nhập mỗi ngày

**Cài đặt › Treo game › Kiểm tra đăng nhập mỗi ngày** (bật sẵn). Roblox có thể kết thúc phiên đăng nhập mà không báo ai. Mỗi ngày một lần, khoảng 45 giây sau khi bạn mở khóa, Bloxkeep kiểm tra đăng nhập của từng tài khoản, lần lượt, cách nhau 2 giây, đồng thời xem Roblox có báo bị ban không. Một thông báo duy nhất nêu tên các tài khoản cần đăng nhập lại. Dòng cài đặt có ghi "Lần kiểm tra gần nhất: …".

Muốn kiểm tra ngay: **⋯ › Kiểm tra đăng nhập** trên một tài khoản, hoặc **⋯ Tùy chọn khác › Kiểm tra đăng nhập mọi tài khoản**.

### Báo khi sắp hết RAM

**Cài đặt › Treo game › Báo khi sắp hết RAM** (bật sẵn):

- trước khi mở, hỏi lại nếu RAM trống không đủ cho các cửa sổ mới (xem [Lưu ý trước khi mở](#lưu-ý-trước-khi-mở));
- trong lúc chơi, báo khi máy dùng từ 92 % RAM trở lên (tối đa 5 phút một lần): "Máy đang dùng 93% RAM. Hãy đóng bớt cửa sổ Roblox nếu thấy lag."

Thanh RAM trong thẻ **Roblox trên máy này** ở thanh bên chuyển đỏ từ 90 %.

### Tiết kiệm RAM cho cửa sổ chạy nền

**Cài đặt › Treo game › Tiết kiệm RAM cho cửa sổ chạy nền** (tắt sẵn). Các cửa sổ Roblox bạn không chơi được đặt mức ưu tiên bộ nhớ thấp, nên Windows lấy RAM của chúng trước; khi máy thật sự thiếu RAM, chúng được yêu cầu nhả bớt. Cửa sổ ở phía trước được trả lại mức ưu tiên bình thường. Roblox có thể không chịu nhả; khi đó Bloxkeep bỏ qua bước này.

### Chạy ẩn dưới khay hệ thống

**Cài đặt › Treo game › Chạy ẩn dưới khay hệ thống khi đóng** (bật sẵn). Bấm ✕ thì Bloxkeep thu xuống khay hệ thống; tự vào lại game, Multi-Roblox và preset hẹn giờ vẫn chạy. Bấm chuột phải vào biểu tượng ở khay để chọn **Cửa sổ nhỏ**, **Mở Bloxkeep** hoặc **Thoát Bloxkeep**. Tắt tùy chọn này thì bấm ✕ là thoát hẳn.

---

## Theo dõi game, cửa sổ và thống kê

### Tab Theo dõi game

**Theo dõi game** ở thanh bên hiện hình trực tiếp của mọi cửa sổ Roblox, kèm tài khoản, game, RAM và PID.

- Bấm vào hình để đưa cửa sổ đó lên trên cùng.
- Chuột phải để chọn **Hiện**, **Đóng Roblox** hoặc **Sao chép PID**.
- Cửa sổ đang thu nhỏ thì ghi "Đang thu nhỏ, không có hình trực tiếp".
- Khi có hộp thoại hay menu mở, các hình tạm ẩn đi.

### Cửa sổ nhỏ

Nút **Cửa sổ nhỏ** ở thanh bên (cũng có trong menu ở khay và trong Ctrl+K) mở một cửa sổ nhỏ luôn nằm trên cùng: các tài khoản đang chơi, kèm game, RAM và PID, mỗi tài khoản có nút **Hiện** và **Đóng**, cùng nút **Đóng tất cả** (có hỏi trước) và **Mở Bloxkeep**. Cửa sổ nhỏ vẫn chạy khi cửa sổ chính đang ẩn dưới khay. Nhấn Esc để ẩn; trong lúc nó đang mở, nút ở thanh bên ghi **Ẩn cửa sổ nhỏ**. Vị trí và chiều cao được nhớ lại.

### Roblox trên máy này

Thẻ này ở thanh bên cho biết:

- số cửa sổ Roblox đang mở (rê chuột lên con số để xem từng tài khoản và PID) và lượng RAM chúng dùng;
- thanh RAM của cả máy: phần của Roblox, rồi phần còn lại đang dùng; "RAM máy đã dùng 64% · còn trống 5,2 GB";
- phiên bản Roblox kèm đèn: xanh là "mới nhất", vàng là đã có bản mới ("có bản 0.744", hoặc "có bản mới" khi chỉ là bản vá cùng số phiên bản);
- nút **Cửa sổ nhỏ**, và **Đóng tất cả** khi đang có Roblox chạy (có hỏi trước).

Nếu máy chưa cài Roblox, một thẻ cảnh báo ghi "Hình như máy này chưa cài Roblox."

### Xếp cửa sổ

- Nút **Xếp cửa sổ** ở thanh chọn game, **⋯ Tùy chọn khác › Xếp cửa sổ Roblox**, hoặc Ctrl+K. Máy có nhiều màn hình thì chọn màn hình trước ("Màn hình 2 (1920×1080)", chữ "chính" đánh dấu màn hình chính).
- Kiểu xếp lấy từ **Cài đặt › Khởi chạy › Kiểu xếp cửa sổ**: **Lấp đầy màn hình**, hoặc **Ô nhỏ**, **Ô vừa**, **Ô lớn** xếp từ góc trên bên trái, phần còn lại của màn hình để trống. Cửa sổ càng nhỏ thì card đồ họa càng đỡ tải.
- Muốn tự xếp sau khi mở: **Cài đặt › Khởi chạy › Khi cửa sổ Roblox mở ra › Xếp cạnh nhau** (cần Multi-Roblox), hoặc tùy chọn **Xếp cửa sổ** của preset. Cửa sổ của preset dùng **Treo máy nhẹ** không được xếp (chúng được thu nhỏ).

### Thống kê

**Thống kê** ở thanh bên có hai tab: **Thống kê** và **Hoạt động**. Chỉ tính các game mở bằng Bloxkeep.

**Thống kê**: chọn **Hôm nay**, **7 ngày** hoặc **30 ngày**.

- Các ô số liệu: **Thời gian chơi**, **Lượt chơi**, **Mất kết nối**, **Chơi nhiều nhất**; khi cửa sổ đủ rộng có thêm **Tài khoản đã chơi** và **Lượt dài nhất**.
- **Thời gian chơi theo giờ** (Hôm nay) hoặc **Thời gian chơi theo ngày**.
- **Theo tài khoản**: thời gian chơi, số lượt và số lần mất kết nối của từng tài khoản.
- **Game chơi nhiều** và **Theo preset**.
- Số liệu tự làm mới mỗi 30 giây khi tab đang mở.

**Hoạt động**: những gì Bloxkeep ghi nhận được, mới nhất ở trên: mở tài khoản, vào và rời game, bị văng kèm lý do, tự vào lại, hẹn giờ, đăng nhập, cập nhật. Chỉ giữ 1.000 sự kiện gần nhất.

- Lọc theo **Tài khoản** và **Loại sự kiện** (**Vào/ra game**, **Văng và lỗi**, **Hẹn giờ**, **Đăng nhập**, **Cập nhật**).
- **Xem thêm** để tải các sự kiện cũ hơn.
- **Xuất CSV** lưu những gì đang lọc ra file (cũng có trong Ctrl+K › "Xuất nhật ký hoạt động (CSV)").

---

## Gửi lên Discord

Bloxkeep có thể gửi tổng kết thời gian chơi, một tin trạng thái tự cập nhật và các cảnh báo vào kênh Discord của bạn, thông qua webhook (một đường link cho phép chương trình gửi tin vào đúng một kênh). Mặc định tắt, chỉ gửi khi bạn bật.

### Cách thiết lập

1. Trong Discord: **Cài đặt kênh › Tích hợp › Webhook › Webhook mới › Sao chép URL webhook**.
2. Trong Bloxkeep: **Cài đặt › Discord › Link webhook › Dán link…**, dán vào rồi bấm **Lưu**. (Bật **Gửi lên Discord** khi chưa có link thì app cũng hỏi link.) Link đúng có dạng `https://discord.com/api/webhooks/…`.
3. Bật **Gửi lên Discord**, rồi bấm **Gửi thử**: "Đã gửi tin thử. Hãy xem trong kênh."

Cần biết:

- Link được mã hóa và lưu cùng các tài khoản. Ai có link này cũng gửi được tin vào kênh đó, nên đừng đưa cho người khác.
- **Hiện** cho xem link trong 30 giây. **Thay…** để dán link khác. **Gỡ** có hỏi trước; các tin đã có trong kênh vẫn giữ nguyên.
- Dòng trạng thái ghi "Gửi lần cuối: …", "Chưa gửi gì", số tin đang chờ gửi, hoặc lỗi gần nhất.
- Nếu Discord báo webhook không còn nữa (đã bị xóa hoặc link bị đặt lại), Bloxkeep ngừng gửi, hiện dòng báo, và trang **Discord** có chấm đỏ. Hãy dán link mới.
- Khi Bloxkeep đang khóa thì không gửi gì. Tin không gửi được lúc mất mạng sẽ chờ; tin quá cũ bị bỏ và tin tiếp theo có ghi rõ.

### Tổng kết thời gian chơi

| Dòng | Lựa chọn |
|---|---|
| **Gửi tự động** | **Không**, **Hằng ngày**, **Hằng tuần**, **Cả hai**. **Hằng ngày lúc** một giờ; **Hằng tuần vào** một ngày và giờ. |
| **Khi preset hẹn giờ chạy xong** | Gửi khoảng nửa phút sau khi các tài khoản đóng, chỉ tính tài khoản của preset đó (vào kênh riêng nếu preset có). |
| **Khoảng thời gian** | **24 giờ qua**, **7 ngày qua**, **30 ngày qua**. Áp dụng cho bản tổng kết hằng ngày và **Gửi ngay**. Bản hằng tuần luôn tính 7 ngày; bản gửi sau preset hẹn giờ tính từ đầu hôm nay. |
| **Gồm** | **Tổng thời gian chơi**, **Thời gian từng tài khoản**, **Game chơi nhiều**, **Số lượt chơi**, **Số lần mất kết nối** (ít nhất một mục). |
| **Tài khoản** | **Mọi tài khoản**, hoặc **Chọn…** một số tài khoản. |
| **Tag** | Xem [Tag](#tag). |
| **Gửi ngay** | **Gửi tổng kết** gửi luôn, theo các lựa chọn ở trên. |

### Cảnh báo

- **Gửi cảnh báo** bật hoặc tắt mọi loại cùng lúc.
- **Tag** đặt cùng một kiểu tag cho mọi loại (ghi "Tùy từng loại" khi các loại khác nhau).
- **Chọn loại cảnh báo** mở ra một công tắc và một ô tag cho từng loại: **Tài khoản bị văng hoặc tự vào lại**, **Preset hẹn giờ**, **Tài khoản cần đăng nhập lại** (gồm cả bị ban), **Sắp hết RAM**, **Lỗi khi mở game**.
- Phần này tách riêng với thông báo Windows. Các cảnh báo cách nhau dưới 10 giây được gộp thành một tin.

### Tin trạng thái

**Luôn cập nhật tin trạng thái**, chọn **Mỗi 2 / 5 / 10 / 15 / 30 / 60 phút**: chỉ một tin, được sửa liên tục: tài khoản nào đang mở, game gì, đã chơi bao lâu, PID và RAM. Không bao giờ tag ai. Khi Bloxkeep khóa hoặc tắt, tin này ghi rõ.

### Tag

**Không ai**, **@here**, **@everyone**, **Một người…** hoặc **Một vai trò…**. Tag một người hay một vai trò thì cần ID (17 đến 20 chữ số): trong Discord bật **Cài đặt › Nâng cao › Chế độ nhà phát triển**, rồi bấm chuột phải vào người hoặc vai trò › **Sao chép ID**.

### Giờ yên lặng

**Giờ yên lặng** với **Từ** và **Đến**: tin vẫn được gửi nhưng không tag ai. Nếu giờ bắt đầu và giờ kết thúc trùng nhau thì giờ yên lặng không có tác dụng.

### Tùy chỉnh tin nhắn

Mở **Tùy chỉnh tin nhắn**:

- **Tên người gửi** (tối đa 80 ký tự, không chứa "discord" hay "clyde");
- **Ảnh người gửi** (link `https://`; để trống thì dùng logo Bloxkeep);
- **Ngôn ngữ**: **Giống Bloxkeep**, English, Tiếng Việt;
- **Kiểu tin**: **Thẻ** (dễ đọc hơn) hoặc **Văn bản thường** (hợp với thông báo trên điện thoại);
- **Ẩn tên tài khoản trong tin nhắn**: ghi "Tài khoản 1", "Tài khoản 2"… thay cho tên.

Tin nhắn có tên tài khoản (hoặc "Tài khoản N"), tên game, thời gian chơi, số lượt, PID, RAM và link tới ảnh của chính Roblox. Không bao giờ có cookie, mật khẩu hay link webhook.

---

## Cài đặt, từng trang một

**Cài đặt** ở thanh bên liệt kê các trang ở bên trái. Ô **Tìm cài đặt** ở trên cùng (Ctrl+F khi đang ở Cài đặt) tìm các dòng trên mọi trang; nhấn Esc để xóa. Chấm đỏ cạnh tên trang nghĩa là có chỗ **Cần xem lại**: Roblox đã đóng cửa sổ hoặc có cảnh báo Multi-Roblox (trang **Phiên bản & đồ họa Roblox**), hoặc Discord đã ngừng gửi (trang **Discord**).

### Khởi chạy

**Multi-Roblox & cửa sổ**

| Dòng | Lựa chọn | Tác dụng |
|---|---|---|
| **Multi-Roblox** | Công tắc, **Chi tiết** | Chơi nhiều tài khoản cùng lúc. Xem [Multi-Roblox](#multi-roblox). |
| **Hiện tên tài khoản trên cửa sổ** | Công tắc (bật sẵn) | Ghi tên tài khoản lên tiêu đề cửa sổ Roblox, dễ phân biệt dưới taskbar. |
| **Khi cửa sổ Roblox mở ra** | **Để Roblox tự chọn**, **Phóng to**, **Xếp cạnh nhau** | Cửa sổ phóng to vẫn là cửa sổ thường, nên không dùng chung cài đặt toàn màn hình của Roblox. Xếp cạnh nhau cần bật Multi-Roblox. |
| **Kiểu xếp cửa sổ** | **Lấp đầy màn hình**, **Ô nhỏ**, **Ô vừa**, **Ô lớn** | Cách xếp khi bấm **Xếp cửa sổ** (và khi tự xếp sau khi mở). |

**Launcher & thời gian chờ**

| Dòng | Lựa chọn | Tác dụng |
|---|---|---|
| **Mở Roblox bằng** | Xem bảng dưới; nút **Chọn…** | Chương trình dùng để mở Roblox. Dùng Bloxstrap hoặc Fishstrap thì vẫn giữ được mod. |
| **Chờ giữa các tài khoản** | 0–60 giây (mặc định 8) | Mở quá nhiều tài khoản cùng lúc dễ bị lỗi. |

**Mở Roblox bằng**:

| Lựa chọn | Ghi chú |
|---|---|
| **Mặc định** (*tên*) | Chương trình đang mở link Roblox trên máy. |
| **Bloxstrap**, **Fishstrap** | Giữ nguyên mod của bạn. |
| Launcher kiểu Bloxstrap khác | Tự hiện nếu tìm thấy trên máy. |
| **Roblox (chính thức)** | Bản cài từ roblox.com. |
| **Chạy thẳng RobloxPlayerBeta (bỏ qua launcher)** | Không qua launcher nào. Nên thử khi launcher chặn mở nhiều cửa sổ. |
| **Roblox (Microsoft Store), đang thử nghiệm** | Chỉ hiện khi máy có Roblox bản Microsoft Store. Bloxkeep mở Roblox của Store ngay bên trong gói cài của nó, với phiên đăng nhập riêng của từng tài khoản, bỏ qua màn hình khởi động của Microsoft (vốn chỉ cho một cửa sổ), nên mỗi tài khoản mở trong một cửa sổ riêng. Microsoft Store vẫn tự cài các bản cập nhật như thường. Nếu không chạy được, hãy báo ở **Cập nhật Bloxkeep › Gửi góp ý**. |
| **Launcher của bạn:** *tên* | Chương trình bạn chọn bằng nút **Chọn…** ("Chọn launcher khác…"). Nếu file đó bị chuyển đi hoặc xóa, lúc mở game app sẽ báo. |

Lựa chọn chưa cài sẽ ghi "*tên* (chưa cài)" và bị mờ. Nếu máy chỉ có Roblox bản Microsoft Store, ở đây có dòng ghi chú nói rõ.

### Phiên bản & đồ họa Roblox

**Cập nhật Roblox**

- Dòng báo khi Roblox đã đóng cửa sổ, kèm nút **Ẩn**.
- **Phiên bản Roblox**: "Đã là bản mới nhất", hoặc đã có bản mới và cách lấy về: Roblox tự cập nhật lần mở tới; nếu dùng **Chạy thẳng RobloxPlayerBeta** thì mở Roblox bình thường một lần; với Roblox bản Microsoft Store thì Store tự cài, hoặc cài ngay ở **Microsoft Store › Thư viện › Nhận bản cập nhật**. Ngoài ra có thể là "Chưa cài" hoặc "Chưa kiểm tra được".
- **Bản đang cài** (kèm launcher nó đến từ), **Bản mới nhất của Roblox** (chỉ hiện khi khác bản đang cài).
- **Kiểm tra Multi-Roblox** kèm link **Cách hoạt động**, và **Bản Roblox sắp ra**. Xem [Kiểm tra Multi-Roblox](#kiểm-tra-multi-roblox). Khi có cảnh báo, một dòng ghi chú nói nên làm gì, kèm nút **Kiểm tra bản cập nhật Bloxkeep**.
- **Lịch sử cập nhật**: mọi bản Roblox đã thấy trên máy, mới nhất ở trên, mỗi bản có link **Ghi chú** (ghi chú phát hành của Roblox, viết cho người làm game, hiếm khi nhắc tới thay đổi của chính app Roblox).

**Cấu hình Roblox**

| Dòng | Lựa chọn |
|---|---|
| **Áp dụng cho** | **Không áp dụng**, **Mọi tài khoản**, **Tài khoản đã chọn** (rồi bấm **Chọn tài khoản…**, hoặc dùng **⋯ › Dùng cấu hình Roblox** trên từng tài khoản) |
| **Đồ họa** | **Giữ nguyên**, 1–10 |
| **Giới hạn FPS** | **Giữ nguyên**, 30 / 60 / 144 / 240 FPS |
| **Âm lượng** | **Giữ nguyên**, **Tắt tiếng**, 25 / 50 / 75 / 100 % |

Ba dòng giá trị bị mờ khi **Áp dụng cho** là **Không áp dụng**. Bloxkeep ghi các giá trị này vào cài đặt của Roblox ngay trước khi mở tài khoản, rồi trả lại cài đặt của bạn khi Roblox đã chạy.

### Treo game

Trang này trước đây tên là "Hiệu năng".

**Giữ tài khoản luôn trong game**

| Dòng | Tác dụng |
|---|---|
| **Tự vào lại game** | **Bật cho mọi tài khoản** / **Tắt cho mọi tài khoản** (khi chỗ bật chỗ tắt ghi "Bật cho *n*/*m* tài khoản"; bị mờ khi chưa có tài khoản). Menu ⋯ của từng tài khoản bật/tắt riêng cho tài khoản đó; preset có đặt riêng thì cửa sổ do preset mở theo preset. Xem [Tự vào lại game](#tự-vào-lại-game). |
| **Chống AFK** | Treo máy không lo bị kick, kể cả khi bạn đi vắng hay đang dùng cửa sổ khác. Không hoạt động khi máy đang khóa. Xem [Chống AFK](#chống-afk). |
| **Kiểm tra đăng nhập mỗi ngày** | Bật sẵn. Ghi "Lần kiểm tra gần nhất: …". Xem [Kiểm tra đăng nhập mỗi ngày](#kiểm-tra-đăng-nhập-mỗi-ngày). |

**Bộ nhớ (RAM)**

| Dòng | Tác dụng |
|---|---|
| **Báo khi sắp hết RAM** | Bật sẵn. Xem [Báo khi sắp hết RAM](#báo-khi-sắp-hết-ram). |
| **Tiết kiệm RAM cho cửa sổ chạy nền** | Tắt sẵn. Xem [Tiết kiệm RAM](#tiết-kiệm-ram-cho-cửa-sổ-chạy-nền). |

**Ứng dụng**

| Dòng | Tác dụng |
|---|---|
| **Chạy ẩn dưới khay hệ thống khi đóng** | Bật sẵn. Xem [Chạy ẩn dưới khay hệ thống](#chạy-ẩn-dưới-khay-hệ-thống). |
| **Tăng tốc phần cứng** | Vẽ cửa sổ Bloxkeep bằng card đồ họa. Tự tắt nếu driver card đồ họa bị lỗi. Bật hay tắt thì cửa sổ được tải lại. |

### Thông báo

- **Thông báo Windows** (bật sẵn): chỉ hiện khi Bloxkeep chạy nền (đang ẩn, thu nhỏ, hoặc bị cửa sổ khác che). Mỗi tài khoản, mỗi loại tối đa một thông báo trong 5 phút. Thông báo bên trong Bloxkeep thì vẫn hiện như thường.
- **Loại sự kiện**: **Tài khoản bị văng hoặc tự vào lại**, **Preset hẹn giờ**, **Tài khoản cần đăng nhập lại** (gồm cả bị ban), **Sắp hết RAM**, **Lỗi khi mở game**. Bị mờ khi **Thông báo Windows** đang tắt.
- **Câu hỏi xác nhận › Câu hỏi đã bỏ qua**: số câu hỏi bạn đã tích **Không hỏi lại**. **Hỏi lại** đưa tất cả trở lại (bị mờ khi chưa bỏ qua câu nào). Xem [Lưu ý trước khi mở](#lưu-ý-trước-khi-mở).

### Discord

Từng dòng được giải thích ở [Gửi lên Discord](#gửi-lên-discord): **Gửi lên Discord**, **Link webhook**, **Gửi thử**, **Tổng kết thời gian chơi**, **Cảnh báo**, **Luôn cập nhật tin trạng thái**, **Giờ yên lặng**, **Tùy chỉnh tin nhắn**.

### Giao diện

| Dòng | Tác dụng |
|---|---|
| **Chủ đề** | **Theo Windows** (sáng hay tối theo Windows), hoặc một trong 74 chủ đề thuộc các nhóm **Cơ bản**, **Phong cảnh**, **Màu sắc**, **Phong cách** và **Theo mùa**. Mặc định là **Tối**. Dùng phím mũi tên để xem thử; chủ đề được lưu khi bạn dừng tay. |
| **Ngôn ngữ** | EN / VI. |
| **Ẩn tên tài khoản** | Xem [Ẩn tên tài khoản](#ẩn-tên-tài-khoản). |

### Bảo mật

**Mật khẩu chủ**: **Mật khẩu hiện tại**, **Mật khẩu mới (ít nhất 6 ký tự)**, **Nhập lại mật khẩu mới**, rồi bấm **Đổi mật khẩu**. Độ mạnh hiện ra khi bạn gõ. App không chấp nhận mật khẩu hiện tại sai, mật khẩu mới ngắn hơn 6 ký tự, chỉ toàn dấu cách, hoặc trùng mật khẩu cũ. Các bản sao lưu làm trước khi đổi vẫn chỉ mở được bằng mật khẩu cũ.

### Sao lưu & dữ liệu

**Sao lưu**

| Dòng | Tác dụng |
|---|---|
| **Sao lưu vào thư mục** | Chép kho dữ liệu đã mã hóa vào một thư mục, để không mất tài khoản khi máy hỏng. Nên chọn thư mục Google Drive hoặc OneDrive. |
| **Thư mục** | **Chọn…**, hoặc thư mục đồng bộ mà Bloxkeep tìm thấy (**Dùng Google Drive**, **Dùng OneDrive**). **Mở thư mục** để mở ra xem. Thư mục phải nằm ngoài thư mục dữ liệu của Bloxkeep. |
| Lần sao lưu gần nhất | "Sao lưu gần nhất: …" hoặc "Chưa có bản sao lưu", kèm nút **Sao lưu ngay** (bị mờ khi chưa chọn thư mục). |

- Bloxkeep sao lưu sau mỗi thay đổi (tối đa 10 phút một lần), khi khóa hoặc thoát nếu có gì thay đổi, và ngay sau khi bạn đổi mật khẩu chủ hoặc chọn thư mục mới.
- Giữ 10 bản mới nhất, cộng bản mới nhất của mỗi ngày trong 30 ngày. App OneDrive hay Google Drive sẽ tự tải chúng lên.
- Nếu mật khẩu chủ dễ đoán mà thư mục lại là thư mục đám mây, app sẽ nhắc bạn đổi mật khẩu dài hơn.

**Khôi phục & nhập**

| Dòng | Tác dụng |
|---|---|
| **Khôi phục** › **Khôi phục…** | Luôn hỏi trước. Dữ liệu hiện tại được giữ lại một bản, sau đó Bloxkeep khóa lại: mở khóa bằng mật khẩu chủ dùng lúc sao lưu. |
| **Nhập tài khoản từ file dữ liệu Bloxkeep** › **Nhập…** | Thêm tài khoản từ một file dữ liệu hay bản sao lưu khác; tài khoản đang có vẫn giữ nguyên. Xem [Nhập từ file Bloxkeep khác](#nhập-từ-file-bloxkeep-khác). |

**Dữ liệu**

| Dòng | Tác dụng |
|---|---|
| **Nơi lưu dữ liệu** › **Mở thư mục** | Nơi Bloxkeep cất dữ liệu của bạn (xem [Dữ liệu nằm ở đâu?](#dữ-liệu-nằm-ở-đâu)). |

### Cập nhật Bloxkeep

- Dòng **Bloxkeep** *phiên bản*, kèm trạng thái: "Bloxkeep tự động cập nhật", "Đang kiểm tra bản cập nhật…", "Đang cập nhật Bloxkeep… *N*%", "Sắp xong! …", "Bloxkeep đã là bản mới nhất · kiểm tra lúc …", "Không kiểm tra được bản cập nhật", "Tự động cập nhật đang tắt". Các nút: **Có gì mới**, và **Kiểm tra ngay** (bị mờ khi đang kiểm tra hoặc đang tải) hay **Khởi động lại để cập nhật**.
- **Tự động cập nhật**: Bloxkeep tự tải bản mới và tự cài vào lúc bạn không dùng. Tắt đi thì không có gì tự tải hay tự cài; **Kiểm tra ngay** vẫn dùng được. Bị mờ với bản không tự cập nhật được ("Chỉ có bản cài đặt là tự cập nhật được").
- **Hướng dẫn & góp ý**: **Hướng dẫn sử dụng › Mở** mở chính trang này. **Gửi góp ý › Viết góp ý** (xem [Gửi góp ý](#gửi-góp-ý-kèm-nhật-ký-lỗi)).

Khi bản cập nhật đã sẵn sàng, nút **Khởi động lại để cập nhật** hiện ở thanh bên. Roblox vẫn chạy bình thường trong lúc Bloxkeep khởi động lại. Cập nhật xong, mục **Có gì mới** cho biết những gì đã thay đổi.

---

## Phím tắt

**Ở bất kỳ đâu trong Bloxkeep**

| Phím | Tác dụng |
|---|---|
| Ctrl+K | Tìm nhanh: tài khoản, preset, game, trang cài đặt và thao tác. Nhấn lần nữa để đóng. Không dùng được ở màn hình khóa hay khi đang có hộp thoại khác. |
| Ctrl+F | Ô tìm kiếm của tab đang mở (Tài khoản, Preset khi đã có preset, Cài đặt). Ở các tab khác thì nhảy tới ô tìm tài khoản. |
| Ctrl+L hoặc `/` | Nhảy tới ô game. |
| Esc | Rời ô đang gõ; nếu không thì bỏ chọn tài khoản. Đóng menu, danh sách hoặc hộp thoại. |

**Tab Tài khoản** (khi không đang gõ trong ô nào)

| Phím | Tác dụng |
|---|---|
| Bấm / Shift + bấm | Chọn hoặc bỏ chọn một thẻ / chọn liền một dãy. |
| Bấm đúp | Vào game riêng tài khoản đó. |
| Chuột phải | Menu của tài khoản. |
| Phím mũi tên, Home, End | Đi giữa các thẻ. |
| Space | Chọn thẻ đang trỏ tới. |
| Ctrl+A | Chọn mọi tài khoản đang hiện. |
| Enter | Vào game với các tài khoản đã chọn. |
| F2 | Sửa tài khoản đang chọn (đúng một tài khoản). |
| Delete | Xóa các tài khoản đã chọn (có hỏi trước). |

**Ô game và ô Job ID**

| Phím | Tác dụng |
|---|---|
| ↓ / ↑ | Đi trong danh sách game. |
| Enter | Chọn game đang sáng; nếu ô đang có link hoặc Place ID thì vào game. Trong ô Job ID: vào game. |
| Esc | Đóng danh sách game. |

**Tìm nhanh (Ctrl+K)**

| Phím | Tác dụng |
|---|---|
| ↓ / ↑, Page Down / Page Up | Đi trong danh sách kết quả. |
| Enter | Mở mục đang sáng (tài khoản thì được chọn và hiện ra; preset thì chạy; game thì đưa vào ô game). |
| Ctrl+Enter / Shift+Enter | Với tài khoản: **Vào game** / **Sửa**. |
| Esc | Đóng. |

**Menu và hộp thoại**

| Phím | Tác dụng |
|---|---|
| ↓ / ↑ | Đi trong menu. Tab hoặc Esc để đóng menu. |
| Enter | Trong form: bấm nút chính. |
| Esc | Đóng hộp thoại (hỏi trước nếu bạn đã gõ gì đó). |
| Tab | Di chuyển giữa các ô trong hộp thoại. |

**Những chỗ khác**

| Phím | Ở đâu | Tác dụng |
|---|---|---|
| ↓ / ↑, Home, End | Danh sách trang Cài đặt | Chuyển trang (cửa sổ hẹp thì dùng thêm ← / →). |
| Phím mũi tên, Home, End | Bảng chọn chủ đề | Xem thử các chủ đề. |
| Esc | Ô Tìm cài đặt | Xóa nội dung tìm. |
| Esc | Cửa sổ nhỏ | Ẩn cửa sổ nhỏ. |
| Ctrl+T / Ctrl+W / Ctrl+Tab | Cửa sổ trình duyệt của Bloxkeep | Mở tab trống / đóng tab / chuyển tab. |

---

## Gỡ rối và câu hỏi thường gặp

### Roblox đóng mất các cửa sổ khác

- Multi-Roblox đã bật chưa? Thẻ ở thanh bên ghi **Đang tắt** thì bật lên.
- Đèn vàng **Chưa hoạt động, bấm để sửa**: Roblox đã mở trước Bloxkeep. Bấm vào và làm theo các bước (đóng hết Roblox, kể cả ở khay hệ thống; tắt tự khởi động nếu có). Lần sau nhớ mở Bloxkeep trước Roblox.
- Đèn vàng **Roblox đã đóng cửa sổ** hoặc **Bản Roblox mới có thể chặn**: một bản Roblox mới có thể đã đổi cách chặn nhiều cửa sổ. Mở **Cài đặt › Phiên bản & đồ họa Roblox**, bấm **Kiểm tra bản cập nhật Bloxkeep**, và chơi từng tài khoản một cho tới khi có bản sửa.
- Đang dùng launcher? Nếu launcher chặn mở nhiều cửa sổ, chọn **Chạy thẳng RobloxPlayerBeta (bỏ qua launcher)** ở **Cài đặt › Khởi chạy › Mở Roblox bằng**.

### Tài khoản báo "Cần đăng nhập lại"

Roblox đã kết thúc phiên đăng nhập đã lưu (đổi mật khẩu, đăng xuất mọi phiên, hoặc một đợt kiểm tra bảo mật). Bấm **Đăng nhập** trên thẻ và xác nhận bằng điện thoại, hoặc thêm lại tài khoản bằng cách bất kỳ: Bloxkeep chỉ cập nhật chứ không tạo bản trùng. Bộ lọc **Cần đăng nhập lại** gom hết các tài khoản này. **Kiểm tra đăng nhập mỗi ngày** sẽ phát hiện trước khi bạn bấm Vào game.

### Tài khoản bị ban

Nhãn **Bị ban** lấy từ thông tin công khai của Roblox, đọc trong lúc kiểm tra đăng nhập. **Vào game** và preset bỏ qua tài khoản này, tự vào lại game cũng không mở lại nó. Khi hết ban, bấm **⋯ › Kiểm tra đăng nhập** để bỏ nhãn.

### Máy chưa cài Roblox, hoặc chỉ có bản Microsoft Store

- Thanh bên báo "Hình như máy này chưa cài Roblox.": cài Roblox từ roblox.com/download.
- Chỉ có Roblox bản Microsoft Store: chọn **Roblox (Microsoft Store), đang thử nghiệm** ở **Cài đặt › Khởi chạy › Mở Roblox bằng**, hoặc cài Roblox từ roblox.com.
- Launcher nào chưa cài thì ghi "(chưa cài)" trong danh sách đó. Hãy chọn cách mở khác.

### "Sao chép mật khẩu" bị mờ hoặc báo chưa lưu

- Bị mờ: bạn đang chọn nhiều tài khoản. Các mục sao chép chỉ dùng cho một tài khoản mỗi lần.
- "Tài khoản này chưa lưu mật khẩu": Bloxkeep không giữ lại mật khẩu lúc đăng nhập. Hãy gõ mật khẩu ở **Sửa (F2) › Mật khẩu Roblox** rồi lưu. Xem [Sao chép mật khẩu](#sao-chép-mật-khẩu-và-cách-lưu-mật-khẩu).
- **Sao chép mã 2FA** cũng vậy, với **Sửa › Khóa app xác thực**.

### Các mục bị mờ khác

| Bị mờ | Vì sao |
|---|---|
| Nút **Vào game** ở thanh dưới ("Chọn tài khoản") | Chưa chọn tài khoản nào. |
| **Vào game** trên thẻ | Tài khoản Roblox VNG, hoặc tài khoản đang được mở. |
| **Mở Roblox**, **Vào cùng người chơi…** | Chưa chọn tài khoản nào. |
| **Đóng** ở thanh dưới, **Đóng Roblox**, **Buộc tắt** | Không tài khoản nào đã chọn đang mở trên máy này. |
| **Sửa…**, **Vào cùng server**, **Trang cá nhân Roblox**, **Mở trang web**, các mục **Sao chép** | Đang chọn nhiều tài khoản. **Vào cùng server** còn cần biết server; **Mở trang web** không dùng được cho Roblox VNG. |
| **Chạy** của preset | Preset không còn tài khoản, hoặc đang có lượt mở khác. |
| **Đồ họa**, **Giới hạn FPS**, **Âm lượng** | **Áp dụng cho** đang là **Không áp dụng**. |
| Các dòng trong **Loại sự kiện** | **Thông báo Windows** đang tắt. |
| **Sao lưu ngay** | Chưa chọn thư mục sao lưu. |
| **Hỏi lại** | Bạn chưa bỏ qua câu hỏi nào. |
| **Tự động cập nhật** | Bản này không tự cập nhật được. |

### Dữ liệu nằm ở đâu?

**Cài đặt › Sao lưu & dữ liệu › Nơi lưu dữ liệu › Mở thư mục** sẽ mở đúng chỗ. Bản cài đặt dùng `%LOCALAPPDATA%\Bloxkeep`; bản portable dùng thư mục `data` nằm cạnh `Bloxkeep.exe`. Tài khoản nằm trong `vault.rmv`, đã mã hóa; `vault.rmv.bak` là bản lưu trước đó; `error.log` ghi lỗi, không có thông tin tài khoản. Đừng bao giờ gửi `vault.rmv` hay file sao lưu cho ai. Danh sách đầy đủ có trong [README](../README.vi.md#dữ-liệu-và-bảo-mật).

### Quên mật khẩu chủ

Không lấy lại được. Hãy khôi phục một bản sao lưu mà bạn còn nhớ mật khẩu (**Khôi phục từ bản sao lưu** ở màn hình đầu, hoặc **Cài đặt › Sao lưu & dữ liệu › Khôi phục…**), hoặc xóa `vault.rmv` và `vault.rmv.bak` để làm lại từ đầu, chấp nhận mất các tài khoản đã lưu.

### Preset hẹn giờ không chạy

Đến giờ chạy, Bloxkeep phải đang chạy và đã mở khóa. Những lần bị lỡ khi Bloxkeep tắt, đang khóa hoặc máy ngủ sẽ không được chạy bù. Lần chạy cũng bị bỏ qua nếu cần bạn xác nhận mới mở được (Multi-Roblox tắt hoặc chưa sẵn sàng, thiếu RAM, có tài khoản đang ở game khác); thông báo sẽ nói rõ lý do.

### Đã thoát game mà vẫn báo đang chơi

Trạng thái của Roblox bị chậm, có khi tới vài phút. Với các cửa sổ trên máy này, Bloxkeep đọc theo log của chính Roblox nên không bị ảnh hưởng.

### Gửi góp ý kèm nhật ký lỗi

**Cài đặt › Cập nhật Bloxkeep › Gửi góp ý › Viết góp ý** (hoặc Ctrl+K rồi gõ "góp ý"):

1. **Bạn muốn gửi gì?**: **Báo lỗi** hoặc **Góp ý, ý tưởng**.
2. **Nội dung**, và **Cách liên hệ với bạn** (không bắt buộc) nếu bạn muốn được trả lời.
3. Ô **Gửi kèm nhật ký lỗi của Bloxkeep** được tích sẵn khi báo lỗi. Tên tài khoản, cookie và mật khẩu được xóa trước khi gửi. Bloxkeep phải đang mở khóa mới gửi kèm được.
4. **Xem đúng những gì sẽ gửi đi** cho xem toàn bộ nội dung trước khi gửi.
5. Bấm **Gửi**. Mỗi phút chỉ gửi được một lần.

Lời nhắn đi thẳng tới người làm Bloxkeep qua Discord, không kèm gì khác từ máy bạn ([chi tiết](feedback.md), tiếng Anh).

---

## Quyền riêng tư

Bloxkeep không có máy chủ, tài khoản hay công cụ phân tích riêng. Tài khoản, cài đặt và lịch sử của bạn chỉ nằm trên máy, trong một kho dữ liệu được mã hóa bằng mật khẩu chủ, và mật khẩu chủ không được lưu ở đâu cả. Dữ liệu chỉ rời khỏi máy cho những việc bạn dùng: Roblox (đăng nhập, trạng thái, game, server), GitHub (cập nhật Bloxkeep, không kèm dữ liệu tài khoản), thư mục sao lưu bạn chọn (kho dữ liệu đã mã hóa), webhook Discord của bạn (chỉ khi bạn bật, không bao giờ có cookie hay mật khẩu), và người làm Bloxkeep (chỉ những góp ý bạn tự gửi). Danh sách đầy đủ có trong [chính sách quyền riêng tư](../PRIVACY.md) (tiếng Anh).
